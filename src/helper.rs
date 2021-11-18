use gdnative::prelude::*;
use gdnative::api::*;
use futures::executor::block_on;
use molecule::prelude::Entity;
use ckb_crypto::secp::Signature;
use kabletop_godot_sdk::{
	lua::highlevel::Lua, cache, lua, ckb::*, p2p::{
		client, server, protocol::types::GodotType, protocol_relay::types::ClientInfo
	}
};
use kabletop_ckb_sdk::{
	config::VARS, ckb::{
		rpc::methods::get_tip_block_number, transaction::{
			helper::*, channel::{
				interact as channel, protocol::{
					Challenge, Round
				}
			}
		} 
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
	pub static ref CODES:    Mutex<Vec<(String, bool)>>               = Mutex::new(vec![]);
	pub static ref LUA:      Mutex<Option<Lua>>                       = Mutex::new(None);
	pub static ref NFTS:     Mutex<Option<Variant>>                   = Mutex::new(None);
	pub static ref STATUS:   Mutex<Option<(u8, bool)>>                = Mutex::new(None);
	pub static ref P2PMODE:  Mutex<P2pMode>                           = Mutex::new(P2pMode::Empty);
	pub static ref DELAIES:  Mutex<HashMap<String, Vec<(f32, Box<dyn Fn() + 'static + Send + Sync>)>>> = Mutex::new(HashMap::new());
}

pub fn set_godot_emitor(godot_node: Ref<Node>) {
	*EMITOR.lock().unwrap() = Some(godot_node);
}

pub fn get_godot_emitor() -> Option<Ref<Node>> {
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
	run_code(format!("math.randomseed({}, {})", seed_1, seed_2), false);
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

pub fn set_p2p_mode(mode: P2pMode) {
	*P2PMODE.lock().unwrap() = mode;
}

pub fn get_p2p_mode() -> P2pMode {
	*P2PMODE.lock().unwrap()
}

pub fn run_code(code: String, emit: bool) -> bool {
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
						lua::ffi::lua_Event::NumberTable(value) => params.push(value.to_variant()),
						lua::ffi::lua_Event::StringTable(value) => params.push(value.to_variant())
					}
				}
				params
			})
			.collect::<Vec<Vec<_>>>();
		if events.len() > 0 && emit {
			push_event("lua_events", vec![events.to_variant()]);
		}
		true
	} else {
		false
	}
}

pub fn dump_cached_codes(from_sync: bool) -> Vec<String> {
	if from_sync {
		let mut codes = vec![];
		for (code, commited) in &mut *CODES.lock().unwrap() {
			if commited == &mut false {
				codes.push(code.clone());
				*commited = true;
			}
		}
		codes
	} else {
		CODES.lock().unwrap().iter().map(|(code, _)| code.clone()).collect::<Vec<_>>()
	}
}

