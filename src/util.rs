use std::time::{Duration, Instant};

pub fn measure_exec_time<T, F: FnOnce() -> T>(f: F) -> (T, Duration) {
    let start = Instant::now();
    let result = f();
    let duration = Instant::now() - start;
    (result, duration)
}