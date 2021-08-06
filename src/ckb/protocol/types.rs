use serde::{
	Deserialize, Serialize
};
use ckb_jsonrpc_types::{
	TransactionView, JsonBytes
};

// request messages
pub mod request {
	use super::*;

	// reach a consensus of staking_ckb and bet_ckb
	#[derive(Serialize, Deserialize)]
	pub struct ProposeGameParameter {
		pub staking_ckb: u64,
		pub bet_ckb:     u64
	}

	// channel organizer prepares his NFTs and public key to the partner
	#[derive(Serialize, Deserialize)]
	pub struct PrepareChannel {
		pub tx: TransactionView
	}

	// channel organizer signs tx with his private key and submits transaction to open channel
	#[derive(Serialize, Deserialize)]
	pub struct SignAndSubmitChannel {
		pub tx: TransactionView
	}

	// round owner requests to end current round with all of operations made from it
	#[derive(Serialize, Deserialize)]
	pub struct CloseRound {
		pub round:      u8,
		pub operations: Vec<String>
	}

	// round owner syncs round operation
	#[derive(Serialize, Deserialize)]
	pub struct PushOperation {
		pub round:     u8,
		pub operation: String
	}
}

// response messages
pub mod response {
	use super::*;

	// response the agreement of the game parameters
	#[derive(Serialize, Deserialize)]
	pub struct ApproveGameParameter {
		pub result: bool
	}

	// channel partner prepares his NFTs and public key, and then complete the channel tx to response the organizer
	#[derive(Serialize, Deserialize)]
	pub struct CompleteAndSignChannel {
		pub tx: TransactionView
	}

	// response the result of checking confirmation status of channel transaction on the CKB
	#[derive(Serialize, Deserialize)]
	pub struct OpenChannel {
		pub result: bool
	}

	// accept the request of ending round and switch the round owner
	#[derive(Serialize, Deserialize)]
	pub struct OpenRound {
		pub round:     u8,
		pub signature: JsonBytes
	}

	// response the result of pushing operation in server
	#[derive(Serialize, Deserialize)]
	pub struct ApplyOperation {
		pub result: bool
	}
}
