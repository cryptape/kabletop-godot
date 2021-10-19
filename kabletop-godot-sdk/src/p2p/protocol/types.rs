use serde::{
	Deserialize, Serialize
};
use ckb_jsonrpc_types::{
	TransactionView, JsonBytes
};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub enum GodotType {
	Nil,
	Bool(bool),
	I64(i64),
	F64(f64),
	String(String)
}

// request messages
pub mod request {
	use super::*;

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

	// winner requests to close channel in default
	#[derive(Serialize, Deserialize)]
	pub struct CloseChannel {
		pub tx:           TransactionView,
		pub channel_hash: [u8; 32]
	}

	// round owner requests to end current round with all of operations made from it
	#[derive(Serialize, Deserialize)]
	pub struct CloseRound {
		pub round:      u8,
		pub operations: Vec<String>
	}

	// sender tells opposite that the game has reached to the end which hp from one of players is down to zero
	#[derive(Serialize, Deserialize)]
	pub struct CloseGame {
		pub round:      u8,
		pub operations: Vec<String>
	}

	// round owner syncs round operation
	#[derive(Serialize, Deserialize)]
	pub struct PushOperation {
		pub round:     u8,
		pub operation: String
	}

	// send user-defined message from godot
	#[derive(Serialize, Deserialize)]
	pub struct SendP2pMessage {
		pub message:    String,
		pub parameters: HashMap<String, GodotType>
	}
}

// response messages
pub mod response {
	use super::*;

	// channel partner prepares his NFTs and public key, and then complete the channel tx to response the organizer
	#[derive(Serialize, Deserialize)]
	pub struct CompleteAndSignChannel {
		pub tx: TransactionView
	}

	// response the result of checking confirmation status of channel transaction on CKB
	#[derive(Serialize, Deserialize)]
	pub struct OpenChannel {
		pub result: bool
	}

	// response the result of checking channel status and cell on CKB network
	#[derive(Serialize, Deserialize)]
	pub struct CloseChannel {
		pub result: bool
	}

	// accept the request of ending round and switch the round owner
	#[derive(Serialize, Deserialize)]
	pub struct OpenRound {
		pub round:     u8,
		pub signature: JsonBytes
	}

	// response wether accept the game finished and generate signature in case of verification passed
	#[derive(Serialize, Deserialize)]
	pub struct CloseGame {
		pub result:    bool,
		pub signature: JsonBytes
	}

	// response the result of pushing operation in server
	#[derive(Serialize, Deserialize)]
	pub struct ApplyOperation {
		pub result: bool
	}

	// reply user-defined message from godot
	#[derive(Serialize, Deserialize)]
	pub struct ReplyP2pMessage {
		pub message:    String,
		pub parameters: HashMap<String, GodotType>
	}
}
