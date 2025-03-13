use std::io::SeekFrom;

use xx_core::coroutines::{get_context, scoped};

use super::*;
use crate::async_tests::util::read::*;

#[main]
#[test]
pub async fn test_buf_reader() -> Result<()> {
	let context = get_context().await;
	let mut reader = BufReader::new(Sequential::new());
	let mut buf = [0u8; 20];
	let mut stream_pos = 0;
	let mut inner_pos = 0;
	let mut position = 0;
	let mut filled = 0;
	let cap = reader.capacity() as u64;
	let len = reader.stream_len().await?;

	macro_rules! wait {
		($expr:expr) => {
			unsafe { scoped(context, $expr) }
		};
	}

	macro_rules! advance {
		($amount:expr) => {
			stream_pos += $amount as u64;
		};
	}

	macro_rules! consume {
		($amount:expr) => {
			let amount = $amount;

			advance!(amount);

			reader.consume(amount);
			position += amount;

			check!();
		};
	}

	macro_rules! discard {
		() => {
			let discarded = (filled - position) as u64;

			advance!(discarded);

			reader.discard();
			position = 0;
			filled = 0;

			check!();
		};
	}

	macro_rules! read_check {
		($buf:expr) => {
			let mut i = stream_pos as u8;

			for b in $buf.iter() {
				assert_eq!(*b, i);

				i = i.wrapping_add(1);
			}
		};
	}

	macro_rules! read_amount {
		($buf:expr, $amount:expr) => {
			if reader.buffer().len() == 0 {
				let fill = reader.capacity().min((len - stream_pos) as usize);

				inner_pos += fill as u64;

				if fill != 0 {
					position = 0;
					filled = fill;
				}
			}

			let amount = $amount;

			assert_eq!(wait!(reader.read($buf))?, amount);
			read_check!(&$buf[..amount]);

			stream_pos += amount as u64;
			position += amount;

			check!();
		};
	}

	macro_rules! read_exact {
		($buf:expr) => {
			read_amount!($buf, $buf.len());
		};
	}

	macro_rules! read_remaining {
		($buf:expr) => {
			assert_eq!(reader.buffer().len(), filled - position);
			read_amount!($buf, filled - position);
		};
	}

	macro_rules! read_large {
		($buf:expr) => {
			assert_eq!(reader.buffer().len(), 0);
			assert_eq!(wait!(reader.read($buf))?, $buf.len());
			read_check!($buf);

			stream_pos += $buf.len() as u64;
			inner_pos += $buf.len() as u64;

			check!();
		};
	}

	macro_rules! check {
		() => {
			assert_eq!(reader.capacity(), cap as usize);
			assert_eq!(reader.position(), position as usize);
			assert_eq!(reader.buffer().len(), filled - position as usize);
			assert_eq!(
				reader.spare_capacity() + reader.position() + reader.buffer().len(),
				reader.capacity()
			);
			assert_eq!(wait!(reader.stream_position())?, stream_pos);
			assert_eq!(wait!(reader.inner_mut().stream_position())?, inner_pos);
			assert_eq!(reader.spare_capacity(), reader.capacity() - filled);
			assert_eq!(
				reader.stream_len_fast(),
				reader.inner_mut().stream_len_fast()
			);
			assert_eq!(
				reader.stream_position_fast(),
				reader.inner_mut().stream_position_fast()
			);
		};
	}

	macro_rules! move_data_to_beginning {
		() => {
			let avail = reader.buffer().len();

			position = 0;
			filled = avail;
			reader.move_data_to_beginning();

			check!();
		};
	}

	macro_rules! seek {
		($seek:expr) => {
			wait!(reader.seek($seek))?;

			let pos = match $seek {
				SeekFrom::Start(n) => Some(n),
				SeekFrom::Current(n) => stream_pos.checked_add_signed(n),
				SeekFrom::End(n) => len.checked_add_signed(n)
			}
			.ok_or(ErrorKind::Overflow)?;

			stream_pos = pos;
			inner_pos = pos;
			position = 0;
			filled = 0;

			check!();
		};
	}

	macro_rules! fill_amount {
		($amount:expr, $requested:expr) => {
			let amount = $amount;
			let requested = $requested;

			if reader.buffer().len() < requested {
				if reader.spare_capacity() < requested {
					filled -= position;
					position = 0;
				}

				filled += amount;
				inner_pos += amount as u64;
			}

			assert_eq!(wait!(reader.fill_amount(requested))?, amount);
			read_check!(reader.buffer());
			check!();
		};
	}

	read_exact!(&mut buf);

	let buf = &mut buf[0..10];

	read_exact!(buf);
	discard!();

	read_exact!(buf);
	assert_eq!(reader.position(), 10);

	move_data_to_beginning!();

	read_exact!(buf);

	consume!(reader.buffer().len());
	move_data_to_beginning!();
	move_data_to_beginning!();

	let mut buf = vec![0; reader.capacity() * 2 + 559];

	read_exact!(&mut buf[0..10]);
	read_remaining!(&mut buf);
	read_large!(&mut buf);

	seek!(SeekFrom::End(-20));
	read_amount!(&mut buf[0..200], 20);
	read_amount!(&mut buf[0..200], 0);

	seek!(SeekFrom::Start(0));
	fill_amount!(cap as usize, cap as usize);
	fill_amount!(0, cap as usize);
	read_check!(reader.buffer());

	consume!(10);
	fill_amount!(10, cap as usize);
	consume!(reader.buffer().len() - 200);

	for i in 0..200 {
		fill_amount!(0, i);
		assert_eq!(reader.buffer().len(), 200);
	}

	read_check!(reader.buffer());
	fill_amount!(1, 201);
	read_check!(reader.buffer());

	seek!(SeekFrom::End(-10));
	fill_amount!(10, cap as usize);
	fill_amount!(0, cap as usize);
	read_check!(reader.buffer());

	let len = reader.buffer().len();

	for _ in 0..len {
		consume!(1);
	}

	for _ in 0..len {
		stream_pos -= 1;
		position -= 1;
		reader.unconsume(1);

		check!();
	}

	read_check!(reader.buffer());
	discard!();

	seek!(SeekFrom::Start(0));
	seek!(SeekFrom::Current(349083));

	fill_amount!(cap as usize, cap as usize);
	consume!(400);
	seek!(SeekFrom::Current(cap as i64));

	Ok(())
}

