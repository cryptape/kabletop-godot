use kabletop_ckb_sdk::{
	config::VARS, p2p::Caller, ckb::{
		rpc::methods as ckb, transaction::{
			builder::build_tx_close_channel, channel::{
				interact as channel, protocol::Args
			}
		}
	}
};
use crate::{
	cache, p2p::protocol::types::{
		request, response, GodotType
	}
};
use serde_json::{
	json, Value, from_value
};
use ckb_types::{
	prelude::*, packed::Transaction, H256
};
use std::{
	thread, time, collections::HashMap, sync::Mutex, convert::TryInto
};
use futures::{
	executor::block_on, future::BoxFuture
};
use ckb_crypto::secp::Signature;
use molecule::prelude::Entity as MolEntity;

fn check_transaction_committed_or_not(hash: &H256) -> bool {
	for _ in 0..20 {
		if ckb::get_transaction(hash.pack()).is_ok() {
			return true;
		}
		thread::sleep(time::Duration::from_secs(5));
	}
	return false;
}

pub mod send {
	use super::*;

	// try to open a state channel between client and server
	pub fn open_kabletop_channel<T: Caller>(caller: &T) -> Result<[u8; 32], String> {
		let store = cache::get_clone();
		if store.user_nfts.len() == 0 {
			return Err(String::from("playing nfts need to be set before"));
		}
		let hashes = VARS
			.luacodes
			.iter()
			.map(|value| value.data_hash.clone())
			.collect::<Vec<_>>();
		let tx = block_on(channel::prepare_channel_tx(
			store.staking_ckb,
			store.bet_ckb,
			store.max_nfts_count,
			store.user_nfts.clone(),
			store.user_pkhash,
			hashes
		)).map_err(|err| format!("prepare_channel_tx -> {}", err))?;
		let value: response::CompleteAndSignChannel = caller.call(
			"prepare_kabletop_channel", request::PrepareChannel {
				tx: tx.into()
			}).map_err(|err| format!("PrepareChannel -> {}", err))?;
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
			store.user_nfts,
			&VARS.common.user_key.privkey
		).map_err(|err| format!("sign_channel_tx -> {}", err))?;

		// write tx to file for debug
        // let json_tx = ckb_jsonrpc_types::TransactionView::from(tx.clone());
        // let json = serde_json::to_string_pretty(&json_tx).expect("jsonify");
        // std::fs::write("open_kabletop_channel.json", json).expect("write json file");

