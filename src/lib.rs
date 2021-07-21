#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]

use gdnative::prelude::*;
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
#[register_with(Self::register_signals)]
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

	fn register_signals(builder: &ClassBuilder<Self>) {
        builder.add_signal(Signal {
            name: "lua_event",
            args: &[
            	SignalArgument {
					name: "event_params",
					default: Variant::default(),
					export_info: ExportInfo::new(VariantType::VariantArray),
					usage: PropertyUsage::DEFAULT,
				}
			]
        });
    }

    #[export]
    fn _ready(&self, _owner: &Node) {
        godot_print!("welcome to the kabletop world!")
    }

	#[export]
	fn init(&self, _owner: &Node, nfts_1: Vec<String>, nfts_2: Vec<String>) {
		self.lua.push_int64(0);
		self.lua.set_global("_winner", -1, true);
		if nfts_1.iter().any(|nft| nft.len() != 20) 
			|| nfts_2.iter().any(|nft| nft.len() != 20) {
			panic!("invalid nft hash");
		}
		self.lua.push_string_array(nfts_1);
		self.lua.set_global("_user1_nfts", -1, true);
		self.lua.push_string_array(nfts_2);
		self.lua.set_global("_user2_nfts", -1, true);
	}

	#[export]
	fn load(&self, _owner: &Node, path: String) {
		let mut root = PathBuf::from(path.clone());
		assert!(root.extension().unwrap() == "lua", "bad file extension");
		root.pop();
		self.lua.set_root(root.to_str().unwrap());
		let mut file = File::open(path).unwrap();
		let mut code: String = String::new();
		file.read_to_string(&mut code);
		self.lua.do_string(code.as_str());
		self.check_events(_owner);
	}

	#[export]
	fn run(&self, _owner: &Node, code: String) {
		self.lua.do_string(code.as_str());
		self.check_events(_owner);
	}

	fn check_events(&self, _owner: &Node) {
		let events = self.lua.get_events(true);
		for event in events {
			let mut event_params = vec![];
			for param in event {
				match param {
					ffi::lua_Event::Number(value) => event_params.push(value.to_variant()),
					ffi::lua_Event::String(value) => event_params.push(value.to_variant())
				}
			}
			_owner.emit_signal("lua_event", &event_params);
		}
	}
}

fn init(handle: InitHandle) {
    handle.add_class::<Kabletop>();
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
			game:draw_card(Player.One)
			game:advance_round(Player.One)
			game:draw_card(Player.Two)
			game:advance_round(Player.Two)
			game:draw_card(Player.One)
			game:spell_card(Player.One, 1)
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
