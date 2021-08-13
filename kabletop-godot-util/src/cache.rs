use std::sync::Mutex;
use ckb_crypto::secp::Signature;
use kabletop_sdk::{
	config::VARS, ckb::transaction::channel::{
		protocol::Round, interact::make_round
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
	pub capacity:        u64,
	pub max_nfts_count:  u8,
	pub user_nfts:       Vec<[u8; 20]>,
	pub opponent_nfts:   Vec<[u8; 20]>,
	pub user_pkhash:     [u8; 20],
	pub opponent_pkhash: [u8; 20],
	pub luacode_hashes:  Vec<[u8; 32]>,

	// for kabletop round
	pub round:               u8,
	pub round_owner:         u8,
	pub user_type:           u8,
	pub user_operations:     Vec<String>,   // operator latest round operations
	pub opponent_type:       u8,
	pub opponent_operations: Vec<String>,   // opponent latest round operations
	pub signed_rounds:       Vec<(Round, Signature)>
}

impl Default for ChannelCache {
	fn default() -> Self {
		ChannelCache {
			staking_ckb:         5000,
			bet_ckb:             1000,
			script_hash:         [0u8; 32],
			capacity:            0,
			max_nfts_count:      9,
			user_nfts:           vec![],
			opponent_nfts:       vec![],
			user_pkhash:         VARS.common.user_key.pubhash.clone(),
			opponent_pkhash:     [0u8; 20],
			luacode_hashes:      vec![],
			round:               0,
			round_owner:         0,
			user_type:           0,
			user_operations:     vec![],
			opponent_type:       0,
			opponent_operations: vec![],
			signed_rounds:       vec![]
		}
	}
}

pub fn init(player_type: PLAYER_TYPE) {
	let mut channel_cache = CHANNEL_CACHE
		.lock()
		.unwrap();
	*channel_cache = ChannelCache::default();
	match player_type {
		PLAYER_TYPE::ONE => {
			channel_cache.round = 1;
			channel_cache.user_type = 1;
			channel_cache.opponent_type = 2;
			channel_cache.round_owner = channel_cache.user_type;
		},
		PLAYER_TYPE::TWO => {
			channel_cache.round = 1;
			channel_cache.user_type = 2;
			channel_cache.opponent_type = 1;
			channel_cache.round_owner = channel_cache.opponent_type;
		}
	}
}

pub fn set_staking_and_bet_ckb(staking: u64, bet: u64) {
	let mut channel_cache = CHANNEL_CACHE
		.lock()
		.unwrap();
	channel_cache.staking_ckb = staking;
	channel_cache.bet_ckb = bet;
}

pub fn set_scripthash_and_capacity(script_hash: [u8; 32], capacity: u64) {
	let mut channel_cache = CHANNEL_CACHE
		.lock()
		.unwrap();
	channel_cache.script_hash = script_hash;
	channel_cache.capacity = capacity;
}

pub fn set_playing_nfts(nfts: Vec<[u8; 20]>) {
	// TODO: check nfts' existence in CKB
	let mut channel_cache = CHANNEL_CACHE
		.lock()
		.unwrap();
	channel_cache.user_nfts = nfts;
}

pub fn set_opponent_nfts(nfts: Vec<[u8; 20]>) {
	let mut channel_cache = CHANNEL_CACHE
		.lock()
		.unwrap();
	channel_cache.opponent_nfts = nfts;
}

pub fn set_opponent_pkhash(pkhash: [u8; 20]) {
	let mut channel_cache = CHANNEL_CACHE
		.lock()
		.unwrap();
	channel_cache.opponent_pkhash = pkhash;
}

pub fn commit_user_round(signature: Signature) {
	let mut channel_cache = CHANNEL_CACHE
		.lock()
		.unwrap();
	assert!(channel_cache.round_owner == channel_cache.user_type);
	let round = make_round(channel_cache.user_type, &channel_cache.user_operations);
	channel_cache.round += 1;
	channel_cache.round_owner = channel_cache.opponent_type;
	channel_cache.signed_rounds.push((round, signature));
	channel_cache.user_operations = vec![];
}

pub fn commit_opponent_round(signature: Signature) {
	let mut channel_cache = CHANNEL_CACHE
		.lock()
		.unwrap();
	assert!(channel_cache.round_owner == channel_cache.opponent_type);
	let round = make_round(channel_cache.opponent_type, &channel_cache.opponent_operations);
	channel_cache.round += 1;
	channel_cache.round_owner = channel_cache.user_type;
	channel_cache.signed_rounds.push((round, signature));
	channel_cache.opponent_operations = vec![];
}

pub fn commit_round_operation(operation: String) {
	let mut channel_cache = CHANNEL_CACHE
		.lock()
		.unwrap();
	if channel_cache.round_owner == channel_cache.user_type {
		channel_cache.user_operations.push(operation);
	} else if channel_cache.round_owner == channel_cache.opponent_type {
		channel_cache.opponent_operations.push(operation);
	} else {
		panic!("uninited channel cache");
	}
}

pub fn get_clone() -> ChannelCache {
	CHANNEL_CACHE
		.lock()
		.unwrap()
		.clone()
}
