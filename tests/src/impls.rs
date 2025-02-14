use xx_core::impls::UintExt;

#[test]
fn test_overflowing_signed_difference() {
	let test_data = [
		(u64::MAX, u64::MAX, (0, false), 0),
		(u64::MAX, 0, (-1, true), i64::MAX),
		(0, u64::MAX, (1, true), i64::MIN),
		(0x1000, 0x4000, (-0x3000, false), -0x3000),
		(0x4567, 0x1234, (0x3333, false), 0x3333),
		(0, i64::MAX as u64 + 1, (i64::MIN, false), i64::MIN),
		(0, i64::MAX as u64 + 2, (i64::MAX, true), i64::MIN),
		(i64::MAX as u64 + 1, 2, (i64::MAX - 1, false), i64::MAX - 1),
		(u64::MAX, i64::MAX as u64, (i64::MIN, true), i64::MAX),
		(u64::MAX, i64::MAX as u64 + 1, (i64::MAX, false), i64::MAX)
	];

	for (a, b, ov_res, sat_res) in test_data {
		assert_eq!(a.overflowing_signed_diff(b), ov_res);
		assert_eq!(a.saturating_signed_diff(b), sat_res);
	}
}
