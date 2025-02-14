use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::thread;

use xx_core::sync::spin_mutex::*;

#[test]
fn test_poison() {
	let mut mutex = SpinMutex::new(5);

	*mutex.lock() = 7;

	assert!(*mutex.lock() == 7);

	catch_unwind(AssertUnwindSafe(|| {
		*mutex.lock() = 1;

		panic!()
	}));

	assert!(*mutex.lock() == 1);

	let lock = mutex.lock();

	match mutex.try_lock().unwrap_err() {
		TryLockError::WouldBlock => (),
		_ => assert!(false)
	}

	drop(lock);

	*mutex.get_mut() = 22;

	assert!(*mutex.lock() == 22);
}

#[test]
fn test_lock() {
	let mutex = Arc::new(SpinMutex::new(0));
	let mut handles = Vec::new();

	for _ in 0..4 {
		let mtx = mutex.clone();

		handles.push(thread::spawn(move || {
			for _ in 0..1_000 {
				let mut lock = mtx.lock();

				*lock += 1;

				drop(lock);
			}
		}));
	}

	for handle in handles {
		handle.join();
	}

	assert_eq!(*Arc::into_inner(mutex).unwrap().get_mut(), 4_000);
}
