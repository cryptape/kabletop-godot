use ckb_crypto::secp::Signature;
use std::sync::Mutex;
use kabletop_ckb_sdk::{
	config::VARS, ckb::transaction::{
		helper::fee as str_to_capacity, channel::{
			protocol::Round, interact::make_round
		}
	}
};

pub enum PLAYER_TYPE {
	ONE, TWO
}

lazy_static! {
	static ref CHANNEL_CACHE: Mutex<ChannelCache> = Mutex::new(ChannelCache::default());
}

// a cache to temporarily store channel consensus data
#[derive(Clone)]
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
	pub signed_rounds:    Vec<(Round, Signature)>
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
	channel.signed_rounds.push((round, signature));
	channel.round_operations = vec![];
}

pub fn commit_opponent_round(signature: Signature) {
	let mut channel = CHANNEL_CACHE.lock().unwrap();
	let round = make_round(channel.opponent_type, channel.round_operations.clone());
	channel.signed_rounds.push((round, signature));
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

pub fn get_clone() -> ChannelCache {
	CHANNEL_CACHE.lock().unwrap().clone()
}
