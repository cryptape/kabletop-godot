use kabletop_sdk::p2p::{
	Server, ServerClient
};
use crate::p2p::{
	GodotType, protocol::{
		send, reply
	}
};
use std::{
	sync::Mutex, collections::HashMap
};

lazy_static! {
	static ref SERVER: Mutex<Option<ServerClient>> = Mutex::new(None);
}

// start p2p server and listen connections
pub fn listen<F: Fn(bool) + Send + 'static>(socket: &str, callback: F) -> Result<(), String> {
	let server = Server::new(socket)
		.register("propose_channel_parameter", reply::propose_channel_parameter)
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
		.listen(100, callback);
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
