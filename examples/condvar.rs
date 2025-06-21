use core::time::Duration;
use std::thread;
#[cfg(feature = "alloc")]
use syncrs::Arc;
#[cfg(not(feature = "alloc"))]
use std::sync::Arc;
use syncrs::{Mutex, Condvar};

pub fn main() {
    let mutex = Arc::new(Mutex::new(10));
    let cvar = Arc::new(Condvar::new());

    let m = Arc::clone(&mutex);
    let cv = Arc::clone(&cvar);
    let reader_t = thread::spawn(move || {
        let lock = m.lock();
        let _ = cv.wait_while(lock, |n| {
            println!("[R]: Woke up, comparing {n} > 0 = {}", *n > 0);
            *n > 0
        });
        println!("[R]: EXIT");
    });

    let writer_t = thread::spawn(move || {
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            let mut lock = mutex.lock();
            *lock -= 1;
            println!("[W]: Sending notify");
            cvar.notify_one();
        }
        println!("[W]: EXIT");
    });

    writer_t.join().unwrap();
    reader_t.join().unwrap();
}
