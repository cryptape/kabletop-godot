use serde::{
	Deserialize, Serialize
};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Default)]
pub struct ClientInfo {
	pub id:          i32,
	pub nickname:    String,
	pub staking_ckb: u64,
	pub bet_ckb:     u64
}

pub mod request {
	use super::*;

	// request to tell relay server to register client info
	#[derive(Serialize, Deserialize)]
	pub struct RegisterClient {
		pub nickname:    String,
		pub staking_ckb: u64,
		pub bet_ckb:     u64
	}

	// request to tell relay server to erase client register info
	#[derive(Serialize, Deserialize)]
	pub struct UnregisterClient {}

	// request current waiting clients list in relay server
	#[derive(Serialize, Deserialize)]
	pub struct FetchClients {}

	// request to connect specified waiting client
	#[derive(Serialize, Deserialize)]
	pub struct ConnectClient {
		pub client_id: i32,
		pub requester: ClientInfo
	}

	// request to disconnect specified linking client
	#[derive(Serialize, Deserialize)]
	pub struct DisconnectClient {}

	// ask client the proposation of the other client's connection
	#[derive(Serialize, Deserialize)]
	pub struct ProposeConnection {}

	// request to notify client disconnected event
	#[derive(Serialize, Deserialize)]
	pub struct PartnerDisconnect {
		pub client_id: i32
	}
}

pub mod response {
	use super::*;

	// response the result of client register
	#[derive(Serialize, Deserialize)]
	pub struct RegisterClient {
		pub result: bool
	}

	// response the result of client unregister
	#[derive(Serialize, Deserialize)]
	pub struct UnregisterClient {
		pub result: bool
	}

	// response waiting clients list in relay server
	#[derive(Serialize, Deserialize)]
	pub struct FetchClients {
		pub clients: Vec<ClientInfo>
	}

	// response the result of client connection
	#[derive(Serialize, Deserialize)]
	pub struct ConnectClient {
		pub result: bool
	}

	// response the result of client disconnection
	#[derive(Serialize, Deserialize)]
	pub struct DisconnectClient {}

	// response the result of connection confirmation
	#[derive(Serialize, Deserialize)]
	pub struct ProposeConnection {
		pub result: bool
	}

	// response the result of client connection
	#[derive(Serialize, Deserialize)]
	pub struct PartnerDisconnect {}
}
