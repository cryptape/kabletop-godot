use std::{
	os::raw::c_void, ptr, ffi::{
		CStr, CString
	}
};

pub enum lua_State {}
unsafe impl Send for lua_State {}

type lua_KContext = *mut c_void;
type lua_KFunction = unsafe extern "C" fn(state: *mut lua_State, status: i32, ctx: lua_KContext) -> i32;
type lua_CFunction = unsafe extern "C" fn(state: *mut lua_State) -> i32;

extern "C" {
	fn luaL_newstate(_: i64, _: i64) -> *mut lua_State;
	fn luaL_openlibs(L: *mut lua_State);
	fn luaL_loadstring(L: *mut lua_State, s: *const i8) -> i32;
	fn lua_close(L: *mut lua_State);
	fn lua_pcallk(L: *mut lua_State, nargs: i32, nresults: i32, errorfunc: i32, ctx: lua_KContext, k: Option<lua_KFunction>) -> i32;
	fn lua_pushcclosure(L: *mut lua_State, func: lua_CFunction, n: i32);
	fn lua_setglobal(L: *mut lua_State, name: *const i8);
	fn lua_getglobal(L: *mut lua_State, name: *const i8) -> i32;
	fn lua_gettop(L: *mut lua_State) -> i32;
	fn lua_settop(L: *mut lua_State, idx: i32);
	fn lua_rotate(L: *mut lua_State, idx: i32, n: i32);
	fn lua_tolstring(L: *mut lua_State, idx: i32, len: *mut usize) -> *const i8;
	fn lua_tonumberx(L: *mut lua_State, idx: i32, pisnum: *mut i32) -> f64;
	fn lua_tointegerx(L: *mut lua_State, idx: i32, pisnum: *mut i32) -> i64;
	fn lua_pushinteger(L: *mut lua_State, n: i64);
	fn lua_pushstring(L: *mut lua_State, s: *const i8) -> *const i8;
	fn lua_pushvalue(L: *mut lua_State, idx: i32);
	fn lua_pushnil(L: *mut lua_State);
	fn lua_isstring(L: *mut lua_State, idx: i32) -> i32;
	fn lua_type(L: *mut lua_State, idx: i32) -> i32;
	fn lua_rawlen(L: *mut lua_State, idx: i32) -> u64;
	fn lua_rawgeti(L: *mut lua_State, idx: i32, n: i64) -> i32;
	fn lua_createtable(L: *mut lua_State, narray: i32, nrec: i32);
	fn lua_rawseti(L: *mut lua_State, idx: i32, n: i64);
}

const LUA_OK: i32 = 0;
// const LUA_YIELD: i32 = 1;
const LUA_ERRRUN: i32 = 2;
const LUA_ERRSYNTAX: i32 = 3;
const LUA_ERRMEM: i32 = 4;
const LUA_ERRERR: i32 = 5;
const LUA_MULTRET: i32 = -1;
const LUA_TNUMBER: i32 = 3;
const LUA_TSTRING: i32 = 4;
const LUA_TTABLE: i32 = 5;
const LUA_TNIL: i32 = 0;

fn check_ret(ret: i32) {
	match ret {
		LUA_OK        => return,
		LUA_ERRRUN 	  => panic!("lua runtime error"),
		LUA_ERRSYNTAX => panic!("lua syntax error"),
		LUA_ERRMEM    => panic!("lua memory error"),
		LUA_ERRERR    => panic!("lua common error"),
		_             => panic!("unknown error code ({})", ret)
	}
}

macro_rules! cstr {
	($value:expr) => {
		CString::new($value)
			.unwrap()
			.as_ptr()
	};
}

macro_rules! rstr {
	($value:expr) => {
		String::from_utf8(CString::from(CStr::from_ptr($value)).as_bytes().to_vec())
			.unwrap()
	};
}

pub enum lua_Event {
	Number(i64),
	String(String),
	NumberTable(Vec<i64>)
}

pub struct Lua {
	L: *mut lua_State,
	herr: i32
}
unsafe impl Send for Lua {}

impl Lua {
	pub fn new(time: i64, clock: i64) -> Self {
		unsafe {
			let L = luaL_newstate(time, clock);
			luaL_openlibs(L);
			Lua { L, herr: 0 }
		}
	}

	pub fn emplace(L: *mut lua_State) -> Self {
		Lua { L, herr: 0 }
	}

	pub fn close(&self) {
		unsafe { lua_close(self.L); }
	}

	pub fn set_root(&self, root: &str) {
		unsafe {
			lua_pushstring(self.L, cstr!(root));
			lua_setglobal(self.L, cstr!("__root__"));
		}
	}

	pub fn get_root(&self) -> String {
		unsafe {
			let vtype = lua_getglobal(self.L, cstr!("__root__"));
			assert_eq!(vtype, LUA_TSTRING);
			let value = self.to_string(-1);
			lua_settop(self.L, -2);
			value
		}
	}

