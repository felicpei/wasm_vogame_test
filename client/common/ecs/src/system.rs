use crate::metrics::SysMetrics;
use specs::{ReadExpect, RunNow};
use std::{collections::HashMap, time::Instant};

/// measuring the level of threads a unit of code ran on. Use Rayon when it ran
/// on their threadpool. Use Exact when you know on how many threads your code
/// ran on exactly.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ParMode {
    None, /* Job is not running at all */
    Single,
    Rayon,
    Exact(u32),
}

//TODO: make use of the phase of a system for advanced scheduling and logging
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Create,
    Review,
    Apply,
}

//TODO: make use of the origin of the system for better logging
#[derive(Clone, PartialEq, Debug)]
pub enum Origin {
    Common,
    Client,
    Server,
    Frontend(&'static str),
}

impl Origin {
    fn name(&self) -> &'static str {
        match self {
            Origin::Common => "Common",
            Origin::Client => "Client",
            Origin::Server => "Server",
            Origin::Frontend(name) => name,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct CpuTimeline {
    /// measurements for a System
    /// - The first entry will always be ParMode::Single, as when the
    ///   System::run is executed, we run
    /// single threaded until we start a Rayon::ParIter or similar
    /// - The last entry will contain the end time of the System. To mark the
    ///   End it will always contain
    /// ParMode::None, which means from that point on 0 CPU threads work in this
    /// system
    measures: Vec<(Instant, ParMode)>,
}

#[derive(Default)]
pub struct CpuTimeStats {
    /// the first entry will always be 0, the last entry will always be `dt`
    /// `usage` starting from `ns`
    measures: Vec<(/* ns */ u64, /* usage */ f32)>,
}

/// Parallel Mode tells us how much you are scaling. `None` means your code
/// isn't running. `Single` means you are running single threaded.
/// `Rayon` means you are running on the rayon threadpool.
impl ParMode {
    fn threads(&self, rayon_threads: u32) -> u32 {
        match self {
            ParMode::None => 0,
            ParMode::Single => 1,
            ParMode::Rayon => rayon_threads,
            ParMode::Exact(u) => *u,
        }
    }
}

impl CpuTimeline {
    fn reset(&mut self) {
        self.measures.clear();
        self.measures.push((Instant::now(), ParMode::Single));
    }

    /// Start a new measurement. par will be covering the parallelisation AFTER
    /// this statement, till the next / end of the System.
    pub fn measure(&mut self, par: ParMode) { self.measures.push((Instant::now(), par)); }

    fn end(&mut self) -> std::time::Duration {
        let end = Instant::now();
        self.measures.push((end, ParMode::None));
        end.duration_since(
            self.measures
                .first()
                .expect("We just pushed onto the vector.")
                .0,
        )
    }

    fn get(&self, time: Instant) -> ParMode {
        match self.measures.binary_search_by_key(&time, |&(a, _)| a) {
            Ok(id) => self.measures[id].1,
            Err(0) => ParMode::None, /* not yet started */
            Err(id) => self.measures[id - 1].1,
        }
    }
}

impl CpuTimeStats {
    pub fn length_ns(&self) -> u64 { self.end_ns() - self.start_ns() }

    pub fn start_ns(&self) -> u64 {
        self.measures
            .iter()
            .find(|e| e.1 > 0.001)
            .unwrap_or(&(0, 0.0))
            .0
    }

    pub fn end_ns(&self) -> u64 { self.measures.last().unwrap_or(&(0, 0.0)).0 }

    pub fn avg_threads(&self) -> f32 {
        let mut sum = 0.0;
        for w in self.measures.windows(2) {
            let len = w[1].0 - w[0].0;
            let h = w[0].1;
            sum += len as f32 * h;
        }
        sum / (self.length_ns() as f32)
    }
}

/// The Idea is to transform individual timelines per system to a map of all
/// cores and what they (prob) are working on.
///
/// # Example
///
/// - Input: 3 services, 0 and 1 are 100% parallel and 2 is single threaded. `-`
///   means no work for *0.5s*. `#` means full work for *0.5s*. We see the first
///   service starts after 1s and runs for 3s The second one starts a sec later
///   and runs for 4s. The last service runs 2.5s after the tick start and runs
///   for 1s. Read left to right.
/// ```ignore
/// [--######------]
/// [----########--]
/// [-----##-------]
/// ```
///
/// - Output: a Map that calculates where our 6 cores are spending their time.
///   Here each number means 50% of a core is working on it. A '-' represents an
///   idling core. We start with all 6 cores idling. Then all cores start to
///   work on task 0. 2s in, task1 starts and we have to split cores. 2.5s in
///   task2 starts. We have 6 physical threads but work to fill 13. Later task 2
///   and task 0 will finish their work and give more threads for task 1 to work
///   on. Read top to bottom
/// ```ignore
/// 0-1s     [------------]
/// 1-2s     [000000000000]
/// 2-2.5s   [000000111111]
/// 2.5-3.5s [000001111122]
/// 3.5-4s   [000000111111]
/// 4-6s     [111111111111]
/// 6s..     [------------]
/// ```
pub fn gen_stats(
    timelines: &HashMap<String, CpuTimeline>,
    tick_work_start: Instant,
    rayon_threads: u32,
    physical_threads: u32,
) -> HashMap<String, CpuTimeStats> {
    let mut result = HashMap::new();
    let mut all = timelines
        .iter()
        .flat_map(|(s, t)| {
            let mut stat = CpuTimeStats::default();
            stat.measures.push((0, 0.0));
            result.insert(s.clone(), stat);
            t.measures.iter().map(|e| &e.0)
        })
        .collect::<Vec<_>>();

    all.sort();
    all.dedup();
    for time in all {
        let relative_time = time.duration_since(tick_work_start).as_nanos() as u64;
        // get all parallelisation at this particular time
        let individual_cores_wanted = timelines
            .iter()
            .map(|(k, t)| (k, t.get(*time).threads(rayon_threads)))
            .collect::<Vec<_>>();
        let total = individual_cores_wanted
            .iter()
            .map(|(_, a)| a)
            .sum::<u32>()
            .max(1) as f32;
        let total_or_max = total.max(physical_threads as f32);
        // update ALL states
        for individual in individual_cores_wanted.iter() {
            let actual = (individual.1 as f32 / total_or_max) * physical_threads as f32;
            if let Some(p) = result.get_mut(individual.0) {
                if p.measures
                    .last()
                    .map(|last| (last.1 - actual).abs())
                    .unwrap_or(0.0)
                    > 0.0001
                {
                    p.measures.push((relative_time, actual));
                }
            } else {
                log::warn!("Invariant violation: keys in both hashmaps should be the same.");
            }
        }
    }
    result
}

/// This trait wraps around specs::System and does additional veloren tasks like
/// metrics collection
///
/// ```
/// use specs::Read;
/// pub use veloren_common_ecs::{Job, Origin, ParMode, Phase, System};
/// # use std::time::Duration;
/// pub struct Sys;
/// impl<'a> System<'a> for Sys {
///     type SystemData = (Read<'a, ()>, Read<'a, ()>);
///
///     const NAME: &'static str = "example";
///     const ORIGIN: Origin = Origin::Frontend("voxygen");
///     const PHASE: Phase = Phase::Create;
///
///     fn run(job: &mut Job<Self>, (_read, _read2): Self::SystemData) {
///         std::thread::sleep(Duration::from_millis(100));
///         job.cpu_stats.measure(ParMode::Rayon);
///         std::thread::sleep(Duration::from_millis(500));
///         job.cpu_stats.measure(ParMode::Single);
///         std::thread::sleep(Duration::from_millis(40));
///     }
/// }
/// ```
pub trait System<'a> {
    const NAME: &'static str;
    const PHASE: Phase;
    const ORIGIN: Origin;

    type SystemData: specs::SystemData<'a>;
    fn run(job: &mut Job<Self>, data: Self::SystemData);
    fn sys_name() -> String { format!("{}_{}_sys", Self::ORIGIN.name(), Self::NAME) }
}

pub fn dispatch<'a, 'b, T>(builder: &mut specs::DispatcherBuilder<'a, 'b>, dep: &[&str])
where
    T: for<'c> System<'c> + Send + 'a + Default,
{
    builder.add(Job::<T>::default(), &T::sys_name(), dep);
}

pub fn run_now<'a, 'b, T>(world: &'a specs::World)
where
    T: for<'c> System<'c> + Send + 'a + Default,
{
    Job::<T>::default().run_now(world);
}

/// This Struct will wrap the System in order to avoid the can only impl trait
/// for local defined structs error It also contains the cpu measurements
pub struct Job<T>
where
    T: ?Sized,
{
    pub own: Box<T>,
    pub cpu_stats: CpuTimeline,
}

impl<'a, T> specs::System<'a> for Job<T>
where
    T: System<'a>,
{
    type SystemData = (T::SystemData, ReadExpect<'a, SysMetrics>);

    fn run(&mut self, data: Self::SystemData) {
        self.cpu_stats.reset();
        T::run(self, data.0);
        let millis = self.cpu_stats.end().as_millis();
        let name = T::NAME;
        if millis > 500 {
            log::warn!("slow system execution:{}{}", millis, name);
        }
        data.1
            .stats
            .lock()
            .unwrap()
            .insert(T::NAME.to_string(), self.cpu_stats.clone());
    }
}

impl<'a, T> Default for Job<T>
where
    T: System<'a> + Default,
{
    fn default() -> Self {
        Self {
            own: Box::new(T::default()),
            cpu_stats: CpuTimeline::default(),
        }
    }
}
