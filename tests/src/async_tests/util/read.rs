use std::io::SeekFrom;

use super::*;

pub struct Sequential(u64, u64);

impl Sequential {
	pub fn new() -> Self {
		Self(0, 1024 * 1024 * 1024)
	}
}

#[asynchronous]
impl Read for Sequential {
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		read_into!(buf, (self.1 - self.0) as usize);

		for b in buf.iter_mut() {
			*b = self.0 as u8;
			self.0 += 1;
		}

		Ok(buf.len())
	}
}

#[asynchronous]
impl Seek for Sequential {
	async fn seek(&mut self, seek: SeekFrom) -> Result<u64> {
		match seek {
			SeekFrom::Start(pos) => self.0 = pos,
			SeekFrom::Current(rel) => self.0 = self.0.checked_add_signed(rel).unwrap(),
			SeekFrom::End(rel) => self.0 = self.1.checked_add_signed(rel).unwrap()
		}

		assert!(self.0 <= self.1);

		Ok(self.0)
	}
}

pub struct MalformedRead;

#[asynchronous]
impl Read for MalformedRead {
	async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		Ok(buf.len() + 1)
	}
}
