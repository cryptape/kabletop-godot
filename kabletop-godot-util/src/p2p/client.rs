use kabletop_sdk::p2p::{
	Client, ClientSender
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
	static ref CLIENT: Mutex<Option<ClientSender>> = Mutex::new(None);
}

// try to enstablish connection between client and server
pub fn connect<F: Fn() + Send + 'static>(socket: &str, callback: F) -> bool {
	let connection = Client::new(socket)
		.register("switch_round", reply::switch_round)
		.register("sync_operation", reply::sync_operation)
		.register("sync_p2p_message", reply::sync_p2p_message)
		.register("close_kabletop_channel", reply::close_kabletop_channel)
		.register("notify_game_over", reply::notify_game_over)
		.register_call("propose_channel_parameter")
		.register_call("prepare_kabletop_channel")
		.register_call("open_kabletop_channel")
		.register_call("close_kabletop_channel")
		.register_call("switch_round")
		.register_call("sync_operation")
		.register_call("sync_p2p_message")
		.register_call("notify_game_over")
		.connect(100, callback);
	if let Ok(conn) = connection {
		*CLIENT.lock().unwrap() = Some(conn);
		true
	} else {
		false
	}
}

// cut down connection between client and server
pub fn disconnect() {
	let mut client = CLIENT.lock().unwrap();
	client.as_ref().unwrap().shutdown();
	*client = None;
}

pub fn propose_channel_parameter() -> Result<(), String> {
	send::propose_channel_parameter(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}

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
