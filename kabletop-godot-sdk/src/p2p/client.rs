use kabletop_ckb_sdk::p2p::{
	Client, ClientSender
};
use crate::p2p::{
	protocol::{
		types::GodotType, methods::{
			send, reply
		}
	}, protocol_relay::{
		types::ClientInfo, methods::{
			send as send_relay, reply as reply_relay
		}
	}
};
use std::{
	sync::Mutex, collections::HashMap
};

lazy_static! {
	static ref CLIENT: Mutex<Option<ClientSender>> = Mutex::new(None);
}

// try to enstablish connection between client and server
pub fn connect<F: Fn() + Send + 'static>(socket: &str, callback: F) -> Result<(), String> {
	let client = Client::new(socket)
		// for native connections
		.register("switch_round", reply::switch_round)
		.register("sync_operation", reply::sync_operation)
		.register("sync_p2p_message", reply::sync_p2p_message)
		.register("close_kabletop_channel", reply::close_kabletop_channel)
		.register("notify_game_over", reply::notify_game_over)
		.register_call("prepare_kabletop_channel")
		.register_call("open_kabletop_channel")
		.register_call("close_kabletop_channel")
		.register_call("switch_round")
		.register_call("sync_operation")
		.register_call("sync_p2p_message")
		.register_call("notify_game_over")
		// for relay connections
		.register("prepare_kabletop_channel", reply::prepare_kabletop_channel)
		.register("open_kabletop_channel", reply::open_kabletop_channel)
		.register("propose_connection", reply_relay::propose_connection)
		.register("partner_disconnect", reply_relay::partner_disconnect)
		.register_call("register_client")
		.register_call("unregister_client")
		.register_call("fetch_clients")
		.register_call("connect_client")
		.register_call("disconnect_client")
		.connect(100, callback);
	match client {
		Ok(connection) => {
			*CLIENT.lock().unwrap() = Some(connection);
			Ok(())
		},
		Err(error) => Err(error.to_string())
	}
}

// cut down connection between client and server
pub fn disconnect() {
	CLIENT.lock().unwrap().as_ref().unwrap().shutdown();
}

//////////////////////////////////////////////
// for native connection sending messages
//////////////////////////////////////////////

pub fn open_kabletop_channel() -> Result<[u8; 32], String> {
	send::open_kabletop_channel(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}

pub fn close_kabletop_channel() -> Result<[u8; 32], String> {
	send::close_kabletop_channel(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}

pub fn switch_round() -> Result<[u8; 65], String> {
	send::switch_round(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}

pub fn notify_game_over() -> Result<[u8; 65], String> {
	send::notify_game_over(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}

pub fn sync_operation(operation: String) -> Result<(), String> {
	send::sync_operation(
		CLIENT.lock().unwrap().as_ref().unwrap(), operation
	)
}

pub fn sync_p2p_message(
	message: String, parameters: HashMap<String, GodotType>
) -> Result<(String, HashMap<String, GodotType>), String> {
	send::sync_p2p_message(
		CLIENT.lock().unwrap().as_ref().unwrap(), message, parameters
	)
}

//////////////////////////////////////////////
// for relay connection sending messages
//////////////////////////////////////////////

pub fn register_client(
	nickname: String, staking_ckb: u64, bet_ckb: u64
) -> Result<(), String> {
	send_relay::register_client(
		CLIENT.lock().unwrap().as_ref().unwrap(), ClientInfo {
			id: 0, nickname, staking_ckb, bet_ckb
		}
	)
}

pub fn unregister_client() -> Result<(), String> {
	send_relay::unregister_client(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}

pub fn fetch_clients() -> Result<Vec<ClientInfo>, String> {
	send_relay::fetch_clients(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}

pub fn connect_client(
	partial_id: i32, nickname: String, staking_ckb: u64, bet_ckb: u64
) -> Result<(), String> {
	send_relay::connect_client(
		CLIENT.lock().unwrap().as_ref().unwrap(), partial_id, ClientInfo {
			id: 0, nickname, staking_ckb, bet_ckb
		}
	)
}

pub fn disconnect_client() -> Result<(), String> {
	send_relay::disconnect_client(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}
