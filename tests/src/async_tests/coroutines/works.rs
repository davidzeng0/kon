use super::*;

#[asynchronous]
async fn async_add(a: i32, b: i32) -> i32 {
	a + b
}

#[main]
#[test]
pub async fn test_add() {
	let result = async_add(12, 5).await;

	assert_eq!(result, 17);
}
