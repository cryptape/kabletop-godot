#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[macro_use]
extern crate lazy_static;

lazy_static! {
	pub static ref USE_GODOT: std::sync::Mutex<bool> = std::sync::Mutex::new(false);
}

#[macro_use]
macro_rules! println {
	($($args:tt)*) => {
		if *crate::USE_GODOT.lock().unwrap() {
			gdnative::godot_print!($($args)*);
		} else {
			std::println!($($args)*);
		}
	};
}

pub mod p2p;
pub mod lua;
pub mod cache;
pub mod ckb;