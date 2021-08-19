use kabletop_sdk::p2p::{
	Server, ServerClient
};
use crate::p2p::protocol::{
	send, reply
};
use std::sync::Mutex;

lazy_static! {
	static ref SERVER: Mutex<Option<ServerClient>> = Mutex::new(None);
}

// start p2p server and listen connections
pub fn listen<F: Fn(bool) + Send + 'static>(socket: &str, callback: F) {
	let mut serverclient = SERVER
		.lock()
		.unwrap();
	*serverclient = Some(
		Server::new(socket)
			.register("propose_channel_parameter", reply::propose_channel_parameter)
			.register("prepare_kabletop_channel", reply::prepare_kabletop_channel)
			.register("open_kabletop_channel", reply::open_kabletop_channel)
			.register("switch_round", reply::switch_round)
			.register("sync_operation", reply::sync_operation)
			.register_call("switch_round")
			.register_call("sync_operation")
			.listen(100, callback)
			.expect("listen")
	);
}

pub fn switch_round() -> [u8; 65] {
	send::switch_round(
		SERVER.lock().unwrap().as_ref().unwrap()
	)
}

pub fn sync_operation(operation: String) -> bool {
	send::sync_operation(
		SERVER.lock().unwrap().as_ref().unwrap(), operation
	)
}
