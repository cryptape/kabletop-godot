#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
// #![allow(unused)]

#[macro_use]
extern crate lazy_static;

lazy_static! {
	pub static ref USE_GODOT: std::sync::Mutex<bool> = std::sync::Mutex::new(true);
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

mod kabletop;

pub mod lua;
pub mod ckb;
pub mod cache;