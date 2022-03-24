use hashbrown::HashMap;
use rayon::ThreadPool;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use instant::Instant;

/// Provides a Wrapper around rayon threadpool to execute slow-jobs.
/// slow means, the job doesn't need to not complete within the same tick.
/// DO NOT USE I/O blocking jobs, but only CPU heavy jobs.
/// Jobs run here, will reduce the ammount of threads rayon can use during the
/// main tick.
///
/// ## Configuration
/// This Pool allows you to configure certain names of jobs and assign them a
/// maximum number of threads # Example
/// Your system has 16 cores, you assign 12 cores for slow-jobs.
/// Then you can configure all jobs with the name `CHUNK_GENERATOR` to spawn on
/// max 50% (6 = cores)
///
/// ## Spawn Order
/// - At least 1 job of a configuration is allowed to run if global limit isn't
///   hit.
/// - remaining capacities are spread in relation to their limit. e.g. a
///   configuration with double the limit will be sheduled to spawn double the
///   tasks, starting by a round robin.
///
/// ## States
/// - queued
/// - spawned
/// - started
/// - finished
/// ```
/// # use veloren_common::slowjob::SlowJobPool;
/// # use std::sync::Arc;
///
/// let threadpool = rayon::ThreadPoolBuilder::new()
///     .num_threads(16)
///     .build()
///     .unwrap();
/// let pool = SlowJobPool::new(3, 10, Arc::new(threadpool));
/// pool.configure("CHUNK_GENERATOR", |n| n / 2);
/// pool.spawn("CHUNK_GENERATOR", move || println!("this is a job"));
/// ```
#[derive(Clone)]
pub struct SlowJobPool {
    internal: Arc<Mutex<InternalSlowJobPool>>,
}

#[derive(Debug)]
pub struct SlowJob {
    name: String,
    id: u64,
}

struct InternalSlowJobPool {
    next_id: u64,
    queue: HashMap<String, VecDeque<Queue>>,
    configs: HashMap<String, Config>,
    last_spawned_configs: Vec<String>,
    global_spawned_and_running: u64,
    global_limit: u64,
    jobs_metrics_cnt: usize,
    jobs_metrics: HashMap<String, Vec<JobMetrics>>,
    threadpool: Arc<ThreadPool>,
    internal: Option<Arc<Mutex<Self>>>,
}

#[derive(Debug)]
struct Config {
    local_limit: u64,
    local_spawned_and_running: u64,
}

struct Queue {
    id: u64,
    name: String,
    task: Box<dyn FnOnce() + Send + Sync + 'static>,
}

pub struct JobMetrics {
    pub queue_created: Instant,
    pub execution_start: Instant,
    pub execution_end: Instant,
}

impl Queue {
    fn new<F>(name: &str, id: u64, internal: &Arc<Mutex<InternalSlowJobPool>>, f: F) -> Self
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        let internal = Arc::clone(internal);
        let name_cloned = name.to_owned();
        let queue_created = Instant::now();
        Self {
            id,
            name: name.to_owned(),
            task: Box::new(move || {
                let execution_start = Instant::now();
                f();
                let execution_end = Instant::now();
                let metrics = JobMetrics {
                    queue_created,
                    execution_start,
                    execution_end,
                };
                // directly maintain the next task afterwards
                {
                    let mut lock = internal.lock().expect("slowjob lock poisoned");
                    lock.finish(&name_cloned, metrics);
                    lock.spawn_queued();
                }
            }),
        }
    }
}

impl InternalSlowJobPool {
    pub fn new(
        global_limit: u64,
        jobs_metrics_cnt: usize,
        threadpool: Arc<ThreadPool>,
    ) -> Arc<Mutex<Self>> {
        let link = Arc::new(Mutex::new(Self {
            next_id: 0,
            queue: HashMap::new(),
            configs: HashMap::new(),
            last_spawned_configs: Vec::new(),
            global_spawned_and_running: 0,
            global_limit: global_limit.max(1),
            jobs_metrics_cnt,
            jobs_metrics: HashMap::new(),
            threadpool,
            internal: None,
        }));

        let link_clone = Arc::clone(&link);
        link.lock()
            .expect("poisoned on InternalSlowJobPool::new")
            .internal = Some(link_clone);
        link
    }