pub fn remove_cached_codes() {
	let codes = CODES
		.lock()
		.unwrap()
		.iter()
		.filter_map(|(code, commited)| {
			if !commited {
				Some((code.clone(), false))
			} else {
				None
			}
		})
		.collect::<Vec<_>>();
	*CODES.lock().unwrap() = codes;
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

pub fn persist_kabletop_cache() {
	let filename = hex::encode(cache::get_clone().script_hash);
	cache::persist(filename).expect("persist");
}

pub fn remove_kabletop_cache(script_hash: String) -> bool {
	let filename = format!("db/{}.json", script_hash);
	if let Ok(_) = std::fs::remove_file(filename) {
		true
	} else {
		println!("file {}.json not found", script_hash);
		false
	}
}

pub fn complete_signed_rounds_for_challenge() -> Result<(Vec<(Round, Signature)>, bool), String> {
	let store = cache::get_clone();
	match block_on(get_kabletop_challenge_data(store.script_args.clone())) {
		Ok((true, data)) => {
			let mut challenging = false;
			if let Some(data) = data {
				if u8::from(data.challenger()) == store.user_type {
					return Err(String::from("dumplicate challenge"))
				}
				// the signature which matches challenger operations of last challenge transaction can be
				// found in current challenge data, so extract it and complete the rounds data
				if !store.round_operations.is_empty() {
					cache::commit_user_round(data.snapshot_signature().into());
				}
				// make new round for pending operations of user who had been challenged
				let opponent_operations = Vec::from(data.operations())
					.into_iter()
					.map(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string()))
					.collect::<Result<Vec<_>, _>>()?
					.into_iter()
					.map(|operation| {
						cache::commit_opponent_operation(operation.clone());
						operation
					})
					.collect::<Vec<_>>();
				if !opponent_operations.is_empty() {
					let opponent_round = channel::make_round(store.opponent_type, opponent_operations);
					let script_hash = kabletop_script(store.script_args).calc_script_hash();
					let signature = channel::sign_channel_round(
						script_hash, cache::get_kabletop_signed_rounds()?, opponent_round, &VARS.common.user_key.privkey
					);
					if let Err(error) = signature {
						return Err(error.to_string())
					}
					cache::commit_opponent_round(signature.unwrap());
				}
				challenging = true;
			}
			Ok((cache::get_kabletop_signed_rounds()?, challenging))
		},
		Ok((false, _)) => Err(String::from("invalid script_args, can't find valid kabletop cell")),
		Err(err)       => Err(err)
	}
}

pub fn scan_uncomplete_kabletop_cache() -> Result<Vec<Dictionary>, String> {
	let db = std::fs::read_dir("db").map_err(|err| err.to_string())?;
	let tipnumber = get_tip_block_number().map_err(|err| err.to_string())?;
	let mut values = vec![];
	for path in db {
		let script_hash = {
			let filename = path.map_err(|err| err.to_string())?.file_name();
			let filename = String::from(filename.to_str().unwrap());
			String::from(filename.strip_suffix(".json").unwrap())
		};
		let store = cache::recover(script_hash.clone())?;
		if let Ok(hash) = hex::decode(script_hash.clone()) {
			if hash[..] != store.script_hash[..] {
				println!("skip unmatched cache file {}.json", script_hash);
				continue
			}
		} else {
			println!("skip invalid cache file {}.json", script_hash);
			continue
		}
		let lock_args = cache::get_kabletop_args()?;
		let signed_rounds = cache::get_kabletop_signed_rounds()?;
		let user1_pkhash: [u8; 20] = lock_args.user1_pkhash().into();
		let user2_pkhash: [u8; 20] = lock_args.user2_pkhash().into();
		let owner_pkhash = privkey_to_pkhash(&VARS.common.user_key.privkey);
		let challenge = Dictionary::new();
		if owner_pkhash[..] == user1_pkhash[..] || owner_pkhash[..] == user2_pkhash[..] {
			let user1_nfts = Vec::from(lock_args.user1_nfts()).iter().map(|nft| hex::encode(nft)).collect::<Vec<_>>();
			let user2_nfts = Vec::from(lock_args.user2_nfts()).iter().map(|nft| hex::encode(nft)).collect::<Vec<_>>();
			challenge.insert("user1_pkhash", hex::encode(user1_pkhash));
			challenge.insert("user2_pkhash", hex::encode(user2_pkhash));
			challenge.insert("user1_nfts", user1_nfts);
			challenge.insert("user2_nfts", user2_nfts);
			if owner_pkhash[..] == user1_pkhash[..] {
				challenge.insert("user_type", 1);
			} else {
				challenge.insert("user_type", 2);
			}
		} else {
			continue
		}
		let round_count = {
			if !store.round_operations.is_empty() {
				signed_rounds.len() + 1
			} else {
				signed_rounds.len()
			}
		};
		let blocknumber: u64 = lock_args.begin_blocknumber().into();
		let countdown = {
			let mut round_count = round_count as u64;
			if signed_rounds.len() > 30 {
				round_count = 30;
			} else if signed_rounds.len() < 5 {
				round_count = 5;
			}
			let targetnumber = blocknumber + round_count * round_count;
			std::cmp::max(targetnumber, tipnumber) - tipnumber
		};
		challenge.insert("script_hash", hex::encode(store.script_hash));
		challenge.insert("staking_ckb", store.staking_ckb / 100_000_000);
		challenge.insert("bet_ckb", store.bet_ckb / 100_000_000);
		challenge.insert("round_count", round_count);
		challenge.insert("block_countdown", countdown);
		values.push((challenge, get_kabletop_challenge_data(lock_args.as_slice().to_vec())));
	}
	let values = values
		.into_iter()
		.map(|(value, check)| block_on(async move { check.await }).map(|challenge| {
			if let (true, challenge) = challenge {
				let challenge = match challenge {
					Some(value) => value,
					None        => Challenge::default()
				};
				let challenger: u8 = challenge.challenger().into();
				let operations: Vec<String> = {
					let operations: Vec<Vec<u8>> = challenge.operations().into();
					match operations.into_iter().map(|v| String::from_utf8(v)).collect::<Result<Vec<_>, _>>() {
						Ok(value) => value,
						Err(_)    => return None
					}
				};
				if value.get("user_type").to_u64() != challenger as u64 {
					let round_count = value.get("round_count").to_u64();
					value.insert("round_count", round_count + 1);
				}
				value.insert("operations", operations);
				value.insert("challenger", challenger);
			} else {
				remove_kabletop_cache(value.get("script_hash").to_string());
				return None
			}
			Some(value.into_shared())
		}))
		.collect::<Result<Vec<_>, _>>()?
		.into_iter()
		.filter_map(|value| value)
		.collect::<Vec<_>>();
	Ok(values)
}

