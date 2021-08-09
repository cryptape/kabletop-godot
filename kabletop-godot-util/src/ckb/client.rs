use kabletop_sdk::p2p::{
	Client, ClientSender
};
use crate::ckb::protocol::{
	send, reply
};
use std::sync::Mutex;

lazy_static! {
	static ref CLIENT: Mutex<Option<ClientSender>> = Mutex::new(None);
}

// try to enstablish connection between client and server
pub fn connect<F: Fn() + Send + 'static>(socket: &str, callback: F) {
	let mut client = CLIENT
		.lock()
		.unwrap();
	*client = Some(
		Client::new(socket)
			.register("switch_round", reply::switch_round)
			.register("sync_operation", reply::sync_operation)
			.register_call("propose_channel_parameter")
			.register_call("prepare_kabletop_channel")
			.register_call("open_kabletop_channel")
			.register_call("switch_round")
			.register_call("sync_operation")
			.connect(200, callback)
			.expect("connect")
	);
}

pub fn propose_channel_parameter() -> bool {
	send::propose_channel_parameter(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}

pub fn open_kabletop_channel() -> bool {
	send::open_kabletop_channel(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}

pub fn switch_round() -> u8 {
	send::switch_round(
		CLIENT.lock().unwrap().as_ref().unwrap()
	)
}

pub fn sync_operation(operation: String) -> bool {
	send::sync_operation(
		CLIENT.lock().unwrap().as_ref().unwrap(), operation
	)
}
