use kabletop_sdk::ckb::{
	rpc::methods::{
		send_transaction, get_transaction
	}, transaction::builder::{
		build_tx_discard_nft, build_tx_purchase_nft_package, build_tx_reveal_nft_package
	}
};
use futures::executor::block_on;
use ckb_types::{
	core::TransactionView, prelude::Pack
};
use std::{
	convert::TryInto, thread, sync::Mutex, time::Duration
};

lazy_static! {
	static ref UUID: Mutex<u32> = Mutex::new(1);
}

fn next_uuid() -> u32 {
	let uuid = *UUID.lock().unwrap();
	*UUID.lock().unwrap() = uuid + 1;
	uuid
}

pub use kabletop_sdk::ckb::transaction::helper::owned_nfts;
pub use ckb_types::H256;

fn push_transaction(tx: TransactionView) -> Result<H256, String> {
	let error: String;
	match send_transaction(tx.data()) {
		Ok(hash) => {
			for _ in 0..10 {
				if get_transaction(hash.pack()).is_ok() {
					return Ok(hash)
				}
				thread::sleep(Duration::from_secs(1));
			}
			error = String::from("transaction confirmation timeout");
		},
		Err(err) => error = err.to_string()
	}
	Err(error)
}

pub fn discard_nfts<F>(nfts: &Vec<String>, f: F) -> u32
where 
	F: Fn(u32, Result<H256, String>) + Send + 'static
{
	let uuid = next_uuid();
	let nfts = nfts
		.iter()
		.map(|nft| hex::decode(nft).unwrap().try_into().unwrap())
		.collect::<Vec<_>>();
	thread::spawn(move || {
		match block_on(build_tx_discard_nft(&nfts)) {
			Ok(tx) => {
				match push_transaction(tx) {
					Ok(hash) => f(uuid, Ok(hash)),
					Err(err) => f(uuid, Err(err.to_string()))
				}
			},
			Err(err) => f(uuid, Err(err.to_string()))
		}
	});
	uuid
}

pub fn purchase_nfts<F>(count: u8, f: F) ->  u32
where
	F: Fn(u32, Result<H256, String>) + Send + 'static
{
	let uuid = next_uuid();
	thread::spawn(move || {
		match block_on(build_tx_purchase_nft_package(count)) {
			Ok(tx) => {
				match push_transaction(tx) {
					Ok(hash) => f(uuid, Ok(hash)),
					Err(err) => f(uuid, Err(err.to_string()))
				}
			},
			Err(err) => f(uuid, Err(err.to_string()))
		}
	});
	uuid
}

pub fn reveal_nfts<F>(f: F) -> u32
where
	F: Fn(u32, Result<H256, String>) + Send + 'static
{
	let uuid = next_uuid();
	thread::spawn(move || {
		match block_on(build_tx_reveal_nft_package()) {
			Ok(tx) => {
				match push_transaction(tx) {
					Ok(hash) => f(uuid, Ok(hash)),
					Err(err) => f(uuid, Err(err.to_string()))
				}
			},
			Err(err) => f(uuid, Err(err.to_string()))
		}
	});
	uuid
}
