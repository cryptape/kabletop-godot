use std::{
	io::prelude::*, fs::File, path::PathBuf
};
use super::{
	ffi, inject
};

// a high-level wrapper for ffi::Lua that represents a well-designed version for Kabletop
pub struct Lua {
	lua: ffi::Lua
}

impl Lua {
	// get new Lua instance with native functions set
	pub fn new(time: i64, clock: i64) -> Self {
		let mut lua = ffi::Lua::new(time, clock);
		lua.set_error_func(inject::error);
		lua.register("print", inject::print);
		lua.register("require", inject::require);
        Lua {
			lua
		}
	}

	// close lua instance
	pub fn close(&self) {
		self.lua.close();
	}

	// prepare [_winner, _user1_nfts, _user2_nfts] global variable for lua vm
	pub fn inject_nfts(&self, nfts1: Vec<String>, nfts2: Vec<String>) {
		self.lua.push_int64(0);
		self.lua.set_global("_winner", -1, true);
		if nfts1.iter().any(|nft| nft.len() != 40) 
			|| nfts2.iter().any(|nft| nft.len() != 40) {
			panic!("invalid nft hash");
		}
		self.lua.push_string_array(nfts1);
		self.lua.set_global("_user1_nfts", -1, true);
		self.lua.push_string_array(nfts2);
		self.lua.set_global("_user2_nfts", -1, true);
	}

	// load lua file from disk and init lua vm
	pub fn boost(&self, lua_path: String) {
		let mut root = PathBuf::from(lua_path.clone());
		assert!(root.extension().unwrap() == "lua", "bad file extension");
		root.pop();
		self.lua.set_root(root.to_str().unwrap());
		let mut file = File::open(lua_path).unwrap();
		let mut code: String = String::new();
		file.read_to_string(&mut code).expect("reading lua file");
		self.lua.do_string(code.as_str());
	}

	// run a concrete lua code and collect the events emited from the code for the caller
	pub fn run(&self, lua_code: String) -> Vec<Vec<ffi::lua_Event>> {
		self.lua.do_string(lua_code.as_str());
		self.lua.get_events(true)
	}
}
