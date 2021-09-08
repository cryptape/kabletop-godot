use kabletop_sdk::ckb::{
	wallet::keystore, rpc::{
		types::{
			SearchKey, ScriptType
		}, methods::{
			send_transaction, get_transaction, get_live_nfts, get_live_cells
		}
	}, transaction::{
		builder::*, helper::{
			sighash_script, wallet_script, nft_script, payment_script, blake160_to_byte20
		}
	}
};
use futures::executor::block_on;
use ckb_types::{
	core::TransactionView, prelude::Pack
};
use std::{
	thread, time::Duration, collections::HashMap
};
pub use ckb_types::H256;

// get user owned nfts by their lock_script (from user_pkhash) and type_script (from composer_pkhash)
pub fn owned_nfts() -> Result<HashMap<String, u32>, String> {
    let lock_script = sighash_script(&keystore::USER_PUBHASH.to_vec());
    let type_script = {
        let wallet = wallet_script(keystore::COMPOSER_PUBHASH.to_vec());
        nft_script(wallet.calc_script_hash().raw_data().to_vec())
    };
	let nfts = block_on(get_live_nfts(lock_script, Some(type_script), 10))
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
