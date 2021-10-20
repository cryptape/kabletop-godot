use gdnative::prelude::*;
use gdnative::api::*;
use kabletop_godot_sdk::{
	lua::highlevel::Lua, lua, ckb::*, p2p::{
		client, server, protocol::types::GodotType, protocol_relay::types::ClientInfo
	}
};
use std::{
	sync::Mutex, thread, convert::TryInto, collections::HashMap
};

#[derive(PartialEq, Copy, Clone)]
pub enum P2pMode {
	Client, Server, Empty
}

lazy_static::lazy_static! {
	pub static ref EMITOR:   Mutex<Option<Ref<Node>>>                 = Mutex::new(None);
	pub static ref LUAENTRY: Mutex<String>                            = Mutex::new(String::new());
	pub static ref EVENTS:   Mutex<Vec<(String, Vec<Variant>)>>       = Mutex::new(vec![]);
	pub static ref FUNCREFS: Mutex<Vec<(Ref<FuncRef>, Vec<Variant>)>> = Mutex::new(vec![]);
	pub static ref LUA:      Mutex<Option<Lua>>                       = Mutex::new(None);
	pub static ref NFTS:     Mutex<Option<Variant>>                   = Mutex::new(None);
	pub static ref STATUS:   Mutex<Option<(u8, bool)>>                = Mutex::new(None);
	pub static ref P2PMODE:  Mutex<P2pMode>                           = Mutex::new(P2pMode::Empty);
	// pub static ref HOOKREFS: Mutex<HashMap<String, Ref<FuncRef>>>     = Mutex::new(HashMap::new());
	pub static ref DELAIES:  Mutex<HashMap<String, Vec<(f32, Box<dyn Fn() + 'static + Send + Sync>)>>> = Mutex::new(HashMap::new());
}

pub fn set_emitor(godot_node: Ref<Node>) {
	*EMITOR.lock().unwrap() = Some(godot_node);
}

pub fn get_emitor() -> Option<Ref<Node>> {
	*EMITOR.lock().unwrap()
}

pub fn set_lua_entry(entry: String) {
	*LUAENTRY.lock().unwrap() = entry;
}

pub fn get_lua_entry() -> String {
	LUAENTRY.lock().unwrap().clone()
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

pub fn set_lua(lua: Lua) {
	unset_lua();
	*LUA.lock().unwrap() = Some(lua);
}

pub fn unset_lua() {
	if let Some(lua) = LUA.lock().unwrap().as_ref() {
		lua.close();
	}
	*LUA.lock().unwrap() = None;
}

pub fn run_code(code: String) -> bool {
	if let Some(lua) = LUA.lock().unwrap().as_ref() {
		let events = lua
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
		true
	} else {
		false
	}
}

pub fn handle_transaction<F>(f: F, callback: Ref<FuncRef>) -> Box<dyn Fn(Result<H256, String>) + 'static + Send> 
where
	F: Fn() + 'static + Send
{
	return Box::new(move |result: Result<H256, String>| {
		match result {
			Ok(hash) => {
				f();
				FUNCREFS.lock().unwrap().push((callback.clone(), vec![true.to_variant(), hex::encode(hash).to_variant()]));
			},
			Err(err) => {
				// f();
				FUNCREFS.lock().unwrap().push((callback.clone(), vec![false.to_variant(), err.to_string().to_variant()]));
			}
		}
	})
}

pub fn update_owned_nfts() {
	thread::spawn(|| {
		let nfts = {
			let nfts = Dictionary::new();
			match owned_nfts() {
				Ok(owned_nfts) => {
					for (nft, count) in owned_nfts {
						nfts.insert(nft, count.to_variant());
					}
					Some(nfts.into_shared().to_variant())
				},
				Err(err) => {
					godot_print!("update_owned_nfts error: {}", err);
					None
				}
			}
		};
		if *NFTS.lock().unwrap() != nfts {
			*NFTS.lock().unwrap() = nfts.clone();
			DELAIES.lock().unwrap().remove(&String::from("update_owned_nfts"));
			push_event("owned_nfts_updated", vec![nfts.unwrap_or_default()]);
		} else {
			DELAIES.lock().unwrap().insert(String::from("update_owned_nfts"), vec![
				(2.0, Box::new(update_owned_nfts)), (2.0, Box::new(update_owned_nfts)), (2.0, Box::new(update_owned_nfts))
			]);
		}
	});
}

pub fn update_box_status() {
	thread::spawn(|| {
		let mut status = (0, true);
		match wallet_status() {
			Ok((count, ready)) => status = (count, ready),
			Err(err)           => godot_print!("update_box_status error: {}", err)
		}
		if *STATUS.lock().unwrap() != Some(status) {
			*STATUS.lock().unwrap() = Some(status);
			DELAIES.lock().unwrap().remove(&String::from("update_box_status"));
			push_event("box_status_updated", vec![status.0.to_variant(), status.1.to_variant()]);
		} else {
			DELAIES.lock().unwrap().insert(String::from("update_box_status"), vec![
				(2.0, Box::new(update_box_status)), (2.0, Box::new(update_box_status)), (2.0, Box::new(update_box_status))
			]);
		}
	});
}

pub fn process_delay_funcs(delta_sec: f32) {
	if let Ok(mut delaies) = DELAIES.try_lock() {
		for (_, funcs) in &mut *delaies {
			if let Some((delay, func)) = funcs.get_mut(0) {
				*delay -= delta_sec;
				if *delay < 0.0 {
					func();
				}
				drop(funcs.remove(0));
			}
		}
	}
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

pub fn into_dictionary(value: &Vec<String>) -> Dictionary {
	if !value.is_empty() {
		let mut last_nft = value[0].clone();
		let mut count = 0;
		let nfts = Dictionary::new();
		for nft in value {
			if &last_nft == nft {
				count += 1;
			} else {
				nfts.insert(last_nft, count);
				last_nft = nft.clone();
				count = 1;
			}
		}
		nfts.insert(last_nft, count);
		nfts.into_shared()
	} else {
		Dictionary::new_shared()
	}
}

pub fn from_dictionary(value: Dictionary) -> Vec<String> {
	value
		.iter()
		.map(|(nft, count)| vec![nft.to_string(); count.to_u64() as usize])
		.collect::<Vec<_>>()
		.concat()
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

// pub fn add_hook_funcref(hook_name: &str, callback: Ref<FuncRef>) {
// 	HOOKREFS.lock().unwrap().insert(String::from(hook_name), callback);
// }

// pub fn del_hook_funcref(hook_name: &str) {
// 	HOOKREFS.lock().unwrap().remove(&String::from(hook_name));
// }

// pub fn call_hook_funcref(hook_name: &str, params: Vec<Variant>) -> bool {
// 	let hook_name = String::from(hook_name);
// 	let mut refs = HOOKREFS.lock().unwrap();
// 	if let Some(callback) = refs.get(&hook_name) {
// 		unsafe { callback.assume_safe().call_func(params.as_slice()); }
// 		refs.remove(&hook_name);
// 		true
// 	} else {
// 		false
// 	}
// }

pub fn set_mode(mode: P2pMode) {
	*P2PMODE.lock().unwrap() = mode;
}

pub fn get_mode() -> P2pMode {
	*P2PMODE.lock().unwrap()
}

pub fn close_kabletop_channel() -> Result<[u8; 32], String> {
	match *P2PMODE.lock().unwrap() {
		P2pMode::Client => client::close_kabletop_channel(),
		P2pMode::Server => server::close_kabletop_channel(),
		P2pMode::Empty  => Err(String::from("empty mode"))
	}
}

pub fn sync_operation(code: String) -> Result<(), String> {
	match *P2PMODE.lock().unwrap() {
		P2pMode::Client => client::sync_operation(code),
		P2pMode::Server => server::sync_operation(code),
		P2pMode::Empty  => Err(String::from("empty mode"))
	}
}

pub fn switch_round() -> Result<[u8; 65], String> {
	match *P2PMODE.lock().unwrap() {
		P2pMode::Client => client::switch_round(),
		P2pMode::Server => server::switch_round(),
		P2pMode::Empty  => Err(String::from("empty mode"))
	}
}

pub fn sync_p2p_message(
	message: String, parameters: HashMap<String, GodotType>
) -> Result<(String, HashMap<String, GodotType>), String> {
	match *P2PMODE.lock().unwrap() {
		P2pMode::Client => client::sync_p2p_message(message, parameters),
		P2pMode::Server => server::sync_p2p_message(message, parameters),
		P2pMode::Empty  => Err(String::from("empty mode"))
	}
}

pub fn notify_game_over() -> Result<[u8; 65], String> {
	match *P2PMODE.lock().unwrap() {
		P2pMode::Client => client::notify_game_over(),
		P2pMode::Server => server::notify_game_over(),
		P2pMode::Empty  => Err(String::from("empty mode"))
	}
}

pub fn disconnect() -> Result<(), String> {
	match *P2PMODE.lock().unwrap() {
		P2pMode::Client => Ok(client::disconnect()),
		P2pMode::Server => Ok(server::disconnect()),
		P2pMode::Empty  => Err(String::from("empty mode"))
	}
}

pub fn register_client(nickname: String, staking_ckb: u64, bet_ckb: u64) -> Result<(), String> {
	assert!(*P2PMODE.lock().unwrap() == P2pMode::Client, "register_client only available in CLIENT mode");
	client::register_client(nickname, staking_ckb, bet_ckb)
}

pub fn unregister_client() -> Result<(), String> {
	assert!(*P2PMODE.lock().unwrap() == P2pMode::Client, "unregister_client only available in CLIENT mode");
	client::unregister_client()
}

pub fn fetch_clients() -> Result<Vec<ClientInfo>, String> {
	assert!(*P2PMODE.lock().unwrap() == P2pMode::Client, "fetch_clients only available in CLIENT mode");
	client::fetch_clients()
}

pub fn connect_client(partial_id: i32, nickname: String, staking_ckb: u64, bet_ckb: u64) -> Result<(), String> {
	assert!(*P2PMODE.lock().unwrap() == P2pMode::Client, "connect_client only available in CLIENT mode");
	client::connect_client(partial_id, nickname, staking_ckb, bet_ckb)
}

pub fn disconnect_client() -> Result<(), String> {
	assert!(*P2PMODE.lock().unwrap() == P2pMode::Client, "disconnect_client only available in CLIENT mode");
	client::disconnect_client()
}
