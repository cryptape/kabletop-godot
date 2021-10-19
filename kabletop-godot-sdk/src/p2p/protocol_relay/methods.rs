use kabletop_ckb_sdk::p2p::{
	ClientSender, Caller
};
use super::types::{
	ClientInfo, request, response
};
use serde_json::{
	json, Value, from_value
};
use std::{
	thread, collections::HashMap, sync::Mutex
};
use futures::future::BoxFuture;

pub mod send {
	use super::*;

	// call to register client in relay server
	pub fn register_client(caller: &ClientSender, client_info: ClientInfo) -> Result<(), String> {
		let value: response::RegisterClient = caller.call(
			"register_client", request::RegisterClient {
				nickname:    client_info.nickname,
				staking_ckb: client_info.staking_ckb,
				bet_ckb:     client_info.bet_ckb
			}).map_err(|err| format!("RegisterClient -> {}", err))?;
		if !value.result {
			return Err(String::from("relayserver REFUSED registering client"));
		}
		Ok(())
	}

	// call to unregister client in relay server
	pub fn unregister_client(caller: &ClientSender) -> Result<(), String> {
		let value: response::UnregisterClient = caller.call(
			"unregister_client", request::UnregisterClient {}
		).map_err(|err| format!("UnregisterResult -> {}", err))?;
		if !value.result {
			return Err(String::from("relayserver REFUSED unregistering client"));
		}
		Ok(())
	}

	// request waiting client list 
	pub fn fetch_clients(caller: &ClientSender) -> Result<Vec<ClientInfo>, String> {
		let value: response::FetchClients = caller.call(
			"fetch_clients", request::FetchClients {}
		).map_err(|err| format!("FetchWaitingClients -> {}", err))?;
		Ok(value.clients)
	}

	// connect to one waiting client
	pub fn connect_client(caller: &ClientSender, partial_id: i32, client_info: ClientInfo) -> Result<(), String> {
		let value: response::ConnectClient = caller.call(
			"connect_client", request::ConnectClient {
				client_id: partial_id,
				requester: client_info
			}).map_err(|err| format!("ConnectClient -> {}", err))?;
		if !value.result {
			return Err(String::from("relayserver REFUSED connecting client"));
		}
		Ok(())
	}

	// disconnect from one linking client
	pub fn disconnect_client(caller: &ClientSender) -> Result<(), String> {
		let _: response::DisconnectClient = caller.call(
			"disconnect_client", request::DisconnectClient {}
		).map_err(|err| format!("DisconnectClient -> {}", err))?;
		Ok(())
	}
}

pub mod reply {
	use super::*;

	pub mod hook {
		use super::HOOKS;

		pub fn add<F: Fn(&Vec<u8>) + Sync + Send + 'static>(method: &str, hook: F) {
			let mut hooks = HOOKS.lock().unwrap();
			if let Some(hooks) = hooks.get_mut(&String::from(method)) {
				hooks.push(Box::new(hook));
			} else {
				hooks.insert(String::from(method), vec![Box::new(hook)]);
			}
		}
	}

	lazy_static! {
		static ref HOOKS: Mutex<HashMap<String, Vec<Box<dyn Fn(&Vec<u8>) + Sync + Send>>>> = Mutex::new(HashMap::new());
	}

	fn trigger_hook(method: &str, param: Vec<u8>) {
		let method = String::from(method);
		thread::spawn(move || {
			if let Some(hooks) = HOOKS.lock().unwrap().get(&method) {
				for hook in hooks {
					hook(&param);
				}
			}
		});
	}

	// handle messsage of proposation of client connection
	pub fn propose_connection(_: i32, value: Value) -> BoxFuture<'static, Result<Value, String>> {
		Box::pin(async {
			let _: request::ProposeConnection = from_value(value)
				.map_err(|err| format!("deserialize ProposeConnection -> {}", err))?;
			trigger_hook("propose_connection", vec![]);
			Ok(json!(response::ProposeConnection {
				result: true
			}))
		})
	}

	// handle message of partner client disconnected
	pub fn partner_disconnect(_: i32, value: Value) -> BoxFuture<'static, Result<Value, String>> {
		Box::pin(async {
			let value: request::PartnerDisconnect = from_value(value)
				.map_err(|err| format!("deserialize PartnerDisconnect -> {}", err))?;
			trigger_hook("partner_disconnect", value.client_id.to_le_bytes().to_vec());
			Ok(json!(response::PartnerDisconnect {}))
		})
	}
}
