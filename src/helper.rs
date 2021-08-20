use gdnative::prelude::*;
use gdnative::api::*;
use kabletop_godot_util::{
	lua::highlevel::Lua, lua, ckb::*
};
use std::{
	sync::Mutex, thread, convert::TryInto
};

lazy_static::lazy_static! {
	pub static ref EMITOR: Mutex<Option<Ref<Node>>>           = Mutex::new(None);
	pub static ref EVENTS: Mutex<Vec<(String, Vec<Variant>)>> = Mutex::new(vec![]);
	pub static ref LUA:    Mutex<Option<Lua>>                 = Mutex::new(None);
	pub static ref NFTS:   Mutex<Option<Variant>>             = Mutex::new(None);
	pub static ref STATUS: Mutex<(u8, bool)>                  = Mutex::new((0, true));
}

pub fn randomseed(seed: &[u8]) {
	let seed = {
		assert!(seed.len() >= 16);
		&seed[..16]
	};
	let seed_1 = i64::from_le_bytes(seed[..8].try_into().unwrap());
	let seed_2 = i64::from_le_bytes(seed[8..].try_into().unwrap());
	run_code(format!("math.randomseed({}, {})", seed_1, seed_2));
}

pub fn run_code(code: String) {
	let events = LUA
		.lock()
		.unwrap()
		.as_ref()
		.expect("no kabletop channel is opened")
		.run(code.clone())
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
		.collect::<Vec<Vec<_>>>();
	if events.len() > 0 {
		push_event("lua_events", vec![events.to_variant()]);
	}
}

pub fn handle_transaction<F>(f: F, callback: Ref<FuncRef>) -> Box<dyn Fn(Result<H256, String>) + 'static + Send> 
where
	F: Fn() + 'static + Send
{
	return Box::new(move |result: Result<H256, String>| {
		match result {
			Ok(hash) => {
				godot_print!("hash = {}", hash);
				unsafe {
					callback.assume_safe().call_func(&[Variant::default()]);
				}
				f();
			},
			Err(err) => {
				unsafe {
					callback.assume_safe().call_func(&[err.to_string().to_variant()]);
				}
			}
		}
	})
}

pub fn update_owned_nfts() {
	thread::spawn(|| {
		let nfts = {
			let nfts = Dictionary::new();
			for (nft, count) in owned_nfts().expect("get owned nfts") {
				nfts.insert(nft, count.to_variant());
			}
			nfts.into_shared()
		};
		*NFTS.lock().unwrap() = Some(nfts.to_variant());
		push_event("owned_nfts_updated", vec![nfts.to_variant()]);
	});
}

pub fn update_box_status() {
	thread::spawn(|| {
		let mut status = (0, true);
		match wallet_status() {
			Ok((count, ready)) => status = (count, ready),
			Err(err)           => godot_print!("{}", err)
		}
		*STATUS.lock().unwrap() = status;
		push_event("box_status_updated", vec![status.0.to_variant(), status.1.to_variant()]);
	});
}

pub fn push_event(name: &str, value: Vec<Variant>) {
	EVENTS
		.lock()
		.unwrap()
		.push((String::from(name), value));
}

pub fn into_nfts(value: Vec<String>) -> Vec<[u8; 20]> {
	value
		.iter()
		.map(|v| {
			let mut hash = [0u8; 20];
			let bytes = hex::decode(v).expect("decode blake160 hashcode");
			hash.clone_from_slice(bytes.as_slice());
			hash
		})
		.collect::<_>()
}

pub fn from_nfts(value: Vec<[u8; 20]>) -> Vec<String> {
	value
		.iter()
		.map(|v| hex::encode(v))
		.collect::<_>()
}

pub fn init_panic_hook() {
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
