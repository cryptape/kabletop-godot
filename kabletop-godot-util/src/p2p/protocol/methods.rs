use kabletop_sdk::{
	config::VARS, p2p::Caller, ckb::{
		rpc::methods as ckb, transaction::channel::{
			interact as channel, protocol::Args
		}
	}
};
use crate::{
	cache, p2p::protocol::{
		request, response, GodotType
	}
};
use serde_json::{
	json, Value, from_value
};
use ckb_types::{
	prelude::*, packed::Transaction
};
use std::{
	thread, time, collections::HashMap, sync::Mutex, convert::TryInto
};
use ckb_crypto::secp::Signature;
use futures::executor::block_on;
use molecule::prelude::Entity as MolEntity;

pub mod send {
	use super::*;

	// request a proposal of making consensus of channel parameters
	pub fn propose_channel_parameter<T: Caller>(caller: &T) -> Result<(), String> {
		let store = cache::get_clone();
		let value: response::ApproveGameParameter = caller.call(
			"propose_channel_parameter", request::ProposeGameParameter {
				staking_ckb: store.staking_ckb,
				bet_ckb:     store.bet_ckb
			}).map_err(|err| format!("ProposeGameParameter -> {}", err.to_string()))?;
		if !value.result {
			return Err(String::from("opposite REFUSED proposing channel paramters"));
		}
		Ok(())
	}

	// try to open a state channel between client and server
	pub fn open_kabletop_channel<T: Caller>(caller: &T) -> Result<[u8; 32], String> {
		let store = cache::get_clone();
		if store.user_nfts.len() == 0 {
			return Err(String::from("playing nfts need to be set before"));
		}
		let hashes = store.luacode_hashes
			.iter()
			.map(|&value| value.pack())
			.collect::<Vec<_>>();
		let tx = block_on(channel::prepare_channel_tx(
			store.staking_ckb,
			store.bet_ckb,
			store.max_nfts_count,
			&store.user_nfts,
			&store.user_pkhash,
			&hashes
		)).map_err(|err| format!("prepare_channel_tx -> {}", err.to_string()))?;
		let value: response::CompleteAndSignChannel = caller.call(
			"prepare_kabletop_channel", request::PrepareChannel {
				tx: tx.into()
			}).map_err(|err| format!("PrepareChannel -> {}", err.to_string()))?;
		let tx = {
			let tx: Transaction = value.tx.inner.into();
			tx.into_view()
		};
		let lock_args: Vec<u8> = tx.output(0).unwrap().lock().args().unpack();
		let kabletop_args = Args::new_unchecked(lock_args.into());
		cache::set_opponent_pkhash(kabletop_args.user2_pkhash().into());
		cache::set_opponent_nfts(kabletop_args.user2_nfts().into());
		let tx = channel::sign_channel_tx(
			tx,
			store.staking_ckb,
			store.bet_ckb,
			store.max_nfts_count,
			&store.user_nfts,
			&VARS.common.user_key.privkey
		).map_err(|err| format!("sign_channel_tx -> {}", err.to_string()))?;
		let hash = ckb::send_transaction(tx.data())
			.map_err(|err| format!("send_transaction -> {}", err.to_string()))?;
		// let mut send_ok = false;
		// for _ in 0..20 {
		// 	if ckb::get_transaction(hash.pack()).is_ok() {
		// 		send_ok = true;
		// 		break;
		// 	}
		// 	thread::sleep(time::Duration::from_secs(5));
		// }
		// if !send_ok {
		// 	return Err(String::from("send_transaction successed, but no committed transaction found in CKB network in 100s"));
		// }
		let value: response::OpenChannel = caller.call(
			"open_kabletop_channel", request::SignAndSubmitChannel {
				tx: tx.clone().into()
			}).map_err(|err| format!("SignAndSubmitChannel -> {}", err.to_string()))?;
		if !value.result {
			return Err(String::from("opposite responsed FAIL for creating kabletop channel"));
		}
		let kabletop = tx.output(0).unwrap();
		cache::set_scripthash_and_capacity(kabletop.calc_lock_hash().unpack(), kabletop.capacity().unpack());
		Ok(tx.hash().unpack())
	}

