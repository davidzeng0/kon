use std::time::Duration;

use xx_core::macros::duration;

#[test]
fn test_duration() {
	assert_eq!(Duration::from_secs(170), duration!(2 m 50 s));
	assert_eq!(Duration::from_secs(3600), duration!(1 h));
	assert_eq!(
		Duration::from_nanos(432_020_000_100_001),
		duration!(5 d 20 s 100 us 1 ns)
	);

	assert_eq!(Duration::from_secs(745), duration!(12::25));
	assert_eq!(Duration::from_secs(35547), duration!(9:52:27));
	assert_eq!(Duration::from_secs(2_022_747), duration!(23:9:52:27));
	assert_eq!(Duration::from_secs_f64(182.5), duration!(2.2 m 50.5 s));
	assert_eq!(Duration::from_secs(125), duration!(2 m) + duration!(5 s));

	assert_eq!(Duration::from_secs(86400), duration!(1 d));
	assert_eq!(Duration::from_secs(3600), duration!(1 h));
	assert_eq!(Duration::from_secs(60), duration!(1 m));
	assert_eq!(Duration::from_secs(1), duration!(1 s));
	assert_eq!(Duration::from_nanos(1_000_000), duration!(1 ms));
	assert_eq!(Duration::from_nanos(1_000), duration!(1 us));
	assert_eq!(Duration::from_nanos(1), duration!(1 ns));

	assert_eq!(Duration::from_secs_f64(60.002), duration!(1 ms 1 m 1 ms));

	assert_eq!(Duration::from_millis(1), duration!(1 / 1000));
	assert_eq!(Duration::from_nanos(1_003_009), duration!(1 / 997));
	assert_eq!(Duration::from_millis(200), duration!(1 / 5));
	assert_eq!(Duration::from_secs(10), duration!(20 / 2));

	let var = 90;
	let var2 = 34;

	assert_eq!(Duration::from_nanos(3_913_043_478), duration!(var / 23));
	assert_eq!(Duration::from_nanos(11_111_111), duration!(1 / var));
	assert_eq!(Duration::from_nanos(22_222_222), duration!(2 / var));
	assert_eq!(Duration::from_nanos(2_647_058_823), duration!(var / var2));
	assert_eq!(Duration::from_nanos(377_777_777), duration!(var2 / var));
	assert_eq!(
		Duration::from_nanos(75_555_555),
		duration!(var2 / (var * 5))
	);

	assert_eq!(Duration::from_secs(var * 60), duration!(var minutes));
}
