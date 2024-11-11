use std::thread::sleep;
use std::time::{Duration, Instant};

pub fn measure_exec_time<T, F: FnOnce() -> T>(f: F) -> (T, Duration) {
    let start = Instant::now();
    let result = f();
    let duration = Instant::now() - start;
    (result, duration)
}

pub fn vec_to_array<const N: usize>(vec: Vec<u8>) -> [u8; N] {
    let boxed_slice = vec.into_boxed_slice();
    let boxed_array: Box<[u8; N]> = match boxed_slice.try_into() {
        Ok(array) => array,
        Err(_) => panic!("vector has an incorrect length"),
    };
    *boxed_array
}

#[allow(dead_code)]
pub fn pause() {
    sleep(Duration::from_secs(10));
}