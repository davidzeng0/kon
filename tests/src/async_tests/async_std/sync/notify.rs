use std::rc::Rc;
use std::time::Duration;

use xx_core::async_std::sync::RcNotify;
use xx_core::error::Result;
use xx_pulse::*;

#[asynchronous]
async fn waiter(notify: RcNotify) -> Result<()> {
	notify.wait().await
}

#[asynchronous]
async fn canceller(notify: RcNotify) -> Result<()> {
	match select(notify.wait(), sleep(Duration::from_secs(1))).await {
		Select::First(f, _) => f,
		Select::Second(_, f) => f.unwrap()
	}
}

#[asynchronous]
async fn nested_cancel(notify: RcNotify) {
	match select(notify.wait(), waiter(notify.clone())).await {
		Select::First(success, error) => (success.unwrap(), error.unwrap().unwrap_err()),
		Select::Second(..) => panic!("Order failed")
	};
}

#[asynchronous]
async fn expect_success(notify: RcNotify) {
	waiter(notify).await.unwrap();
}

#[asynchronous]
async fn spawn_within(notify: RcNotify) -> JoinHandle<()> {
	notify.wait().await.unwrap();

	spawn(expect_success(notify.clone())).await
}

#[asynchronous]
async fn notify_within(notify: RcNotify) {
	notify.wait().await.unwrap();

	for _ in 0..30 {
		spawn(expect_success(notify.clone())).await;
	}

	notify.notify(());
}

#[main]
#[test]
async fn test_notify() {
	let notify = RcNotify::new();
	let handle = spawn(waiter(notify.clone())).await;

	notify.notify(());
	handle.await.unwrap();
	notify.notify(());

	let notify = RcNotify::new();
	let handle = spawn(canceller(notify.clone())).await;

	sleep(Duration::from_secs(1)).await.unwrap();

	notify.notify(());
	handle.await.unwrap_err();
	notify.notify(());

	let notify = RcNotify::new();

	spawn(nested_cancel(notify.clone())).await;

	for _ in 0..30 {
		for _ in 0..30 {
			spawn(expect_success(notify.clone())).await;
		}

		spawn(nested_cancel(notify.clone())).await;
	}

	notify.notify(());
	notify.notify(());

	let notify = RcNotify::new();
	let mut handle = None;

	for i in 0..30 {
		if i == 1 {
			handle = Some(spawn(spawn_within(notify.clone())).await);
		}

		spawn(nested_cancel(notify.clone())).await;

		for _ in 0..30 {
			spawn(expect_success(notify.clone())).await;
		}
	}

	spawn(nested_cancel(notify.clone())).await;

	let handle = handle.unwrap();

	assert!(!handle.is_done());

	notify.notify(());

	let handle = handle.await;

	assert!(!handle.is_done());

	notify.notify(());

	assert!(handle.is_done());

	notify.notify(());

	let notify = RcNotify::new();

	for _ in 0..30 {
		spawn(spawn_within(notify.clone())).await;
	}

	notify.notify(());
	notify.notify(());
	notify.notify(());

	let notify = RcNotify::new();

	for _ in 0..30 {
		spawn(notify_within(notify.clone())).await;
	}

	notify.notify(());
	notify.notify(());
}
