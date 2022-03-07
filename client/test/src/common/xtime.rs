

pub fn now() -> f64 {
    instant::now()
}

pub fn now_sec() -> i32 {
    let now = instant::now();
    let res = now / 1000.0;
    res as i32
}