    /// returns order of configuration which are queued next
    fn calc_queued_order(
        &self,
        mut queued: HashMap<&String, u64>,
        mut limit: usize,
    ) -> Vec<String> {
        let mut roundrobin = self.last_spawned_configs.clone();
        let mut result = vec![];
        let spawned = self
            .configs
            .iter()
            .map(|(n, c)| (n, c.local_spawned_and_running))
            .collect::<HashMap<_, u64>>();
        let mut queried_capped = self
            .configs
            .iter()
            .map(|(n, c)| {
                (
                    n,
                    queued
                        .get(&n)
                        .cloned()
                        .unwrap_or(0)
                        .min(c.local_limit - c.local_spawned_and_running),
                )
            })
            .collect::<HashMap<_, _>>();
        // grab all configs that are queued and not running. in roundrobin order
        for n in roundrobin.clone().into_iter() {
            if let Some(c) = queued.get_mut(&n) {
                if *c > 0 && spawned.get(&n).cloned().unwrap_or(0) == 0 {
                    result.push(n.clone());
                    *c -= 1;
                    limit -= 1;
                    queried_capped.get_mut(&n).map(|v| *v -= 1);
                    roundrobin
                        .iter()
                        .position(|e| e == &n)
                        .map(|i| roundrobin.remove(i));
                    roundrobin.push(n);
                    if limit == 0 {
                        return result;
                    }
                }
            }
        }
        //schedule rest based on their possible limites, don't use round robin here
        let total_limit = queried_capped.values().sum::<u64>() as f32;
        if total_limit < f32::EPSILON {
            return result;
        }
        let mut spawn_rates = queried_capped
            .iter()
            .map(|(&n, l)| (n, ((*l as f32 * limit as f32) / total_limit).min(*l as f32)))
            .collect::<Vec<_>>();
        while limit > 0 {
            spawn_rates.sort_by(|(_, a), (_, b)| {
                if b < a {
                    core::cmp::Ordering::Less
                } else if (b - a).abs() < f32::EPSILON {
                    core::cmp::Ordering::Equal
                } else {
                    core::cmp::Ordering::Greater
                }
            });
            match spawn_rates.first_mut() {
                Some((n, r)) => {
                    if *r > f32::EPSILON {
                        result.push(n.clone());
                        limit -= 1;
                        *r -= 1.0;
                    } else {
                        break;
                    }
                },
                None => break,
            }
        }
        result
    }

    fn can_spawn(&self, name: &str) -> bool {
        let queued = self
            .queue
            .iter()
            .map(|(n, m)| (n, m.len() as u64))
            .collect::<HashMap<_, u64>>();
        let mut to_be_queued = queued.clone();
        let name = name.to_owned();
        *to_be_queued.entry(&name).or_default() += 1;
        let limit = (self.global_limit - self.global_spawned_and_running) as usize;
        // calculate to_be_queued first
        let to_be_queued_order = self.calc_queued_order(to_be_queued, limit);
        let queued_order = self.calc_queued_order(queued, limit);
        // if its queued one time more then its okay to spawn
        let to_be_queued_cnt = to_be_queued_order
            .into_iter()
            .filter(|n| n == &name)
            .count();
        let queued_cnt = queued_order.into_iter().filter(|n| n == &name).count();
        to_be_queued_cnt > queued_cnt
    }

