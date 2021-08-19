use kabletop_sdk::{
	config::VARS, p2p::Caller, ckb::{
		rpc::methods as ckb, transaction::channel::{
			interact as channel, protocol::Args
		}
	}
};
use crate::{
	cache, p2p::protocol::{
		request, response
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
	pub fn propose_channel_parameter<T: Caller>(caller: &T) -> bool {
		let clone = cache::get_clone();
		let value: response::ApproveGameParameter = caller.call(
			"propose_channel_parameter", request::ProposeGameParameter {
				staking_ckb: clone.staking_ckb,
				bet_ckb:     clone.bet_ckb
			}).expect("ProposeGameParameter call");
		value.result
	}

	// try to open a state channel between client and server
	pub fn open_kabletop_channel<T: Caller>(caller: &T) -> [u8; 32] {
		let clone = cache::get_clone();
		assert!(clone.user_nfts.len() > 0, "playing nfts need to be set before");
		let hashes = clone.luacode_hashes
			.iter()
			.map(|&value| value.pack())
			.collect::<Vec<_>>();
		let tx = block_on(channel::prepare_channel_tx(
			clone.staking_ckb,
			clone.bet_ckb,
			clone.max_nfts_count,
			&clone.user_nfts,
			&clone.user_pkhash,
			&hashes
		)).expect("prepare_channel_tx");
		let value: response::CompleteAndSignChannel = caller.call(
			"prepare_kabletop_channel", request::PrepareChannel {
				tx: tx.into()
			}).expect("PrepareChannel call");
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
			clone.staking_ckb,
			clone.bet_ckb,
			clone.max_nfts_count,
			&clone.user_nfts,
			&VARS.common.user_key.privkey
		).expect("sign_channel_tx");
		// let hash = ckb::send_transaction(tx.data()).expect("send_transaction");
		// let mut send_ok = false;
		// for _ in 0..10 {
		// 	if ckb::get_transaction(hash.pack()).is_ok() {
		// 		send_ok = true;
		// 		break;
		// 	}
		// 	thread::sleep(time::Duration::from_secs(3));
		// }
		// assert!(send_ok, "send_transaction failed");
		let value: response::OpenChannel = caller.call(
			"open_kabletop_channel", request::SignAndSubmitChannel {
				tx: tx.clone().into()
			}).expect("SignAndSubmitChannel call");
		assert_eq!(value.result, true);
		let kabletop = tx.output(0).unwrap();
		cache::set_scripthash_and_capacity(kabletop.calc_lock_hash().unpack(), kabletop.capacity().unpack());
		tx.hash().unpack()
	}

	// send operations to verify and make round move forward
	pub fn switch_round<T: Caller>(caller: &T) -> [u8; 65] {
		let mut clone = cache::get_clone();
		let value: response::OpenRound = caller.call(
			"switch_round", request::CloseRound {
				round:      clone.round,
				operations: clone.user_operations.clone()
			}).expect("OpenRound call");
		assert!(value.round == clone.round + 1);
		let signature = Signature::from_slice(value.signature.as_bytes()).expect("signature jsonbytes");
		clone.signed_rounds.push((channel::make_round(clone.user_type, &clone.user_operations), signature.clone()));
		match channel::check_channel_round(&clone.script_hash.into(), clone.capacity, &clone.signed_rounds, &clone.opponent_pkhash) {
			Ok(value) => {
				if value {
					cache::commit_user_round(signature.clone());
				} else {
					panic!("signature not match pkhash {}", hex::encode(clone.opponent_pkhash));
				}
			},
			Err(err) => Err(err).unwrap()
		}
		signature.serialize().try_into().unwrap()
	}

	// synchronize operations in current round
	pub fn sync_operation<T: Caller>(caller: &T, operation: String) -> bool {
		let clone = cache::get_clone();
		let value: response::ApplyOperation = caller.call(
			"sync_operation", request::PushOperation {
				round:     clone.round,
				operation: operation.clone()
			}).expect("PushOperation call");
		if value.result {
			cache::commit_round_operation(operation);
		}
		value.result
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
		let value: request::ProposeGameParameter = from_value(value).expect("deserialize ProposeGameParameter");
		let clone = cache::get_clone();
		trigger_hook("propose_channel_parameter", vec![]);
		Ok(json!(response::ApproveGameParameter {
			result: value.staking_ckb == clone.staking_ckb && value.bet_ckb == clone.bet_ckb
		}))
	}

	// response client's operation of openning kabletop channel
	pub fn prepare_kabletop_channel(value: Value) -> Result<Value, String> {
		let value: request::PrepareChannel = from_value(value).expect("deserialize prepare_kabletop_channel");
		let clone = cache::get_clone();
		let hashes = clone.luacode_hashes
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
			clone.staking_ckb,
			clone.bet_ckb,
			clone.max_nfts_count,
			&clone.user_nfts,
			&clone.user_pkhash,
			&hashes
		)).expect("complete channel transaction");
		let tx = channel::sign_channel_tx(
			tx,
			clone.staking_ckb,
			clone.bet_ckb,
			clone.max_nfts_count,
			&clone.user_nfts,
			&VARS.common.user_key.privkey
		).expect("sign channel transaction");
		let kabletop = tx.output(0).unwrap();
		cache::set_scripthash_and_capacity(kabletop.calc_lock_hash().unpack(), kabletop.capacity().unpack());
		trigger_hook("prepare_kabletop_channel", tx.data().as_slice().to_vec());
		Ok(json!(response::CompleteAndSignChannel {
			tx: tx.into()
		}))
	}

	// response client's operation of submitting openning kabletop channel transaction
	pub fn open_kabletop_channel(value: Value) -> Result<Value, String> {
		let value: request::SignAndSubmitChannel = from_value(value).expect("deserialize open_kabletop_channel");
		let hash = value.tx.hash;
		// let mut ok = false;
		// for _ in 0..10 {
		// 	if ckb::get_transaction(hash.pack()).is_ok() {
		// 		ok = true;
		// 		break;
		// 	}
		// 	thread::sleep(time::Duration::from_secs(3));
		// }
		trigger_hook("open_kabletop_channel", hash.as_bytes().to_vec());
		Ok(json!(response::OpenChannel {
			result: true //ok
		}))
	}

	// response client's operation of switching kabletop round
	pub fn switch_round(value: Value) -> Result<Value, String> {
		let value: request::CloseRound = from_value(value).expect("deserialize switch_round");
		let clone = cache::get_clone();
		if value.round != clone.round {
			return Err(String::from("client round exceeds server round"));
		}
		else if value.operations != clone.opponent_operations {
			return Err(String::from("client and server operations are mismatched"));
		}
		let next_round = channel::make_round(clone.opponent_type, &clone.opponent_operations);
		let signature = channel::sign_channel_round(
			clone.script_hash.pack(),
			clone.capacity,
			&clone.signed_rounds,
			&next_round,
			&VARS.common.user_key.privkey
		).expect("sign round");
		cache::commit_opponent_round(signature.clone());
		trigger_hook("switch_round", signature.serialize());
		Ok(json!(response::OpenRound {
			round:     clone.round + 1,
			signature: signature.serialize().pack().into()
		}))
	}

	// accept operations generated from current round
	pub fn sync_operation(value: Value) -> Result<Value, String> {
		let value: request::PushOperation = from_value(value).expect("deserialize PushOperation");
		let clone = cache::get_clone();
		if value.round != clone.round {
			return Err(String::from("client and server round are mismatched"));
		}
		cache::commit_round_operation(value.operation.clone());
		trigger_hook("sync_operation", value.operation.as_bytes().to_vec());
		Ok(json!(response::ApplyOperation {
			result: true
		}))
	}
}
