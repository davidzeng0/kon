use std::time::Duration;

use xx_pulse::*;

use super::*;

#[asynchronous]
async fn sync_complete() -> Result<i32> {
	Ok(23)
}

#[asynchronous]
async fn async_complete() -> Result<i32> {
	sleep(Duration::from_secs(5)).await?;

	Ok(23)
}

#[asynchronous]
async fn spawn_sync() -> Result<i32> {
	spawn(sync_complete()).await.await
}

#[asynchronous]
async fn spawn_async() -> Result<i32> {
	spawn(async_complete()).await.await
}

#[asynchronous]
async fn sync_finish_await() -> Result<()> {
	let result = spawn(sync_complete()).await;
	let result = result.await?;

	assert_eq!(result, 23);

	Ok(())
}

#[asynchronous]
async fn sync_finish_drop() -> Result<()> {
	spawn(sync_complete()).await;

	Ok(())
}

#[asynchronous]
async fn async_finish_await() -> Result<()> {
	let result = spawn(async_complete()).await;
	let result = result.await?;

	assert_eq!(result, 23);

	Ok(())
}

#[asynchronous]
async fn async_finish_drop() -> Result<()> {
	spawn(async_complete()).await;

	Ok(())
}

#[asynchronous]
async fn do_sleep() -> Result<i32> {
	sleep(Duration::from_secs(1)).await?;

	Ok(-20)
}

#[asynchronous]
async fn async_cancel() -> Result<()> {
	let result = select(spawn(async_complete()).await, do_sleep()).await;

	let result = match result {
		Select::First(result, _) => result,
		Select::Second(result, _) => result
	}?;

	assert_eq!(result, -20);

	Ok(())
}

#[main]
#[test]
async fn test_concurrency() -> Result<()> {
	sync_finish_await().await?;
	sync_finish_drop().await?;
	async_finish_await().await?;
	async_finish_drop().await?;
	async_cancel().await?;
	select(spawn(sync_complete()).await, do_sleep()).await;

	select(
		select(spawn(sync_complete()).await, do_sleep()),
		sync_complete()
	)
	.await;

	select(do_sleep(), spawn(sync_complete()).await).await;

	select(
		select(do_sleep(), spawn(sync_complete()).await),
		sync_complete()
	)
	.await;

	select(spawn_sync(), do_sleep()).await;
	select(select(spawn_sync(), do_sleep()), sync_complete()).await;
	select(do_sleep(), spawn_sync()).await;
	select(select(do_sleep(), spawn_sync()), sync_complete()).await;
	select(spawn_async(), do_sleep()).await;
	select(select(spawn_async(), do_sleep()), sync_complete()).await;
	select(do_sleep(), spawn_async()).await;
	select(select(do_sleep(), spawn_async()), sync_complete()).await;

	select(
		join(select(do_sleep(), spawn_async()), spawn_async()),
		sync_complete()
	)
	.await;

	select(
		select(
			join(
				select(join(do_sleep(), spawn_async()), spawn_async()),
				spawn_async()
			),
			spawn_async()
		),
		sync_complete()
	)
	.await;

	Ok(())
}
