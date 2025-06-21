use syncrs::Semaphore;
#[cfg(feature = "alloc")]
use syncrs::Arc;
#[cfg(not(feature = "alloc"))]
use std::sync::Arc;
use std::{thread, time::Duration};

pub fn main() {
    let semaphore = Arc::new(Semaphore::new());

    let sem1 = Arc::clone(&semaphore);
    let wait_t = thread::spawn(move || {
        for _ in 0..5 {
            sem1.dec(1);
            println!("Thread 1 decremented");
        }
    });

    let sem2 = Arc::clone(&semaphore);
    let writer_t = thread::spawn(move || {
        println!("Thread 2 increments by 1");
        sem2.inc(1);
        thread::sleep(Duration::from_millis(100));

        println!("Thread 2 increments by 2");
        sem2.inc(2);
        thread::sleep(Duration::from_millis(100));

        println!("Thread 2 increments by 1");
        sem2.inc(1);
        thread::sleep(Duration::from_millis(100));

        println!("Thread 2 increments by 2");
        sem2.inc(2);
        thread::sleep(Duration::from_millis(100));
    });

    wait_t.join().unwrap();
    writer_t.join().unwrap();

    assert_eq!(semaphore.get_value(), 1);
}
