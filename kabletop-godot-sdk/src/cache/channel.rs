use ckb_crypto::secp::Signature;
use std::sync::Mutex;
use molecule::prelude::Entity;
use kabletop_ckb_sdk::{
	config::VARS, ckb::transaction::{
		helper::fee as str_to_capacity, channel::{
			interact::make_round, protocol::{
				Round, Args
			}
		}
	}
};
use serde::{
    Deserialize, Serialize
};

pub enum PLAYER_TYPE {
	ONE, TWO
}

lazy_static! {
	static ref CHANNEL_CACHE: Mutex<ChannelCache> = Mutex::new(ChannelCache::default());
}

// a cache to temporarily store channel consensus data
#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelCache {
	// for kabletop state channel
	pub staking_ckb:     u64,
	pub bet_ckb:         u64,
	pub script_hash:     [u8; 32],
	pub script_args:     Vec<u8>,
	pub channel_hash:    [u8; 32],
	pub capacity:        u64,
	pub max_nfts_count:  u8,
	pub user_nfts:       Vec<[u8; 20]>,
	pub opponent_nfts:   Vec<[u8; 20]>,
	pub user_pkhash:     [u8; 20],
	pub opponent_pkhash: [u8; 20],

	// for kabletop round
	pub winner:           u8,
	pub round:            u8,
	pub round_owner:      u8,
	pub user_type:        u8,
	pub opponent_type:    u8,
	pub round_operations: Vec<String>,
	pub signed_rounds:    Vec<(Vec<u8>, Vec<u8>)>
}

impl Default for ChannelCache {
	fn default() -> Self {
		ChannelCache {
			staking_ckb:      str_to_capacity("300").as_u64(),
			bet_ckb:          str_to_capacity("100").as_u64(),
			script_hash:      [0u8; 32],
			script_args:      vec![],
			channel_hash:     [0u8; 32],
			capacity:         0,
			max_nfts_count:   40,
			user_nfts:        vec![],
			opponent_nfts:    vec![],
			user_pkhash:      VARS.common.user_key.pubhash.clone(),
			opponent_pkhash:  [0u8; 20],
			winner:           0,
			round:            0,
			round_owner:      0,
			user_type:        0,
			opponent_type:    0,
			round_operations: vec![],
			signed_rounds:    vec![]
		}
	}
}

pub fn init(player_type: PLAYER_TYPE) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	*channel = ChannelCache::default();
	match player_type {
		PLAYER_TYPE::ONE => {
			channel.user_type = 1;
			channel.opponent_type = 2;
		},
		PLAYER_TYPE::TWO => {
			channel.user_type = 2;
			channel.opponent_type = 1;
		}
	}
}

pub fn clear() {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	*channel = ChannelCache::default();
}

pub fn set_round_status(count: u8, owner: u8) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	channel.round = count;
	channel.round_owner = owner;
}

pub fn set_staking_and_bet_ckb(staking: u64, bet: u64) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	channel.staking_ckb = str_to_capacity(staking.to_string().as_str()).as_u64();
	channel.bet_ckb = str_to_capacity(bet.to_string().as_str()).as_u64();
}

pub fn set_channel_verification(channel_hash: [u8; 32], script_hash: [u8; 32], script_args: Vec<u8>, capacity: u64) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	channel.channel_hash = channel_hash;
	channel.script_hash = script_hash;
	channel.script_args = script_args;
	channel.capacity = capacity;
}

pub fn set_winner(winner: u8) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	channel.winner = winner;
}

pub fn set_playing_nfts(nfts: Vec<[u8; 20]>) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	channel.user_nfts = nfts;
}

pub fn set_opponent_nfts(nfts: Vec<[u8; 20]>) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	channel.opponent_nfts = nfts;
}

pub fn set_opponent_pkhash(pkhash: [u8; 20]) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	channel.opponent_pkhash = pkhash;
}

pub fn commit_user_round(signature: Signature) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	let round = make_round(channel.user_type, channel.round_operations.clone());
	channel.signed_rounds.push((round.as_slice().to_vec(), signature.serialize()));
	channel.round_operations = vec![];
}

pub fn commit_opponent_round(signature: Signature) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	let round = make_round(channel.opponent_type, channel.round_operations.clone());
	channel.signed_rounds.push((round.as_slice().to_vec(), signature.serialize()));
	channel.round_operations = vec![];
}

pub fn commit_user_operation(operation: String) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	channel.round_operations.push(operation);
}

pub fn commit_opponent_operation(operation: String) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	channel.round_operations.push(operation);
}

pub fn get_kabletop_signed_rounds() -> Result<Vec<(Round, Signature)>, String> {
	get_clone()
		.signed_rounds
		.into_iter()
		.map(|(round, signature)| {
			match Round::from_slice(round.as_slice()) {
				Ok(round) => match Signature::from_slice(signature.as_slice()) {
					Ok(signature) => Ok((round, signature)),
					Err(error)    => Err(error.to_string())
				},
				Err(error) => Err(error.to_string())
			}
		})
		.collect::<Result<Vec<_>, _>>()
}

pub fn get_kabletop_args() -> Result<Args, String> {
	Ok(Args::from_slice(&get_clone().script_args).map_err(|err| err.to_string())?)
}

pub fn persist(name: String) -> Result<(), String> {
	let path = format!("db/{}.json", name);
	let content = serde_json::to_string_pretty(&get_clone()).map_err(|err| err.to_string())?;
	std::fs::write(path, content).map_err(|err| err.to_string())?;
	Ok(())
}

pub fn recover(name: String) -> Result<ChannelCache, String> {
	let path = format!("db/{}.json", name);
	let content = std::fs::read_to_string(path.clone())
		.map_err(|err| format!("{} => {}", err, path))?;
	let channel: ChannelCache = serde_json::from_str(content.as_str())
		.map_err(|err| err.to_string())?;
	*CHANNEL_CACHE.lock().unwrap() = channel;
	Ok(get_clone())
}

pub fn get_clone() -> ChannelCache {
	CHANNEL_CACHE.lock().unwrap().clone()
}
