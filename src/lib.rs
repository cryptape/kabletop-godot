use gdnative::prelude::*;
use gdnative::api::*;
use kabletop_godot_util::{
	lua::highlevel::Lua, cache, ckb::*, p2p::{
		client, server, hook, GodotType
	}
};
use std::{
	thread, collections::HashMap
};

mod helper;
use helper::*;

#[derive(NativeClass)]
#[inherit(Node)]
#[register_with(Self::register_signals)]
struct Kabletop {
	nfts: Vec<String>
}

#[gdnative::methods]
impl Kabletop {
    fn new(_owner: &Node) -> Self {
		// turn all println! to godot_print!
		*kabletop_godot_util::USE_GODOT.lock().unwrap() = true;
		// set hooks
		hook::add("sync_operation", |operation| {
			let value = String::from_utf8(operation.clone()).unwrap();
			run_code(value);
		});
		hook::add("switch_round", |signature| {
			randomseed(signature);
		});
		hook::add("open_kabletop_channel", |hash| {
			let store = cache::get_clone();
			let mut ckb_time: i64 = 0;
			for i in 0..8 {
				ckb_time = (ckb_time << 8) | (store.script_hash[i] as i64 >> 1);
			}
			let mut ckb_clock: i64 = 0;
			for i in 8..16 {
				ckb_clock = (ckb_clock << 8) | (store.script_hash[i] as i64 >> 1);
			}
			let lua = Lua::new(ckb_time, ckb_clock);
			lua.inject_nfts(from_nfts(store.opponent_nfts.clone()), from_nfts(store.user_nfts.clone()));
			lua.boost(get_lua_entry());
			set_lua(lua);
			randomseed(hash);
			call_hook_funcref("open_kabletop_channel", vec![true.to_variant(), hex::encode(hash).to_variant()]);
		});
		// instance kabletop godot object
        Kabletop {
			nfts: vec![]
		}
    }

	fn register_signals(builder: &ClassBuilder<Self>) {
        builder.add_signal(Signal {
            name: "connect_status",
            args: &[
				SignalArgument {
					name: "mode",
					default: Variant::default(),
					export_info: ExportInfo::new(VariantType::GodotString),
					usage: PropertyUsage::DEFAULT
				},
				SignalArgument {
					name: "status",
					default: Variant::default(),
					export_info: ExportInfo::new(VariantType::Bool),
					usage: PropertyUsage::DEFAULT
				}
            ]
        });
        builder.add_signal(Signal {
            name: "lua_events",
            args: &[
				SignalArgument {
					name: "events",
					default: Vec::<Variant>::new().to_variant(),
					export_info: ExportInfo::new(VariantType::VariantArray),
					usage: PropertyUsage::DEFAULT
				}
            ]
        });
        builder.add_signal(Signal {
            name: "owned_nfts_updated",
            args: &[
				SignalArgument {
					name: "owned_nfts",
					default: Dictionary::new_shared().to_variant(),
					export_info: ExportInfo::new(VariantType::Dictionary),
					usage: PropertyUsage::DEFAULT
				}
			]
        });
        builder.add_signal(Signal {
            name: "box_status_updated",
            args: &[
				SignalArgument {
					name: "box_count",
					default: 0.to_variant(),
					export_info: ExportInfo::new(VariantType::I64),
					usage: PropertyUsage::DEFAULT
				},
				SignalArgument {
					name: "reveal_ready",
					default: true.to_variant(),
					export_info: ExportInfo::new(VariantType::Bool),
					usage: PropertyUsage::DEFAULT
				}
			]
        });
        builder.add_signal(Signal {
            name: "p2p_message_reply",
            args: &[
				SignalArgument {
					name: "message",
					default: "".to_variant(),
					export_info: ExportInfo::new(VariantType::GodotString),
					usage: PropertyUsage::DEFAULT
				},
				SignalArgument {
					name: "parameters",
					default: Dictionary::new_shared().to_variant(),
					export_info: ExportInfo::new(VariantType::Dictionary),
					usage: PropertyUsage::DEFAULT
				}
			]
        });
    }

    #[export]
    fn _ready(&mut self, owner: TRef<Node>) {
        godot_print!("welcome to the kabletop world!");
		set_emitor(owner.claim());
		update_owned_nfts();
		update_box_status();
    }

	#[export]
	fn _process(&mut self, _owner: &Node, delta: f32) {
		if let Ok(mut events) = EVENTS.try_lock() {
			if let Some(emitor) = get_emitor() {
				for (name, value) in &*events {
					if name == "owned_nfts_updated" {
						self.nfts = vec![];
					}
					unsafe { emitor.assume_safe().emit_signal(name, value.as_slice()); }
				}
				(*events).clear();
			}
		}
		if let Ok(mut funcrefs) = FUNCREFS.try_lock() {
			for (callback, values) in &*funcrefs {
				unsafe { callback.assume_safe().call_func(values); }
			}
			(*funcrefs).clear();
		}
		process_delay_funcs(delta);
	}

	#[export]
	fn set_entry(&mut self, _owner: &Node, entry: String) {
		set_lua_entry(entry);
	}