#[main]
#[test]
#[should_panic = "assertion failed: len <= buf.len()"]
pub async fn test_malformed_read() {
	let mut reader = BufReader::new(MalformedRead);
	let mut buf = [0u8; 20];

	reader.read(&mut buf).await.unwrap();
}

#[main]
#[test]
#[should_panic = "assertion failed: amount <= self.capacity()"]
pub async fn test_over_fill() {
	let mut reader = BufReader::new(Sequential::new());

	reader.fill_amount(reader.capacity() + 1).await.unwrap();
}

#[main]
#[test]
#[should_panic = "assertion failed: count <= self.buffer().len()"]
pub async fn test_over_consume() {
	let mut reader = BufReader::new(Sequential::new());

	reader.fill_amount(200).await.unwrap();
	reader.consume(100);
	reader.consume(101);
}

#[main]
#[test]
#[should_panic = "`count` > `self.position()`"]
pub async fn test_over_unconsume() {
	let mut reader = BufReader::new(Sequential::new());

	reader.fill_amount(200).await.unwrap();
	reader.consume(100);
	reader.unconsume(101);
}

#[main]
#[test]
pub async fn test_buf_reader_capacity() {
	let capacities = [2000, 500, 16383, 8220, 0, 1024 * 1024];

	for capacity in capacities {
		let reader = BufReader::with_capacity(Sequential::new(), capacity);

		assert_eq!(reader.capacity(), capacity);
	}
}

#[main]
#[test]
#[should_panic = "capacity overflow"]
pub async fn test_buf_reader_capacity_fail() {
	BufReader::with_capacity(Sequential::new(), isize::MAX as usize + 1);
}

#[main]
#[test]
pub async fn test_buf_reader_from_parts() {
	let capacities = [2000, 500, 16383, 8220, 0];

	for capacity in capacities {
		let reader = BufReader::from_parts(Sequential::new(), Vec::with_capacity(capacity), 0);

		assert_eq!(reader.capacity(), capacity);

		if capacity > 20 {
			for pos in [15, capacity - 5, capacity] {
				let reader = BufReader::from_parts(Sequential::new(), vec![0; capacity], pos);

				assert_eq!(reader.capacity(), capacity);
			}
		}
	}
}

#[main]
#[test]
#[should_panic = "assertion failed: pos <= len"]
pub async fn test_buf_reader_from_parts_fail1() {
	BufReader::from_parts(Sequential::new(), Vec::with_capacity(20), 1);
}

#[main]
#[test]
#[should_panic = "assertion failed: pos <= len"]
pub async fn test_buf_reader_from_parts_fail2() {
	BufReader::from_parts(Sequential::new(), vec![0; 20], 21);
}
