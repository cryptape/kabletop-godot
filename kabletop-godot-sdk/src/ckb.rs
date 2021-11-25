use kabletop_ckb_sdk::ckb::{
	wallet::keystore, rpc::{
		types::{
			SearchKey, ScriptType
		}, methods::{
			send_transaction, get_transaction, get_live_nfts, get_live_cells
		}
	}, transaction::{
		builder::*, helper::*, channel::protocol::{
			Round, Challenge
		}
	}
};
use futures::{
	executor::block_on, future::BoxFuture
};
use ckb_types::{
	core::TransactionView, prelude::Pack
};
use std::{
	thread, time::Duration, collections::HashMap
};
use molecule::prelude::Entity;
use ckb_crypto::secp::Signature;
pub use ckb_types::H256;

// get user owned nfts by their lock_script (from user_pkhash) and type_script (from composer_pkhash)
pub fn owned_nfts() -> Result<HashMap<String, u32>, String> {
    let lock_script = sighash_script(&keystore::USER_PUBHASH.to_vec());
    let type_script = {
        let wallet = wallet_script(keystore::COMPOSER_PUBHASH.to_vec());
        nft_script(wallet.calc_script_hash().raw_data().to_vec())
    };
	let nfts = block_on(get_live_nfts(lock_script, Some(type_script), 20))
		.map_err(|e| e.to_string())?
		.iter()
		.map(|(hash, &value)| {
			(hex::encode(hash), value)
		})
		.collect::<HashMap<_, _>>();
	Ok(nfts)
}

// get user wallet cell status, including existence and payment or reveal status
pub fn wallet_status() -> Result<(u8, bool), String> {
    let wallet_script = wallet_script(keystore::COMPOSER_PUBHASH.to_vec());
    let user_payment_script = payment_script(keystore::USER_PUBHASH.to_vec());
    let search_key = SearchKey::new(wallet_script.into(), ScriptType::Lock)
        .filter(user_payment_script.into());
    let wallet_cell = block_on(get_live_cells(search_key, 1, None))
		.map_err(|e| e.to_string())?
		.objects;
    if wallet_cell.is_empty() {
        Ok((0, false))
    } else {
		match wallet_cell[0].output_data.first() {
			Some(&count) => Ok((count, true)),
			None         => Ok((0, true))
		}
	}
}

// push transaction to ckb network through rpc handler
fn push_transaction(tx: TransactionView) -> Result<H256, String> {
	let error: String;
	match send_transaction(tx.data()) {
		Ok(hash) => {
			for _ in 0..20 {
				if get_transaction(hash.pack()).is_ok() {
					return Ok(hash)
				}
				thread::sleep(Duration::from_secs(5));
			}
			error = format!("transaction ({}) confirmation timeout", hash);
		},
		Err(err) => error = err.to_string()
	}
	Err(error)
}

// remove selected nfts from user's nft cells
pub fn discard_nfts<F>(nfts: Vec<String>, f: F)
where 
	F: Fn(Result<H256, String>) + Send + 'static
{
	let nfts = nfts
		.iter()
		.map(|nft| blake160_to_byte20(nft.as_str()).map_err(|e| e.to_string()))
		.collect::<Result<Vec<_>, _>>();
	match nfts {
		Ok(nfts) => {
			thread::spawn(move || {
				match block_on(build_tx_discard_nft(nfts)) {
					Ok(tx) => {
						match push_transaction(tx) {
							Ok(hash) => f(Ok(hash)),
							Err(err) => f(Err(err.to_string()))
						}
					},
					Err(err) => f(Err(err.to_string()))
				}
			});
		},
		Err(err) => f(Err(err))
	}
}

// transfer selected nfts to target address
pub fn transfer_nfts<F>(nfts: Vec<String>, to: String, f: F)
where 
	F: Fn(Result<H256, String>) + Send + 'static
{
	let nfts = nfts
		.iter()
		.map(|nft| blake160_to_byte20(nft.as_str()).map_err(|e| e.to_string()))
		.collect::<Result<Vec<_>, _>>();
	match nfts {
		Ok(nfts) => {
			let to = match blake160_to_byte20(to.as_str()) {
				Ok(pkhash) => pkhash,
				Err(err)   => return f(Err(err.to_string()))
			};
			thread::spawn(move || {
				match block_on(build_tx_transfer_nft(nfts, to)) {
					Ok(tx) => {
						match push_transaction(tx) {
							Ok(hash) => f(Ok(hash)),
							Err(err) => f(Err(err.to_string()))
						}
					},
					Err(err) => f(Err(err.to_string()))
				}
			});
		},
		Err(err) => f(Err(err))
	}
}

// issue nfts to target address for TEST
pub fn issue_nfts<F>(nfts: Vec<String>, f: F)
where 
	F: Fn(Result<H256, String>) + Send + 'static
{
	let nfts = nfts
		.iter()
		.map(|nft| blake160_to_byte20(nft.as_str()).map_err(|e| e.to_string()))
		.collect::<Result<Vec<_>, _>>();
	match nfts {
		Ok(nfts) => {
			thread::spawn(move || {
				match block_on(build_tx_issue_nft(nfts, keystore::USER_PUBHASH.clone())) {
					Ok(tx) => {
						match push_transaction(tx) {
							Ok(hash) => f(Ok(hash)),
							Err(err) => f(Err(err.to_string()))
						}
					},
					Err(err) => f(Err(err.to_string()))
				}
			});
		},
		Err(err) => f(Err(err))
	}
}