	// send operations to verify and make round move forward
	pub fn switch_round<T: Caller>(caller: &T) -> Result<[u8; 65], String> {
		let store = cache::get_clone();
		let value: response::OpenRound = caller.call(
			"switch_round", request::CloseRound {
				round:      store.round,
				operations: store.user_operations.clone()
			}).map_err(|err| format!("CloseRound -> {}", err.to_string()))?;
		if value.round != store.round + 1 {
			return Err(format!("opposite round count({}) mismatched native round count({})", value.round, store.round));
		}
		let signature = Signature::from_slice(value.signature.as_bytes())
			.map_err(|err| format!("into_signature -> {}", err.to_string()))?;
		let mut signed_rounds = store.signed_rounds.clone();
		signed_rounds.push((channel::make_round(store.user_type, &store.user_operations), signature.clone()));
		match channel::check_channel_round(&store.script_hash.into(), store.capacity, &signed_rounds, &store.opponent_pkhash) {
			Ok(pass) => {
				if pass {
					cache::commit_user_round(signature.clone());
				} else {
					return Err(format!("signature not match pkhash {}", hex::encode(store.opponent_pkhash)));
				}
			},
			Err(err) => return Err(format!("check_channel_round -> {}", err.to_string()))
		}
		Ok(signature.serialize().try_into().unwrap())
	}

	// synchronize operations in current round
	pub fn sync_operation<T: Caller>(caller: &T, operation: String) -> Result<(), String> {
		let store = cache::get_clone();
		let value: response::ApplyOperation = caller.call(
			"sync_operation", request::PushOperation {
				round:     store.round,
				operation: operation.clone()
			}).map_err(|err| format!("PushOperation -> {}", err.to_string()))?;
		if value.result {
			cache::commit_round_operation(operation);
		} else {
			return Err(String::from("opposite REFUSED applying round operation"));
		}
		Ok(())
	}

	// push user-defined godot message from gdscript, only support bool/i64/f64/string variable types
	pub fn sync_p2p_message<T: Caller>(
		caller: &T, message: String, parameters: HashMap<String, GodotType>
	) -> Result<(String, HashMap<String, GodotType>), String> {
		let value: response::ReplyP2pMessage = caller.call(
			"sync_p2p_message", request::SendP2pMessage {
				message, parameters
			}).map_err(|err| format!("SendP2pMessage -> {}", err.to_string()))?;
		Ok((value.message, value.parameters))
	}
}

pub mod reply {
	use super::*;

	pub mod hook {
		use super::HOOKS;

