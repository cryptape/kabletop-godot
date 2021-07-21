#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]

use gdnative::prelude::*;
use gdnative::api::*;
use std::io::prelude::*;
use std::fs::File;
use std::path::PathBuf;

mod ffi;
use ffi::Lua;

pub unsafe extern "C" fn print(L: *mut ffi::lua_State) -> i32 {
	let lua = Lua::emplace(L);
	let mut output = String::new();
	for i in 0..lua.get_top() {
		if lua.is_string(i + 1) {
			output += lua.to_string(i + 1).as_str();
		} else {
			output += "_unknown_"
		}
	}
	if lua.get_global("_RUST_TEST", true) {
		println!("{}", output);
	} else {
		godot_print!("{}", output);
	}
	return 0;
}

pub unsafe extern "C" fn require(L: *mut ffi::lua_State) -> i32 {
	let lua = Lua::emplace(L);
	let previous_top = lua.get_top();
	assert!(previous_top > 0, "require: wrong param num");
	let mut path = PathBuf::from(lua.get_root());
	path.push(lua.to_string(-1));
	path.set_extension("lua");
	let mut ret = 0;
	if lua.get_global(path.to_str().unwrap(), false) {
		if !lua.is_string(-1) || lua.to_string(-1) != "empty" {
			ret = 1;
		}
	} else {
		let mut file = File::open(path.clone()).expect("file not found");
		let mut code: String = String::new();
		file.read_to_string(&mut code);
		lua.do_string(code.as_str());
		if lua.get_top() == previous_top {
			ret = 0;
			lua.push_string("empty");
			lua.set_global(path.to_str().unwrap(), -1, true);
		} else {
			ret = 1;
			lua.set_global(path.to_str().unwrap(), -1, false);
		}
	}
	return ret;
}

#[derive(NativeClass)]
#[inherit(Node)]
struct Kabletop {
	lua: Lua
}

#[gdnative::methods]
impl Kabletop {
    fn new(_owner: &Node) -> Self {
		let mut lua = Lua::new(0, 0);
		lua.set_error_func(print);
		lua.register("print", print);
		lua.register("require", require);
        Kabletop {
			lua
		}
    }

    #[export]
    fn _ready(&self, _owner: &Node) {
        godot_print!("welcome to the kabletop world!")
    }

	#[export]
	fn preload_nfts(&self, _owner: &Node, nfts_1: Vec<String>, nfts_2: Vec<String>) {
		self.lua.push_int64(0);
		self.lua.set_global("_winner", -1, true);
		if nfts_1.iter().any(|nft| nft.len() != 40) 
			|| nfts_2.iter().any(|nft| nft.len() != 40) {
			panic!("invalid nft hash");
		}
		self.lua.push_string_array(nfts_1);
		self.lua.set_global("_user1_nfts", -1, true);
		self.lua.push_string_array(nfts_2);
		self.lua.set_global("_user2_nfts", -1, true);
	}

	#[export]
	fn boost(&self, _owner: &Node, entry_path: String) {
		let mut root = PathBuf::from(entry_path.clone());
		assert!(root.extension().unwrap() == "lua", "bad file extension");
		root.pop();
		self.lua.set_root(root.to_str().unwrap());
		let mut file = File::open(entry_path).unwrap();
		let mut code: String = String::new();
		file.read_to_string(&mut code);
		self.lua.do_string(code.as_str());
	}

	#[export]
	fn run(&self, _owner: &Node, code: String) -> Vec<Vec<Variant>> {
		self.lua.do_string(code.as_str());
		self.check_events(_owner)
	}

	fn check_events(&self, _owner: &Node) -> Vec<Vec<Variant>> {
		let mut events = vec![];
		let lua_events = self.lua.get_events(true);
		for lua_params in lua_events {
			let mut params = vec![];
			for param in lua_params {
				match param {
					ffi::lua_Event::Number(value) => params.push(value.to_variant()),
					ffi::lua_Event::String(value) => params.push(value.to_variant())
				}
			}
			events.push(params);
		}
		events
	}
}

pub fn init_panic_hook() {
    // To enable backtrace, you will need the `backtrace` crate to be included in your cargo.toml, or 
    // a version of rust where backtrace is included in the standard library (e.g. Rust nightly as of the date of publishing)
    // use backtrace::Backtrace;
    // use std::backtrace::Backtrace;
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
        // Uncomment the following line if backtrace crate is included as a dependency
        // godot_error!("Backtrace:\n{:?}", Backtrace::new());
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

#[cfg(test)]
mod test {
	use crate::{
		print, require
	};
	use crate::ffi::{
		self, Lua
	};
	use std::fs::File;
	use std::io::prelude::*;

	#[test]
	fn test_lua() {
		let mut lua = Lua::new(0, 0);
		lua.set_error_func(print);
		lua.register("print", print);
		lua.register("require", require);

		// test flag
		lua.push_int64(1);
		lua.set_global("_RUST_TEST", -1, true);

		// init
		lua.push_int64(0);
		lua.set_global("_winner", -1, true);
		lua.push_string_array(vec![
			String::from("b9aaddf96f7f5c742950611835c040af6b7024ad"),
			String::from("b9aaddf96f7f5c742950611835c040af6b7024ad"),
			String::from("b9aaddf96f7f5c742950611835c040af6b7024ad")
		]);
		lua.set_global("_user1_nfts", -1, true);
		lua.push_string_array(vec![
			String::from("97bff01bcad316a4b534ef221bd66da97018df90"),
			String::from("97bff01bcad316a4b534ef221bd66da97018df90"),
			String::from("97bff01bcad316a4b534ef221bd66da97018df90")
		]);
		lua.set_global("_user2_nfts", -1, true);

		// load
		lua.set_root("../lua");
		let mut file = File::open("../lua/boost.lua").unwrap();
		let mut code: String = String::new();
		file.read_to_string(&mut code);
		lua.do_string(code.as_str());

		// run
		let code = "
			game = Tabletop.new(Role.Silent, Role.Cultist, Player.One)
			game:switch_round()
			game:draw_card()
			game:switch_round()
			game:draw_card()
			game:spell_card(1)
		";
		lua.do_string(code);

		// events
		let events = lua.get_events(true);
		for event in events {
			print!("event_params = [");
			for param in event {
				match param {
					ffi::lua_Event::Number(value) => print!("{},", value),
					ffi::lua_Event::String(value) => print!("{},", value)
				}
			}
			print!("]\n")
		}
	}
}
