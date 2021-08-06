#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
// #![allow(unused)]

#[macro_use]
extern crate lazy_static;

pub mod lua;
pub mod ckb;
pub mod cache;

use gdnative::prelude::*;
use lua::highlevel::Lua;
use ckb::client::*;

macro_rules! println {
	($($args:tt)*) => {
		godot_print!($($args)*)
	};
}

#[derive(NativeClass)]
#[inherit(Node)]
struct Kabletop {
	lua:   Option<Lua>,
	entry: String
}

#[gdnative::methods]
impl Kabletop {
    fn new(_owner: &Node) -> Self {
        Kabletop {
			lua:   None,
			entry: String::new()
		}
    }

    #[export]
    fn _ready(&self, _owner: &Node) {
        println!("welcome to the kabletop world!");
    }

	#[export]
	fn set_entry(&mut self, _: &Node, entry: String) {
		self.entry = entry;
	}

	#[export]
	fn create_channel(&mut self, _owner: &Node, socket: String, nfts: Vec<String>) {
		connect(socket.as_str(), || {});
		cache::set_playing_nfts(into_nfts(nfts));
		open_kabletop_channel();

		// create lua vm
		let clone = cache::get_clone();
		let mut ckb_time: i64 = 0;
		for i in 0..8 {
			ckb_time = (ckb_time << 8) | (clone.script_hash[i] as i64 >> 1);
		}
		let mut ckb_clock: i64 = 0;
		for i in 8..16 {
			ckb_clock = (ckb_clock << 8) | (clone.script_hash[i] as i64 >> 1);
		}
		let lua = Lua::new(ckb_time, ckb_clock);
		lua.inject_nfts(from_nfts(clone.user_nfts), from_nfts(clone.opponent_nfts));
		lua.boost(self.entry.clone());
		if let Some(old) = &self.lua {
			old.close();
		}
		self.lua = Some(lua);
	}

	#[export]
	fn run(&self, _owner: &Node, code: String) -> Vec<Vec<Variant>> {
		self.lua
			.as_ref()
			.expect("no kabletop channel is opened")
			.run(code)
			.iter()
			.map(|event| {
				let mut params = vec![];
				for field in event {
					match field {
						lua::ffi::lua_Event::Number(value)      => params.push(value.to_variant()),
						lua::ffi::lua_Event::String(value)      => params.push(value.to_variant()),
						lua::ffi::lua_Event::NumberTable(value) => params.push(value.to_variant())
					}
				}
				params
			})
			.collect::<Vec<Vec<_>>>()
	}
}

fn into_nfts(value: Vec<String>) -> Vec<[u8; 20]> {
	value
		.iter()
		.map(|v| {
			let mut hash = [0u8; 20];
			hash.clone_from_slice(v.as_bytes());
			hash
		})
		.collect::<_>()
}

fn from_nfts(value: Vec<[u8; 20]>) -> Vec<String> {
	value
		.iter()
		.map(|v| {
			String::from_utf8(v.to_vec()).unwrap()
		})
		.collect::<_>()
}

fn init_panic_hook() {
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let loc_string;
        if let Some(location) = panic_info.location() {
            loc_string = format!("file '{}' at line {}", location.file(), location.line());
        } else {
            loc_string = "unknown location".to_owned()
        }

        let error_message;
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            error_message = format!("[RUST] {}: panic occurred: {:?}", loc_string, s);
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            error_message = format!("[RUST] {}: panic occurred: {:?}", loc_string, s);
        } else {
            error_message = format!("[RUST] {}: unknown panic occurred", loc_string);
        }
        godot_error!("{}", error_message);
        (*(old_hook.as_ref()))(panic_info);

        unsafe {
            if let Some(gd_panic_hook) = gdnative::api::utils::autoload::<gdnative::api::Node>("rust_panic_hook") {
                gd_panic_hook.call("rust_panic_hook", &[GodotString::from_str(error_message).to_variant()]);
            }
        }
    }));
}

fn init(handle: InitHandle) {
    handle.add_class::<Kabletop>();
	init_panic_hook();
}

godot_init!(init);