	#[export]
	fn set_nfts(&mut self, _owner: &Node, nfts: Dictionary) {
		self.nfts = nfts
			.iter()
			.map(|(nft, count)| vec![nft.to_string(); count.to_u64() as usize])
			.collect::<Vec<_>>()
			.concat();
	}

	#[export]
	fn get_nfts(&self, _owner: &Node) -> Dictionary {
		if self.nfts.len() > 0 {
			let mut last_nft = self.nfts[0].clone();
			let mut count = 0;
			let nfts = Dictionary::new();
			for nft in &self.nfts {
				if &last_nft == nft {
					count += 1;
				} else {
					nfts.insert(last_nft, count);
					last_nft = nft.clone();
					count = 1;
				}
			}
			nfts.insert(last_nft, count);
			nfts.into_shared()
		} else {
			Dictionary::new_shared()
		}
	}

	#[export]
	fn get_nfts_count(&self, _owner: &Node, player_id: u8) -> usize {
		let store = cache::get_clone();
		if player_id > 0 {
			if player_id == store.user_type {
				store.user_nfts.len()
			} else if player_id == store.opponent_type {
				store.opponent_nfts.len()
			} else {
				panic!("unknown player_id {}", player_id);
			} 
		} else {
			self.nfts.len()
		}
	}

	#[export]
	fn delete_nfts(&mut self, _owner: &Node, nfts: Dictionary, callback: Ref<FuncRef>) {
		let nfts = nfts
			.iter()
			.map(|(nft, count)| vec![nft.to_string(); count.to_u64() as usize])
			.collect::<Vec<_>>()
			.concat();
		if nfts.len() > 0 {
			discard_nfts(&nfts, handle_transaction(update_owned_nfts, callback));
		}
	}

	#[export]
	fn purchase_nfts(&self, _owner: &Node, count: u8, callback: Ref<FuncRef>) {
		purchase_nfts(count, handle_transaction(update_box_status, callback));
	}

	#[export]
	fn reveal_nfts(&self, _owner: &Node, callback: Ref<FuncRef>) {
		reveal_nfts(handle_transaction(|| {
			update_box_status();
			update_owned_nfts();
		}, callback));
	}

	#[export]
	fn create_nft_wallet(&self, _owner: &Node, callback: Ref<FuncRef>) {
		create_wallet(handle_transaction(update_box_status, callback));
	}

	#[export]
	fn get_owned_nfts(&self, _owner: &Node) -> Option<Variant> {
		(*NFTS.lock().unwrap()).clone()
	}

	#[export]
	fn get_box_status(&self, _owner: &Node) -> Option<Dictionary> {
		if let Some(status) = *STATUS.lock().unwrap() {
			let map = Dictionary::new();
			map.insert("count", status.0.to_variant());
			map.insert("ready", status.1.to_variant());
			Some(map.into_shared())
		} else {
			None
		}
	}

	#[export]
	fn get_player_id(&self, _owner: &Node) -> i64 {
		cache::get_clone().user_type as i64
	}

	#[export]
	fn connect_to(&self, _owner: &Node, socket: String) -> Variant {
		let result = client::connect(socket.as_str(), || {
			unset_lua();
			push_event("connect_status", vec!["CLIENT".to_variant(), false.to_variant()]);
		});
		if let Err(err) = result {
			return err.to_variant();
		}
		set_mode(P2pMode::Client);
		push_event("connect_status", vec!["CLIENT".to_variant(), true.to_variant()]);
		Variant::default()
	}

	#[export]
	fn listen_at(&self, _owner: &Node, socket: String, callback: Ref<FuncRef>) -> Variant {
		cache::init(cache::PLAYER_TYPE::TWO);
		cache::set_playing_nfts(into_nfts(self.nfts.clone()));
		let result = server::listen(socket.as_str(), move |client_connected| {
			if client_connected {
				add_hook_funcref("open_kabletop_channel", callback.clone());
				push_event("connect_status", vec!["SERVER".to_variant(), true.to_variant()]);
			} else {
				unset_lua();
				push_event("connect_status", vec!["SERVER".to_variant(), false.to_variant()]);
			}
		});
		if let Err(err) = result {
			return err.to_variant();
		}
		set_mode(P2pMode::Server);
		Variant::default()
	}

	#[export]
	fn shutdown(&self, _owner: &Node) {
		disconnect().unwrap();
	}