		pub fn add<F: Fn(&Vec<u8>) + Sync + Send + 'static>(method: &str, hook: F) {
			let mut hooks = HOOKS.lock().unwrap();
			if let Some(hooks) = hooks.get_mut(&String::from(method)) {
				hooks.push(Box::new(hook));
			} else {
				hooks.insert(String::from(method), vec![Box::new(hook)]);
			}
		}
	}

	lazy_static! {
		static ref HOOKS: Mutex<HashMap<String, Vec<Box<dyn Fn(&Vec<u8>) + Sync + Send>>>> = Mutex::new(HashMap::new());
	}

	fn trigger_hook(method: &str, param: Vec<u8>) {
		let method = String::from(method);
		thread::spawn(move || {
			if let Some(hooks) = HOOKS.lock().unwrap().get(&method) {
				for hook in hooks {
					hook(&param);
				}
			}
		});
	}

	// response client's proposal of confirmation of channel parameters
	pub fn propose_channel_parameter(value: Value) -> Result<Value, String> {
		let value: request::ProposeGameParameter = from_value(value)
			.map_err(|err| format!("deserialize ProposeGameParameter -> {}", err.to_string()))?;
		let store = cache::get_clone();
		trigger_hook("propose_channel_parameter", vec![]);
		Ok(json!(response::ApproveGameParameter {
			result: value.staking_ckb == store.staking_ckb && value.bet_ckb == store.bet_ckb
		}))
	}

	// response client's operation of openning kabletop channel
	pub fn prepare_kabletop_channel(value: Value) -> Result<Value, String> {
		let value: request::PrepareChannel = from_value(value)
			.map_err(|err| format!("deserialize PrepareChannel -> {}", err.to_string()))?;
		let store = cache::get_clone();
		let hashes = store.luacode_hashes
			.iter()
			.map(|&v| v.pack())
			.collect::<Vec<_>>();
		let tx = {
			let tx: Transaction = value.tx.inner.into();
			tx.into_view()
		};
		let lock_args: Vec<u8> = tx.output(0).unwrap().lock().args().unpack();
		let kabletop_args = Args::new_unchecked(lock_args.into());
		cache::set_opponent_pkhash(kabletop_args.user1_pkhash().into());
		cache::set_opponent_nfts(kabletop_args.user1_nfts().into());
		let tx = block_on(channel::complete_channel_tx(
			tx.into(),
			store.staking_ckb,
			store.bet_ckb,
			store.max_nfts_count,
			&store.user_nfts,
			&store.user_pkhash,
			&hashes
		)).map_err(|err| format!("complete_channel_tx -> {}", err.to_string()))?;
		let tx = channel::sign_channel_tx(
			tx,
			store.staking_ckb,
			store.bet_ckb,
			store.max_nfts_count,
			&store.user_nfts,
			&VARS.common.user_key.privkey
		).map_err(|err| format!("sign_channel_tx -> {}", err.to_string()))?;
		let kabletop = tx.output(0).unwrap();
		cache::set_scripthash_and_capacity(kabletop.calc_lock_hash().unpack(), kabletop.capacity().unpack());
		trigger_hook("prepare_kabletop_channel", tx.data().as_slice().to_vec());
		Ok(json!(response::CompleteAndSignChannel {
			tx: tx.into()
		}))
	}

	// response client's operation of submitting openning kabletop channel transaction
	pub fn open_kabletop_channel(value: Value) -> Result<Value, String> {
		let value: request::SignAndSubmitChannel = from_value(value)
			.map_err(|err| format!("deserialize open_kabletop_channel -> {}", err.to_string()))?;
		let hash = value.tx.hash;
		// let mut ok = false;
		// for _ in 0..20 {
		// 	if ckb::get_transaction(hash.pack()).is_ok() {
		// 		ok = true;
		// 		break;
		// 	}
		// 	thread::sleep(time::Duration::from_secs(5));
		// }
		trigger_hook("open_kabletop_channel", hash.as_bytes().to_vec());
		Ok(json!(response::OpenChannel {
			result: true //ok
		}))
	}

	// response client's operation of switching kabletop round
	pub fn switch_round(value: Value) -> Result<Value, String> {
		let value: request::CloseRound = from_value(value)
			.map_err(|err| format!("deserialize switch_round -> {}", err.to_string()))?;
		let store = cache::get_clone();
		if value.round != store.round {
			return Err(String::from("opposite round exceeds native round"));
		}
		else if value.operations != store.opponent_operations {
			return Err(String::from("opposite and native operations are mismatched"));
		}
		let next_round = channel::make_round(store.opponent_type, &store.opponent_operations);
		let signature = channel::sign_channel_round(
			store.script_hash.pack(),
			store.capacity,
			&store.signed_rounds,
			&next_round,
			&VARS.common.user_key.privkey
		).map_err(|err| format!("sign_channel_round -> {}", err.to_string()))?;
		cache::commit_opponent_round(signature.clone());
		trigger_hook("switch_round", signature.serialize());
		Ok(json!(response::OpenRound {
			round:     store.round + 1,
			signature: signature.serialize().pack().into()
		}))
	}

	// accept operations generated from current round
	pub fn sync_operation(value: Value) -> Result<Value, String> {
		let value: request::PushOperation = from_value(value)
			.map_err(|err| format!("deserialize PushOperation -> {}", err.to_string()))?;
		let store = cache::get_clone();
		if value.round != store.round {
			return Err(String::from("client and server round are mismatched"));
		}
		cache::commit_round_operation(value.operation.clone());
		trigger_hook("sync_operation", value.operation.as_bytes().to_vec());
		Ok(json!(response::ApplyOperation {
			result: true
		}))
	}

	// accept user-defined godot message and reply in a same type
	pub fn sync_p2p_message(value: Value) -> Result<Value, String> {
		let value: request::SendP2pMessage = from_value(value)
			.map_err(|err| format!("deserialize PushP2pMessage -> {}", err.to_string()))?;
		let store = cache::GODOT_CACHE.lock().unwrap();
		let mut result = None;
		if let Some(callback) = store.callbacks.get(&value.message) {
			result = Some(
				callback(value.message.clone(), value.parameters)
			)
		}
		trigger_hook("sync_p2p_message", value.message.as_bytes().to_vec());
		Ok(json!(response::ReplyP2pMessage {
			message:    value.message,
			parameters: result.unwrap_or(HashMap::new())
		}))
	}
}
