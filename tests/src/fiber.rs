use xx_core::fiber::{Fiber, Start};
use xx_core::pointer::*;

unsafe extern "C" fn start(arg: Ptr<()>) {
	let mut val = 0;
	let data = arg.cast::<(Fiber, Fiber, i32)>().cast_mut();

	loop {
		data.as_mut().2 += val;
		val += 1;

		Fiber::switch(ptr!(&mut data=>1), ptr!(&mut data=>0));
	}
}

#[test]
fn test_fibers() {
	unsafe {
		let mut data = (Fiber::main(), Fiber::new(), 0i32);
		let data = ptr!(&mut data);

		data.as_mut()
			.1
			.set_start(Start::new(start, data.cast_const().cast()));

		let mut val = 0;

		for i in 0..10 {
			Fiber::switch(ptr!(&mut data=>0), ptr!(&mut data=>1));

			val += i;

			assert_eq!(data.as_ref().2, val);
		}

		data.as_mut().2 = 0;

		Fiber::switch(ptr!(&mut data=>0), ptr!(&mut data=>1));

		assert_eq!(data.as_mut().2, 10);
	}
}