	pub fn get_events(&self, drop: bool) -> Vec<Vec<lua_Event>> {
		let mut events = vec![];
		unsafe {
			if self.get_global("__events__", false) && lua_type(self.L, -1) == LUA_TTABLE {
				let event_count = lua_rawlen(self.L, -1) as i64;
				for t in 0..event_count {
					if lua_rawgeti(self.L, -1, t + 1) != LUA_TTABLE {
						panic!("event only support TABLE type")
					}
					let mut event = vec![];
					let param_count = lua_rawlen(self.L, -1) as i64;
					for i in 0..param_count {
						match lua_rawgeti(self.L, -1, i + 1) {
							LUA_TNUMBER => event.push(lua_Event::Number(self.to_int64(-1))),
							LUA_TSTRING => event.push(lua_Event::String(self.to_string(-1))),
							LUA_TTABLE  => event.push(lua_Event::NumberTable(self.to_int64_array(-1))),
							_           => panic!("event param ({}) only support INTEGER or STRING or TABLE(i64) type", i + 1)
						}
						lua_settop(self.L, -2); // pop each event params
					}
					lua_settop(self.L, -2); // pop each events
					events.push(event);
				}
				lua_settop(self.L, -2); // pop __events__
				if drop {
					lua_pushnil(self.L);
					lua_setglobal(self.L, cstr!("__events__"))
				}
			}
		}
		events
	}

	pub fn set_global(&self, name: &str, index: i32, drop: bool) {
		unsafe {
			if drop {
				lua_rotate(self.L, index, -1)
			} else {
				lua_pushvalue(self.L, index);
			}
			lua_setglobal(self.L, cstr!(name));
		}
	}

	pub fn get_global(&self, name: &str, read: bool) -> bool {
		unsafe {
			if lua_getglobal(self.L, cstr!(name)) != LUA_TNIL {
				if read {
					lua_settop(self.L, -2);
				}
				true
			} else {
				false
			}
		}
	}

	pub fn remove(&self, index: i32) {
		unsafe {
			lua_rotate(self.L, index, -1);
			lua_settop(self.L, -2);
		}
	}

	pub fn load_string(&self, code: &str) {
		unsafe {
			let ret = luaL_loadstring(self.L, cstr!(code));
			check_ret(ret);
		}
	}

	pub fn pcall(&self, nargs: i32, nresults: i32) {
		unsafe {
			let ret = lua_pcallk(self.L, nargs, nresults, self.herr, ptr::null_mut(), None);
			check_ret(ret);
		}
	}

	pub fn do_string(&self, code: &str) {
		self.load_string(code);
		self.pcall(0, LUA_MULTRET);
	}

	pub fn set_error_func(&mut self, errorfunc: lua_CFunction) {
		unsafe {
			lua_pushcclosure(self.L, errorfunc, 0);
			self.herr = lua_gettop(self.L);
		}
	}

	pub fn register(&self, name: &str, function: lua_CFunction) {
		unsafe {
			lua_pushcclosure(self.L, function, 0);
			lua_setglobal(self.L, cstr!(name))
		}
	}

	pub fn get_top(&self) -> i32 {
		unsafe { lua_gettop(self.L) }
	}

	pub fn to_string(&self, index: i32) -> String {
		unsafe { rstr!(lua_tolstring(self.L, index, ptr::null_mut())) }
	}

	pub fn is_string(&self, index: i32) -> bool {
		unsafe { lua_isstring(self.L, index) == 1 }
	}

	pub fn push_function(&self, function: lua_CFunction) {
		unsafe { lua_pushcclosure(self.L, function, 0) }
	}

	pub fn push_string(&self, value: &str) -> String {
		unsafe { rstr!(lua_pushstring(self.L, cstr!(value))) }
	}
	
	pub fn push_string_array(&self, value: Vec<String>) {
		unsafe {
			lua_createtable(self.L, value.len() as i32, 0);
			for i in 0..value.len() {
				lua_pushstring(self.L, cstr!(value[i].as_str()));
				lua_rawseti(self.L, -2, (i + 1) as i64);
			}
		}
	}

	pub fn push_int64(&self, value: i64) {
		unsafe { lua_pushinteger(self.L, value) }
	}

	pub fn to_int64(&self, index: i32) -> i64 {
		unsafe { lua_tointegerx(self.L, index, ptr::null_mut()) }
	}

	pub fn to_int64_array(&self, index: i32) -> Vec<i64> {
		let mut array = vec![];
		unsafe {
			let count = lua_rawlen(self.L, index) as i64;
			for i in 0..count {
				if lua_rawgeti(self.L, index, i + 1) != LUA_TNUMBER {
					panic!("to_int64_array only support NUMBER object");
				}
				array.push(self.to_int64(-1));
				lua_settop(self.L, -2);
			}
		}
		array
	}

	pub fn to_float64(&self, index: i32) -> f64 {
		unsafe { lua_tonumberx(self.L, index, ptr::null_mut()) }
	}
}