pub fn replay_kabletop_cache(script_hash: String) -> Result<(), String> {
	let hash = hex::decode(script_hash.clone());
	if let Err(error) = hash {
		return Err(error.to_string())
	}
	let store = cache::recover(script_hash)?;
	let lock_args = cache::get_kabletop_args()?;
	let signed_rounds = {
		let mut cached_rounds = cache::get_kabletop_signed_rounds()?;
		if !store.round_operations.is_empty() {
			if let (true, Some(data)) = block_on(get_kabletop_challenge_data(lock_args.as_slice().to_vec()))? {
				if u8::from(data.challenger()) != store.user_type {
					let uncomplete_round = channel::make_round(store.user_type, store.round_operations);
					cached_rounds.push((uncomplete_round, data.snapshot_signature().into()));
				}
			}
		}
		cached_rounds
	};
	let lua = Lua::new(0, 0);
	lua.inject_nfts(from_nfts(lock_args.user1_nfts().into()), from_nfts(lock_args.user2_nfts().into()));
	lua.boost(get_lua_entry());
	set_lua(lua);
	randomseed(hash.unwrap().as_slice());
	for (round, signature) in signed_rounds {
		let operations: Vec<String> = {
			let operations: Vec<Vec<u8>> = round.operations().into();
			match operations.into_iter().map(|v| String::from_utf8(v)).collect::<Result<Vec<_>, _>>() {
				Ok(value)  => value,
				Err(error) => return Err(error.to_string())
			}
		};
		for code in operations {
			run_code(code, false);
		}
		randomseed(signature.serialize().as_slice());
	}
	let user1_pkhash: [u8; 20] = lock_args.user1_pkhash().into();
	let user2_pkhash: [u8; 20] = lock_args.user2_pkhash().into();
	let owner_pkhash = privkey_to_pkhash(&VARS.common.user_key.privkey);
	if (owner_pkhash[..] == user1_pkhash[..] && store.user_type != 1)
		|| (owner_pkhash[..] == user2_pkhash[..] && store.user_type != 2)
		|| (owner_pkhash[..] != user1_pkhash[..] && owner_pkhash[..] != user2_pkhash[..]) {
		return Err(String::from("owner pkhash dosen't match both of two users"))
	}
	Ok(())
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