// buy nfts from user's wallet cell which is on purchase mode
pub fn purchase_nfts<F>(count: u8, f: F)
where
	F: Fn(Result<H256, String>) + Send + 'static
{
	thread::spawn(move || {
		match block_on(build_tx_purchase_nft_package(count)) {
			Ok(tx) => {
				match push_transaction(tx) {
					Ok(hash) => f(Ok(hash)),
					Err(err) => f(Err(err.to_string()))
				}
			},
			Err(err) => f(Err(err.to_string()))
		}
	});
}

// reveal bought nfts from user's wallet cell which is on reveal mode
pub fn reveal_nfts<F>(f: F)
where
	F: Fn(Result<H256, String>) + Send + 'static
{
	thread::spawn(move || {
		match block_on(build_tx_reveal_nft_package()) {
			Ok(tx) => {
				match push_transaction(tx) {
					Ok(hash) => f(Ok(hash)),
					Err(err) => f(Err(err.to_string()))
				}
			},
			Err(err) => f(Err(err.to_string()))
		}
	});
}

// create nft store to enable purchasing nfts in which user has no nft wallet
pub fn create_wallet<F>(f: F)
where
	F: Fn(Result<H256, String>) + Send + 'static
{
	thread::spawn(move || {
		match block_on(build_tx_create_nft_store()) {
			Ok(tx) => {
				match push_transaction(tx) {
					Ok(hash) => f(Ok(hash)),
					Err(err) => f(Err(err.to_string()))
				}
			},
			Err(err) => f(Err(err.to_string()))
		}
	});
}

// challenge specified kabletop channel on-chain
pub fn challenge_kabletop_channel<F>(
	script_args: Vec<u8>, challenger: u8, operations: Vec<String>, signed_rounds: Vec<(Round, Signature)>, f: F
) where
	F: Fn(Result<H256, String>) + Send + 'static
{
	thread::spawn(move || {
		match block_on(build_tx_challenge_channel(script_args, challenger, operations.into(), signed_rounds)) {
			Ok(tx) => {
				// write tx to file for debug
				let json_tx = ckb_jsonrpc_types::TransactionView::from(tx.clone());
				let json = serde_json::to_string_pretty(&json_tx).expect("jsonify");
				std::fs::write("challenge_kabletop_channel.json", json).expect("write json file");

				match push_transaction(tx) {
					Ok(hash) => f(Ok(hash)),
					Err(err) => f(Err(err.to_string()))
				}
			},
			Err(err) => f(Err(err.to_string()))
		}
	});
}

pub fn close_challenged_kabletop_channel<F>(
	script_args: Vec<u8>, winner: u8, from_challenge: bool, signed_rounds: Vec<(Round, Signature)>, f: F
) where
	F: Fn(Result<H256, String>) + Send + 'static
{
	// // for debug
	// println!("\n===========================\nprinting operations:");
	// signed_rounds
	// 	.iter()
	// 	.enumerate()
	// 	.for_each(|(i, (round, _))| {
	// 		println!("=> round {} for user {} <=", i + 1, u8::from(round.user_type()));
	// 		let operations: Vec<String> = {
	// 			let operations: Vec<Vec<u8>> = round.operations().into();
	// 			match operations.into_iter().map(|v| String::from_utf8(v)).collect::<Result<Vec<_>, _>>() {
	// 				Ok(value) => value,
	// 				Err(_)    => return
	// 			}
	// 		};
	// 		for code in operations {
	// 			println!("{}", code);
	// 		}
	// 	});
	// println!("===========================\n");
	// // debug end

	thread::spawn(move || {
		match block_on(build_tx_close_channel(script_args, signed_rounds, winner, from_challenge)) {
			Ok(tx) => {
				// write tx to file for debug
				let json_tx = ckb_jsonrpc_types::TransactionView::from(tx.clone());
				let json = serde_json::to_string_pretty(&json_tx).expect("jsonify");
				std::fs::write("close_challenged_kabletop_channel.json", json).expect("write json file");

				match push_transaction(tx) {
					Ok(hash) => f(Ok(hash)),
					Err(err) => f(Err(err.to_string()))
				}
			},
			Err(err) => return f(Err(err.to_string()))
		}
	});
}

// check the lock_script generated by specified script_args is matched with one live cell on chain
pub fn get_kabletop_challenge_data(script_args: Vec<u8>) -> BoxFuture<'static, Result<(bool, Option<Challenge>), String>> {
	let lock_script = kabletop_script(script_args);
	let key = SearchKey::new(lock_script.into(), ScriptType::Lock);
	Box::pin(async move {
		match get_live_cells(key, 1, None).await {
			Ok(channel) => {
				if channel.objects.is_empty() {
					Ok((false, None))
				} else {
					let object = channel.objects.get(0).unwrap();
					let challenge = match Challenge::from_slice(&object.output_data.to_vec()) {
						Ok(data) => Some(data),
						Err(_)   => None
					};
					Ok((true, challenge))
				}
			},
			Err(err) => Err(err.to_string())
		}
	})
}