		let hash = ckb::send_transaction(tx.data())
			.map_err(|err| format!("send_transaction -> {}", err))?;
		if !check_transaction_committed_or_not(&hash) {
			return Err(String::from("send_transaction successed, but no committed transaction found in CKB network in 100s"));
		}
		let value: response::OpenChannel = caller.call(
			"open_kabletop_channel", request::SignAndSubmitChannel {
				tx: tx.clone().into()
			}).map_err(|err| format!("SignAndSubmitChannel -> {}", err))?;
		if !value.result {
			return Err(String::from("opposite responsed FAIL for creating kabletop channel"));
		}
		let kabletop = tx.output(0).unwrap();
		cache::set_channel_verification(
			tx.hash().unpack(),
			kabletop.calc_lock_hash().unpack(),
			kabletop.lock().args().raw_data().to_vec(),
			kabletop.capacity().unpack()
		);
		Ok(tx.hash().unpack())
	}

	// try to close a state channel between client and server
	pub fn close_kabletop_channel<T: Caller>(caller: &T) -> Result<[u8; 32], String> {
		let store = cache::get_clone();

		// print all operations in each round for debug
		// println!("test close_kabletop_channel");
		// cache::get_clone().signed_rounds.iter().enumerate().for_each(|(i, (round, _))| {
		// 	println!("round {}", i + 1);
		// 	for i in 0..round.operations().len() {
		// 		let operation = match round.operations().get(i) {
		// 			Some(operation) => String::from_utf8(operation.raw_data().to_vec()).unwrap(),
		// 			None => panic!("bad operaion {}", i)
		// 		};
		// 		println!("{}", operation);
		// 	}
		// });

		let tx = block_on(build_tx_close_channel(
			store.script_args,
			store.channel_hash.clone(),
			store.signed_rounds,
			store.winner,
			false
		)).map_err(|err| format!("build_tx_close_channel -> {}", err))?;

		// write tx to file for debug
        let json_tx = ckb_jsonrpc_types::TransactionView::from(tx.clone());
        let json = serde_json::to_string_pretty(&json_tx).expect("jsonify");
        std::fs::write("close_kabletop_channel.json", json).expect("write json file");

		let hash = ckb::send_transaction(tx.data())
			.map_err(|err| format!("send_transaction -> {}", err))?;
		if !check_transaction_committed_or_not(&hash) {
			return Err(String::from("send_transaction successed, but no committed transaction found in CKB network in 100s"));
		}
		let value: response::CloseChannel = caller.call(
			"close_kabletop_channel", request::CloseChannel {
				tx:           tx.into(),
				channel_hash: store.channel_hash
			}).map_err(|err| format!("CloseChannel -> {}", err))?;
		if !value.result {
			return Err(String::from("opposite REFUSED to apply closing kabeltop channel"));
		}
		Ok(hash.pack().unpack())
	}

	// notify opposite to verify the game wether finished or not
	pub fn notify_game_over<T: Caller>(caller: &T) -> Result<[u8; 65], String> {
		let store = cache::get_clone();
		let value: response::CloseGame = caller.call(
			"notify_game_over", request::CloseGame {
				round:      store.round,
				operations: store.round_operations.clone()
			}).map_err(|err| format!("CloseGame -> {}", err))?;
		if !value.result {
			return Err(String::from("opposite verified the finish state of game FAILED"));
		}
		let signature = Signature::from_slice(value.signature.as_bytes())
			.map_err(|err| format!("into_signature -> {}", err))?;
		let mut signed_rounds = store.signed_rounds;
		signed_rounds.push((channel::make_round(store.user_type, store.round_operations), signature.clone()));
		match channel::check_channel_round(store.script_hash.into(), store.capacity, signed_rounds, store.opponent_pkhash) {
			Ok(true)  => cache::commit_user_round(signature.clone()),
			Ok(false) => return Err(format!("signature not match pkhash {}", hex::encode(store.opponent_pkhash))),
			Err(err)  => return Err(format!("check_channel_round -> {}", err.to_string()))
		}
		Ok(signature.serialize().try_into().unwrap())
	}

	// send operations to verify and make round move forward
	pub fn switch_round<T: Caller>(caller: &T) -> Result<[u8; 65], String> {
		let store = cache::get_clone();
		let value: response::OpenRound = caller.call(
			"switch_round", request::CloseRound {
				round:      store.round,
				operations: store.round_operations.clone()
			}).map_err(|err| format!("CloseRound -> {}", err))?;
		if value.round != store.round {
			return Err(format!("opposite round count({}) mismatched native round count({})", value.round, store.round));
		}
		let signature = Signature::from_slice(value.signature.as_bytes())
			.map_err(|err| format!("into_signature -> {}", err))?;
		let mut signed_rounds = store.signed_rounds;
		signed_rounds.push((channel::make_round(store.user_type, store.round_operations), signature.clone()));
		match channel::check_channel_round(store.script_hash.into(), store.capacity, signed_rounds, store.opponent_pkhash) {
			Ok(true)  => cache::commit_user_round(signature.clone()),
			Ok(false) => return Err(format!("signature not match pkhash {}", hex::encode(store.opponent_pkhash))),
			Err(err)  => return Err(format!("check_channel_round -> {}", err.to_string()))
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
			}).map_err(|err| format!("PushOperation -> {}", err))?;
		if value.result {
			cache::commit_user_operation(operation);
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
			}).map_err(|err| format!("SendP2pMessage -> {}", err))?;
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

	// response operation of openning kabletop channel
	pub fn prepare_kabletop_channel(_: i32, value: Value) -> BoxFuture<'static, Result<Value, String>> {
		Box::pin(async {
			let value: request::PrepareChannel = from_value(value)
				.map_err(|err| format!("deserialize PrepareChannel -> {}", err))?;
			let store = cache::get_clone();
			let hashes = VARS
				.luacodes
				.iter()
				.map(|v| v.data_hash.clone())
				.collect::<Vec<_>>();
			let tx = {
				let tx: Transaction = value.tx.inner.into();
				tx.into_view()
			};
			let lock_args: Vec<u8> = tx.output(0).unwrap().lock().args().unpack();
			let kabletop_args = Args::new_unchecked(lock_args.into());
			cache::set_opponent_pkhash(kabletop_args.user1_pkhash().into());
			cache::set_opponent_nfts(kabletop_args.user1_nfts().into());
			let tx = channel::complete_channel_tx(
				tx.into(),
				store.staking_ckb,
				store.bet_ckb,
				store.max_nfts_count,
				store.user_nfts.clone(),
				store.user_pkhash,
				hashes
			).await.map_err(|err| format!("complete_channel_tx -> {}", err))?;
			let tx = channel::sign_channel_tx(
				tx,
				store.staking_ckb,
				store.bet_ckb,
				store.max_nfts_count,
				store.user_nfts,
				&VARS.common.user_key.privkey
			).map_err(|err| format!("sign_channel_tx -> {}", err))?;
			let kabletop = tx.output(0).unwrap();
			cache::set_channel_verification(
				tx.hash().unpack(),
				kabletop.calc_lock_hash().unpack(),
				kabletop.lock().args().raw_data().to_vec(),
				kabletop.capacity().unpack()
			);
			trigger_hook("prepare_kabletop_channel", tx.data().as_slice().to_vec());
			Ok(json!(response::CompleteAndSignChannel {
				tx: tx.into()
			}))
		})
	}

	// response operation of submitting open_kabletop_channel transaction
	pub fn open_kabletop_channel(_: i32, value: Value) -> BoxFuture<'static, Result<Value, String>> {
		Box::pin(async {
			let value: request::SignAndSubmitChannel = from_value(value)
				.map_err(|err| format!("deserialize open_kabletop_channel -> {}", err))?;
			let hash = value.tx.hash;
			let ok = check_transaction_committed_or_not(&hash);
			trigger_hook("open_kabletop_channel", hash.as_bytes().to_vec());
			Ok(json!(response::OpenChannel {
				result: ok
			}))
		})
	}

	// response operation of submitting close_kabletop_channel transaction
	pub fn close_kabletop_channel(_: i32, value: Value) -> BoxFuture<'static, Result<Value, String>> {
		Box::pin(async {
			let value: request::CloseChannel = from_value(value)
				.map_err(|err| format!("deserialize close_kabletop_channel -> {}", err))?;
			if cache::get_clone().channel_hash != value.channel_hash {
				return Err(String::from("opposite and native kabletop_channel_tx_hash are mismatched"));
			}
			let hash = value.tx.hash;
			let ok = check_transaction_committed_or_not(&hash);
			trigger_hook("close_kabletop_channel", hash.as_bytes().to_vec());
			Ok(json!(response::CloseChannel {
				result: ok
			}))
		})
	}

	// response verification of wether game has finished 
	pub fn notify_game_over(_: i32, value: Value) -> BoxFuture<'static, Result<Value, String>> {
		Box::pin(async {
			let value: request::CloseGame = from_value(value)
				.map_err(|err| format!("deserialize verify_game_over -> {}", err))?;
			let mut store = cache::get_clone();
			if value.round != store.round {
				return Err(format!("opposite round #{} exceeds native round #{}", value.round, store.round));
			} else if value.operations != store.round_operations {
				return Err(String::from("opposite and native operations are mismatched"));
			} else if store.winner == 0 {
				let mut ok = false;
				for _ in 0..5 {
					store = cache::get_clone();
					if store.winner != 0 {
						ok = true;
						break
					}
					thread::sleep(time::Duration::from_secs(1));
				}
				if !ok {
					return Err(String::from("native winner hasn't been set"));
				}
			}
			let next_round = channel::make_round(store.opponent_type, store.round_operations);
			let signature = channel::sign_channel_round(
				store.script_hash.pack(),
				store.capacity,
				store.signed_rounds,
				next_round,
				&VARS.common.user_key.privkey
			).map_err(|err| format!("sign_channel_round -> {}", err))?;
			cache::commit_opponent_round(signature.clone());
			trigger_hook("game_over", vec![store.winner]);
			Ok(json!(response::CloseGame {
				result:    true,
				signature: signature.serialize().pack().into()
			}))
		})
	}

	// response operation of switching kabletop round
	pub fn switch_round(_: i32, value: Value) -> BoxFuture<'static, Result<Value, String>> {
		Box::pin(async {
			let value: request::CloseRound = from_value(value)
				.map_err(|err| format!("deserialize switch_round -> {}", err))?;
			let store = cache::get_clone();
			if value.round != store.round {
				return Err(format!("opposite round #{} exceeds native round #{}", value.round, store.round));
			} else if value.operations != store.round_operations {
				return Err(String::from("opposite and native operations are mismatched"));
			}
			let next_round = channel::make_round(store.opponent_type, store.round_operations);
			let signature = channel::sign_channel_round(
				store.script_hash.pack(),
				store.capacity,
				store.signed_rounds,
				next_round,
				&VARS.common.user_key.privkey
			).map_err(|err| format!("sign_channel_round -> {}", err))?;
			cache::commit_opponent_round(signature.clone());
			trigger_hook("switch_round", signature.serialize());
			Ok(json!(response::OpenRound {
				round:     store.round,
				signature: signature.serialize().pack().into()
			}))
		})
	}

	// accept operations generated from current round
	pub fn sync_operation(_: i32, value: Value) -> BoxFuture<'static, Result<Value, String>> {
		Box::pin(async {
			let value: request::PushOperation = from_value(value)
				.map_err(|err| format!("deserialize PushOperation -> {}", err))?;
			let store = cache::get_clone();
			if value.round != store.round {
				println!("CAUTION: sync_operation => native #{} and opposite #{} round are mismatched", store.round, value.round);
			}
			cache::commit_opponent_operation(value.operation.clone());
			trigger_hook("sync_operation", value.operation.as_bytes().to_vec());
			Ok(json!(response::ApplyOperation {
				result: true
			}))
		})
	}

	// accept user-defined godot message and reply in a same type
	pub fn sync_p2p_message(_: i32, value: Value) -> BoxFuture<'static, Result<Value, String>> {
		Box::pin(async {
			let value: request::SendP2pMessage = from_value(value)
				.map_err(|err| format!("deserialize PushP2pMessage -> {}", err))?;
			let store = cache::GODOT_CACHE.lock().unwrap();
			let mut result = None;
			if let Some(callback) = store.callbacks.get(&value.message) {
				result = Some(callback(value.parameters));
			}
			trigger_hook("sync_p2p_message", value.message.as_bytes().to_vec());
			Ok(json!(response::ReplyP2pMessage {
				message:    value.message,
				parameters: result.unwrap_or(HashMap::new())
			}))
		})
	}
}
