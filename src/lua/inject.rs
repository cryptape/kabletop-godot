use std::{
	io::prelude::*, fs::File, path::PathBuf
};
use super::ffi;

// collect exception message from lua vm
pub unsafe extern "C" fn error(L: *mut ffi::lua_State) -> i32 {
	let lua = ffi::Lua::emplace(L);
	if lua.is_string(-1) {
		println!("Error => {}", lua.to_string(-1));
	}
	return 0;
}

// collect only string or string-castable variables in lua stack and print them
pub unsafe extern "C" fn print(L: *mut ffi::lua_State) -> i32 {
	let lua = ffi::Lua::emplace(L);
	let mut output = String::new();
	for i in 0..lua.get_top() {
		if lua.is_string(i + 1) {
			output += lua.to_string(i + 1).as_str();
		} else {
			output += "_unknown_";
		}
	}
	println!("{}", output);
	return 0;
}

// replace native lua require function which has been removed from castrated lua
pub unsafe extern "C" fn require(L: *mut ffi::lua_State) -> i32 {
	let lua = ffi::Lua::emplace(L);
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
		file.read_to_string(&mut code).expect("reading lua file");
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
