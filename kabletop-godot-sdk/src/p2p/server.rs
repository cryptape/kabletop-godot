use kabletop_ckb_sdk::p2p::{
	Server, ServerClient
};
use crate::p2p::protocol::{
	types::GodotType, methods::{
		send, reply
	}
};
use std::{
	sync::Mutex, collections::HashMap, sync::mpsc::Receiver
};

lazy_static! {
	static ref SERVER: Mutex<Option<ServerClient>> = Mutex::new(None);
}

// start p2p server and listen connections
pub fn listen<F>(socket: &str, callback: F) -> Result<(), String>
where
	F: Fn(i32, Option<HashMap<String, Receiver<String>>>) + Send + Sync + 'static
{
	let server = Server::new(socket)
		.register("prepare_kabletop_channel", reply::prepare_kabletop_channel)
		.register("open_kabletop_channel", reply::open_kabletop_channel)
		.register("close_kabletop_channel", reply::close_kabletop_channel)
		.register("switch_round", reply::switch_round)
		.register("sync_operation", reply::sync_operation)
		.register("sync_p2p_message", reply::sync_p2p_message)
		.register("notify_game_over", reply::notify_game_over)
		.register_call("close_kabletop_channel")
		.register_call("switch_round")
		.register_call("sync_operation")
		.register_call("sync_p2p_message")
		.register_call("notify_game_over")
		.listen(100, 1, callback);
	match server {
		Ok(listening) => {
			*SERVER.lock().unwrap() = Some(listening);
			Ok(())
		},
		Err(error) => Err(error.to_string())
	}
}

pub fn disconnect() {
	SERVER.lock().unwrap().as_ref().unwrap().shutdown();
}

pub fn change_client(client_id: i32) {
	SERVER.lock().unwrap().as_mut().unwrap().set_id(client_id);
}

pub fn set_client_receivers(client_id: i32, receivers: HashMap<String, Receiver<String>>) {
	SERVER.lock().unwrap().as_mut().unwrap().append_receivers(client_id, receivers);
}

pub fn close_kabletop_channel() -> Result<[u8; 32], String> {
	send::close_kabletop_channel(
		SERVER.lock().unwrap().as_ref().unwrap()
	)
}

pub fn switch_round() -> Result<[u8; 65], String> {
	send::switch_round(
		SERVER.lock().unwrap().as_ref().unwrap()
	)
}

pub fn notify_game_over() -> Result<[u8; 65], String> {
	send::notify_game_over(
		SERVER.lock().unwrap().as_ref().unwrap()
	)
}

pub fn sync_operation(operation: String) -> Result<(), String> {
	send::sync_operation(
		SERVER.lock().unwrap().as_ref().unwrap(), operation
	)
}

pub fn sync_p2p_message(
	message: String, parameters: HashMap<String, GodotType>
) -> Result<(String, HashMap<String, GodotType>), String> {
	send::sync_p2p_message(
		SERVER.lock().unwrap().as_ref().unwrap(), message, parameters
	)
}
