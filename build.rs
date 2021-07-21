fn main() {
	use std::env;

	let target_os = env::var("CARGO_CFG_TARGET_OS");
	let target_family = env::var("CARGO_CFG_TARGET_FAMILY");

	let mut config = cc::Build::new();

	if target_os == Ok("linux".to_string()) {
		config.define("LUA_USE_LINUX", None);
	} else if target_os == Ok("macos".to_string()) {
		config.define("LUA_USE_MACOSX", None);
	} else if target_family == Ok("unix".to_string()) {
		config.define("LUA_USE_POSIX", None);
	} else if target_family == Ok("windows".to_string()) {
		config.define("LUA_USE_WINDOWS", None);
	}

	config
		.include("lua")
		.file("lua/lapi.c")
		.file("lua/lauxlib.c")
		.file("lua/lbaselib.c")
		.file("lua/lcode.c")
		.file("lua/lctype.c")
		.file("lua/ldblib.c")
		.file("lua/ldebug.c")
		.file("lua/ldo.c")
		.file("lua/ldump.c")
		.file("lua/lfunc.c")
		.file("lua/lgc.c")
		.file("lua/linit.c")
		.file("lua/llex.c")
		.file("lua/lmathlib.c")
		.file("lua/lmem.c")
		.file("lua/loadlib.c")
		.file("lua/lobject.c")
		.file("lua/lopcodes.c")
		.file("lua/lparser.c")
		.file("lua/lstate.c")
		.file("lua/lstring.c")
		.file("lua/lstrlib.c")
		.file("lua/ltable.c")
		.file("lua/ltablib.c")
		.file("lua/ltm.c")
		.file("lua/lundump.c")
		.file("lua/lvm.c")
		.file("lua/lzio.c")
		.file("lua/ltime.c")
		.compile("liblua5.4.a");
}