    pub fn spawn<F>(&mut self, name: &str, f: F) -> SlowJob
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        let queue = Queue::new(name, id, self.internal.as_ref().expect("internal empty"), f);
        self.queue
            .entry(name.to_string())
            .or_default()
            .push_back(queue);
        debug_assert!(
            self.configs.contains_key(name),
            "Can't spawn unconfigured task!"
        );
        //spawn already queued
        self.spawn_queued();
        SlowJob {
            name: name.to_string(),
            id,
        }
    }

    fn finish(&mut self, name: &str, metrics: JobMetrics) {
        let metric = self.jobs_metrics.entry(name.to_string()).or_default();

        if metric.len() < self.jobs_metrics_cnt {
            metric.push(metrics);
        }
        self.global_spawned_and_running -= 1;
        if let Some(c) = self.configs.get_mut(name) {
            c.local_spawned_and_running -= 1;
        } else {
            log::warn!("sync_maintain on a no longer existing config {}", name);
        }
    }

    fn spawn_queued(&mut self) {
        let queued = self
            .queue
            .iter()
            .map(|(n, m)| (n, m.len() as u64))
            .collect::<HashMap<_, u64>>();
        let limit = self.global_limit as usize;
        let queued_order = self.calc_queued_order(queued, limit);
        for name in queued_order.into_iter() {
            match self.queue.get_mut(&name) {
                Some(deque) => match deque.pop_front() {
                    Some(queue) => {
                        //fire
                        self.global_spawned_and_running += 1;
                        self.configs
                            .get_mut(&queue.name)
                            .expect("cannot fire a unconfigured job")
                            .local_spawned_and_running += 1;
                        self.last_spawned_configs
                            .iter()
                            .position(|e| e == &queue.name)
                            .map(|i| self.last_spawned_configs.remove(i));
                        self.last_spawned_configs.push(queue.name.to_owned());
                        self.threadpool.spawn(queue.task);
                    },
                    None => log::error!(
                        "internal calculation is wrong, we extected a schedulable job to be \
                         present in the queue"
                    ),
                },
                None => log::error!(
                    "internal calculation is wrong, we marked a queue as schedulable which \
                     doesn't exist"
                ),
            }
        }
    }

    pub fn take_metrics(&mut self) -> HashMap<String, Vec<JobMetrics>> {
        core::mem::take(&mut self.jobs_metrics)
    }
}

impl SlowJobPool {
    pub fn new(global_limit: u64, jobs_metrics_cnt: usize, threadpool: Arc<ThreadPool>) -> Self {
        Self {
            internal: InternalSlowJobPool::new(global_limit, jobs_metrics_cnt, threadpool),
        }
    }

    /// configure a NAME to spawn up to f(n) threads, depending on how many
    /// threads we globally have available
    pub fn configure<F>(&self, name: &str, f: F)
    where
        F: Fn(u64) -> u64,
    {
        let mut lock = self.internal.lock().expect("lock poisoned while configure");
        let cnf = Config {
            local_limit: f(lock.global_limit).max(1),
            local_spawned_and_running: 0,
        };
        lock.configs.insert(name.to_owned(), cnf);
        lock.last_spawned_configs.push(name.to_owned());
    }

    /// spawn a new slow job on a certain NAME IF it can run immediately
    #[allow(clippy::result_unit_err)]
    pub fn try_run<F>(&self, name: &str, f: F) -> Result<SlowJob, ()>
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        let mut lock = self.internal.lock().expect("lock poisoned while try_run");
        //spawn already queued
        lock.spawn_queued();
        if lock.can_spawn(name) {
            Ok(lock.spawn(name, f))
        } else {
            Err(())
        }
    }

    pub fn spawn<F>(&self, name: &str, f: F) -> SlowJob
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        self.internal
            .lock()
            .expect("lock poisoned while spawn")
            .spawn(name, f)
    }

    pub fn cancel(&self, job: SlowJob) -> Result<(), SlowJob> {
        let mut lock = self.internal.lock().expect("lock poisoned while cancel");
        if let Some(m) = lock.queue.get_mut(&job.name) {
            let p = match m.iter().position(|p| p.id == job.id) {
                Some(p) => p,
                None => return Err(job),
            };
            if m.remove(p).is_some() {
                return Ok(());
            }
        }
        Err(job)
    }

    pub fn take_metrics(&self) -> HashMap<String, Vec<JobMetrics>> {
        self.internal
            .lock()
            .expect("lock poisoned while take_metrics")
            .take_metrics()
    }
}