	#[export]
	fn create_channel(&self, _owner: &Node, callback: Ref<FuncRef>) -> Variant {
		if self.nfts.len() == 0 {
			return "empty nfts".to_variant();
		}
		if get_mode() != P2pMode::Client {
			return "no client mode".to_variant();
		}
		cache::init(cache::PLAYER_TYPE::ONE);
		cache::set_playing_nfts(into_nfts(self.nfts.clone()));
		thread::spawn(move || match client::open_kabletop_channel() {
			Ok(hash) => {
				// create lua vm
				let channel = cache::get_clone();
				let mut ckb_time: i64 = 0;
				for i in 0..8 {
					ckb_time = (ckb_time << 8) | (channel.script_hash[i] as i64 >> 1);
				}
				let mut ckb_clock: i64 = 0;
				for i in 8..16 {
					ckb_clock = (ckb_clock << 8) | (channel.script_hash[i] as i64 >> 1);
				}
				let lua = Lua::new(ckb_time, ckb_clock);
				lua.inject_nfts(from_nfts(channel.user_nfts.clone()), from_nfts(channel.opponent_nfts.clone()));
				lua.boost(get_lua_entry());
				set_lua(lua);

				// set first randomseed and callback to gdscript
				randomseed(&hash);
				FUNCREFS.lock().unwrap().push((callback, vec![true.to_variant(), hex::encode(hash).to_variant()]));
			},
			Err(err) => {
				FUNCREFS.lock().unwrap().push((callback.clone(), vec![false.to_variant(), err.to_variant()]));
			}
		});
		Variant::default()
	}

	#[export]
	fn close_channel(&self, _owner: &Node, callback: Ref<FuncRef>) {
		thread::spawn(move || match close_kabletop_channel() {
			Ok(hash) => {
				disconnect().unwrap();
				FUNCREFS.lock().unwrap().push((callback, vec![true.to_variant(), hex::encode(hash).to_variant()]));
			},
			Err(err) => {
				FUNCREFS.lock().unwrap().push((callback, vec![false.to_variant(), err.to_variant()]));
			}
		});
	}

	#[export]
	fn close_game(&self, _owner: &Node, winner: u8, callback: Ref<FuncRef>) {
		cache::set_winner(winner);
		let store = cache::get_clone();
		if store.round_owner == store.user_type {
			thread::spawn(move || {
				notify_game_over().unwrap();
				FUNCREFS.lock().unwrap().push((callback, vec![winner.to_variant()]));
			});
		} else {
			unsafe { callback.assume_safe().call_func(&[winner.to_variant()]); }
		}
	}

	#[export]
	fn run(&self, _owner: &Node, code: String, terminal: bool) {
		run_code(code.clone());
		thread::spawn(move || {
			sync_operation(code).unwrap();
			if terminal {
				let signature = switch_round().unwrap();
				randomseed(&signature);
			}
		});
	}

	#[export]
	fn reply_p2p_message(&self, _owner: &Node, message: String, callback: Ref<FuncRef>) {
		cache::set_godot_callback(message.as_str(), Box::new(move |parameters: HashMap<String, GodotType>| {
			unsafe {
				let values = Dictionary::new();
				parameters
					.iter()
					.for_each(|(name, value)| {
						let value = match value {
							GodotType::Bool(value)   => value.to_variant(),
							GodotType::I64(value)    => value.to_variant(),
							GodotType::F64(value)    => value.to_variant(),
							GodotType::String(value) => value.to_variant(),
							GodotType::Nil           => Variant::default()
						};
						values.insert(name, value);
					});
				let result = callback
					.assume_safe()
					.call_func(&[values.into_shared().to_variant()]);
				assert!(result.get_type() == VariantType::Dictionary);
				let mut values = HashMap::new();
				result
					.to_dictionary()
					.iter()
					.for_each(|(name, value)| {
						let value = match value.get_type() {
							VariantType::Bool        => GodotType::Bool(value.to_bool()),
							VariantType::I64         => GodotType::I64(value.to_i64()),
							VariantType::F64         => GodotType::F64(value.to_f64()),
							VariantType::GodotString => GodotType::String(value.to_godot_string().to_string()),
							_                        => GodotType::Nil
						};
						values.insert(name.to_godot_string().to_string(), value);
					});
				values
			}
		}));
	}

	#[export]
	fn send_p2p_message(&self, _owner: &Node, message: String, parameters: Dictionary) {
		thread::spawn(move || {
			let mut values = HashMap::new();
			parameters
				.iter()
				.for_each(|(name, value)| {
					let value = match value.get_type() {
						VariantType::Bool        => GodotType::Bool(value.to_bool()),
						VariantType::I64         => GodotType::I64(value.to_i64()),
						VariantType::F64         => GodotType::F64(value.to_f64()),
						VariantType::GodotString => GodotType::String(value.to_godot_string().to_string()),
						_                        => GodotType::Nil
					};
					values.insert(name.to_godot_string().to_string(), value);
				});
			let (message, parameters) = sync_p2p_message(message, values).unwrap();
			let values = Dictionary::new(); 
			parameters
				.iter()
				.for_each(|(name, value)| {
					let value = match value {
						GodotType::Bool(value)   => value.to_variant(),
						GodotType::I64(value)    => value.to_variant(),
						GodotType::F64(value)    => value.to_variant(),
						GodotType::String(value) => value.to_variant(),
						GodotType::Nil           => Variant::default()
					};
					values.insert(name, value);
				});
			push_event("p2p_message_reply", vec![message.to_variant(), values.into_shared().to_variant()]);
		});
	}
}

fn init(handle: InitHandle) {
    handle.add_class::<Kabletop>();
	init_panic_hook();
}

godot_init!(init);
