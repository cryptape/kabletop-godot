use gdnative::prelude::*;
use gdnative::api::*;
use kabletop_godot_sdk::{
	lua::highlevel::Lua, cache, ckb::*, USE_GODOT, p2p::{
		client, server, protocol_relay::methods::reply::hook as relay_hook, protocol::{
			methods::reply::hook, types::GodotType
		}
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
		*USE_GODOT.lock().unwrap() = true;
		// set hooks
		hook::add("sync_operation", |operation| {
			let value = String::from_utf8(operation.clone()).unwrap();
			run_code(value, true);
		});
		hook::add("switch_round", |signature| {
			randomseed(signature);
			persist_kabletop_cache();
		});
		hook::add("open_kabletop_channel", |hash| {
			let store = cache::get_clone();
			let lua = Lua::new(0, 0);
			lua.inject_nfts(from_nfts(store.opponent_nfts.clone()), from_nfts(store.user_nfts.clone()));
			lua.boost(get_lua_entry());
			set_lua(lua);
			randomseed(&store.script_hash);
			push_event("channel_status", vec![true.to_variant(), hex::encode(hash).to_variant()]);
			persist_kabletop_cache();
		});
		hook::add("close_kabletop_channel", |hash| {
			push_event("channel_status", vec![false.to_variant(), hex::encode(hash).to_variant()]);
			remove_kabletop_cache(hex::encode(cache::get_clone().script_hash));
		});
		relay_hook::add("propose_connection", |_| {
			push_event("connect_status", vec!["PARTNER".to_variant(), true.to_variant()]);
		});
		relay_hook::add("partner_disconnect", |_| {
			push_event("connect_status", vec!["PARTNER".to_variant(), false.to_variant()]);
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
            name: "channel_status",
            args: &[
				SignalArgument {
					name: "status",
					default: Variant::default(),
					export_info: ExportInfo::new(VariantType::Bool),
					usage: PropertyUsage::DEFAULT
				},
				SignalArgument {
					name: "hash",
					default: Variant::default(),
					export_info: ExportInfo::new(VariantType::GodotString),
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
		set_godot_emitor(owner.claim());
		// update_owned_nfts();
		update_box_status();
    }

	#[export]
	fn _process(&mut self, _owner: &Node, delta: f32) {
		if let Ok(mut events) = EVENTS.try_lock() {
			if let Some(emitor) = get_godot_emitor() {
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
	fn set_round(&self, _owner: &Node, round: u8, actor: u8) {
		cache::set_round_status(round, actor);
	}

	#[export]
	fn get_ckb(&mut self, _owner: &Node) -> u64 {
		online_capacity().unwrap()
	}

	#[export]
	fn set_selected_nfts(&mut self, _owner: &Node, nfts: Dictionary) {
		self.nfts = nfts
			.iter()
			.map(|(nft, count)| vec![nft.to_string(); count.to_u64() as usize])
			.collect::<Vec<_>>()
			.concat();
	}

	#[export]
	fn get_selected_nfts(&self, _owner: &Node) -> Dictionary {
		into_dictionary(&self.nfts)
	}

	#[export]
	fn get_selected_nfts_count(&self, _owner: &Node, player_id: u8) -> usize {
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
		let nfts = from_dictionary(nfts);
		if !nfts.is_empty() {
			discard_nfts(nfts, handle_transaction(update_owned_nfts, callback));
		}
	}

	#[export]
	fn transfer_nfts(&self, _owner: &Node, nfts: Dictionary, to: String, callback: Ref<FuncRef>) {
		let nfts = from_dictionary(nfts);
		if !nfts.is_empty() {
			transfer_nfts(nfts, to, handle_transaction(update_owned_nfts, callback));
		}
	}

	#[export]
	fn issue_nfts(&self, _owner: &Node, nfts: Dictionary, callback: Ref<FuncRef>) {
		let nfts = from_dictionary(nfts);
		if !nfts.is_empty() {
			issue_nfts(nfts, handle_transaction(update_owned_nfts, callback));
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
	fn get_owned_nfts(&self, _owner: &Node, from_cache: bool) -> Option<Variant> {
		if from_cache {
			(*NFTS.lock().unwrap()).clone()
		} else {
			update_owned_nfts();
			None
		}
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
	fn get_cache(&self, _owner: &Node) -> Dictionary {
		let clone = cache::get_clone();
		let value = Dictionary::new();
		value.insert("staking_ckb", clone.staking_ckb);
		value.insert("bet_ckb", clone.bet_ckb);
		value.insert("script_hash", hex::encode(clone.script_hash));
		value.insert("tx_hash", hex::encode(clone.channel_hash));
		value.insert("capacity", clone.capacity);
		value.insert("max_nfts_count", clone.max_nfts_count);
		value.insert("user_nfts", clone.user_nfts.iter().map(|v| hex::encode(v)).collect::<Vec<_>>());
		value.insert("opponent_nfts", clone.opponent_nfts.iter().map(|v| hex::encode(v)).collect::<Vec<_>>());
		value.insert("user_pkhash", hex::encode(clone.user_pkhash));
		value.insert("opponent_pkhash", hex::encode(clone.opponent_pkhash));
		value.insert("winner", clone.winner);
		value.insert("round", clone.round);
		value.insert("round_owner", clone.round_owner);
		value.insert("user_type", clone.user_type);
		value.insert("opponent_type", clone.opponent_type);
		value.insert("round_operations", clone.round_operations);
		value.into_shared()
	}

	#[export]
	fn connect_to(&self, _owner: &Node, socket: String) -> Variant {
		let result = client::connect(socket.as_str(), || {
			// unset_lua();
			push_event("connect_status", vec!["CLIENT".to_variant(), false.to_variant()]);
		});
		if let Err(err) = result {
			return err.to_variant();
		}
		set_p2p_mode(P2pMode::Client);
		push_event("connect_status", vec!["CLIENT".to_variant(), true.to_variant()]);
		Variant::default()
	}

	#[export]
	fn listen_at(&self, _owner: &Node, socket: String, staking_ckb: u64, bet_ckb: u64) -> Variant {
		if self.nfts.len() == 0 {
			return "empty nfts".to_variant();
		}
		cache::init(cache::PLAYER_TYPE::TWO);
		cache::set_staking_and_bet_ckb(staking_ckb, bet_ckb);
		cache::set_playing_nfts(into_nfts(self.nfts.clone()));
		let result = server::listen(socket.as_str(), move |id, connected_receivers| {
			if let Some(receivers) = connected_receivers {
				server::change_client(id);
				server::set_client_receivers(id, receivers);
				push_event("connect_status", vec!["SERVER".to_variant(), true.to_variant()]);
			} else {
				// unset_lua();
				push_event("connect_status", vec!["SERVER".to_variant(), false.to_variant()]);
			}
		});
		if let Err(err) = result {
			return err.to_variant();
		}
		set_p2p_mode(P2pMode::Server);
		Variant::default()
	}

	#[export]
	fn shutdown(&self, _owner: &Node) {
		disconnect().unwrap();
	}

	#[export]
	fn create_channel(&self, _owner: &Node, staking_ckb: u64, bet_ckb: u64, callback: Ref<FuncRef>) {
		if self.nfts.len() == 0 {
			FUNCREFS.lock().unwrap().push((callback.clone(), vec![false.to_variant(), "empty nfts".to_variant()]));
		}
		if get_p2p_mode() != P2pMode::Client {
			FUNCREFS.lock().unwrap().push((callback.clone(), vec![false.to_variant(), "no client mode".to_variant()]));
		}
		cache::init(cache::PLAYER_TYPE::ONE);
		cache::set_staking_and_bet_ckb(staking_ckb, bet_ckb);
		cache::set_playing_nfts(into_nfts(self.nfts.clone()));
		thread::spawn(move || match client::open_kabletop_channel() {
			Ok(hash) => {
				// create lua vm
				let clone = cache::get_clone();
				let lua = Lua::new(0, 0);
				lua.inject_nfts(from_nfts(clone.user_nfts.clone()), from_nfts(clone.opponent_nfts.clone()));
				lua.boost(get_lua_entry());
				set_lua(lua);

				// set first randomseed and callback to gdscript
				randomseed(&clone.script_hash);
				FUNCREFS.lock().unwrap().push((callback, vec![true.to_variant(), hex::encode(hash).to_variant()]));
				persist_kabletop_cache();
			},
			Err(err) => {
				FUNCREFS.lock().unwrap().push((callback.clone(), vec![false.to_variant(), err.to_variant()]));
			}
		});
	}

	#[export]
	fn close_channel(&self, _owner: &Node, from_challenge: bool, callback: Ref<FuncRef>) {
		let store = cache::get_clone();
		thread::spawn(move || {
			if from_challenge {
				let winner = match store.winner {
					0 => store.user_type,
					_ => store.winner
				};
				let (signed_rounds, from_challenge) = match complete_signed_rounds_for_challenge() {
					Ok(value) => value,
					Err(err)  => {
						FUNCREFS.lock().unwrap().push((callback, vec![false.to_variant(), err.to_variant()]));
						return
					}
				};
				let script_hash = store.script_hash;
				close_challenged_kabletop_channel(
					store.script_args, winner, from_challenge, signed_rounds, handle_transaction(move || {
						remove_kabletop_cache(hex::encode(script_hash));
					}, callback)
				)
			} else {
				match close_kabletop_channel() {
					Ok(hash) => {
						remove_kabletop_cache(hex::encode(store.script_hash));
						FUNCREFS.lock().unwrap().push((callback, vec![true.to_variant(), hex::encode(hash).to_variant()]));
					},
					Err(err) => {
						FUNCREFS.lock().unwrap().push((callback, vec![false.to_variant(), err.to_variant()]));
					}
				}
			}
		});
	}

	#[export]
	fn challenge_channel(&self, _: &Node, script_hash: String, callback: Ref<FuncRef>) {
		persist_kabletop_cache();
		match cache::recover(script_hash.clone()) {
			Ok(store) => {
				let signed_rounds = match complete_signed_rounds_for_challenge() {
					Ok((rounds, _)) => rounds,
					Err(err) => {
						FUNCREFS.lock().unwrap().push((callback, vec![false.to_variant(), err.to_variant()]));
						return
					}
				};
				challenge_kabletop_channel(
					store.script_args, store.user_type, dump_cached_codes(false), signed_rounds, handle_transaction(|| {
						dump_cached_codes(true)
							.into_iter()
							.for_each(|code| cache::commit_user_operation(code));
						persist_kabletop_cache();
						remove_cached_codes();
					}, callback)
				);
			},
			Err(err) => {
				FUNCREFS.lock().unwrap().push((callback, vec![false.to_variant(), err.to_string().to_variant()]));
			}
		}
	}

	#[export]
	fn get_uncomplete_kabletop_caches(&self, _owner: &Node, callback: Ref<FuncRef>) {
		thread::spawn(move || match scan_uncomplete_kabletop_cache() {
			Ok(caches) => {
				FUNCREFS.lock().unwrap().push((callback, vec![true.to_variant(), caches.to_variant()]));
			},
			Err(error) => {
				FUNCREFS.lock().unwrap().push((callback, vec![false.to_variant(), error.to_variant()]));
			}
		});
	}

	#[export]
	fn close_game(&self, _owner: &Node, winner: u8, from_challenge: bool, callback: Ref<FuncRef>) {
		cache::set_winner(winner);
		let store = cache::get_clone();
		if store.round_owner == store.user_type && !from_challenge {
			thread::spawn(move || {
				if let Err(error) = notify_game_over() {
					FUNCREFS.lock().unwrap().push((callback, vec![false.to_variant(), error.to_variant()]));
				} else {
					FUNCREFS.lock().unwrap().push((callback, vec![true.to_variant(), winner.to_variant()]));
				}
			});
		} else {
			unsafe { callback.assume_safe().call_func(&[true.to_variant(), winner.to_variant()]); }
		}
	}

	#[export]
	fn sync(&self, _owner: &Node, terminal: bool, callback: Ref<FuncRef>) {
		let codes = dump_cached_codes(true);
		thread::spawn(move || {
			for code in codes {
				if let Err(error) = sync_operation(code) {
					FUNCREFS.lock().unwrap().push((callback, vec![false.to_variant(), error.to_variant()]));
					return
				}
			}
			if terminal {
				match switch_round() {
					Ok(signature) => {
						remove_cached_codes();
						randomseed(&signature);
						FUNCREFS.lock().unwrap().push((callback, vec![true.to_variant(), Variant::default()]));
						persist_kabletop_cache();
					},
					Err(error) => {
						FUNCREFS.lock().unwrap().push((callback, vec![false.to_variant(), error.to_variant()]));
					}
				}
			} else {
				persist_kabletop_cache();
				FUNCREFS.lock().unwrap().push((callback, vec![true.to_variant(), Variant::default()]));
			}
		});
	}

	#[export]
	fn run(&self, _owner: &Node, code: String, effective: bool) {
		run_code(code.clone(), true);
		if effective {
			CODES.lock().unwrap().push((code, false));
		}
	}

	#[export]
	fn replay(&self, _owner: &Node, script_hash: String) -> Variant {
		match replay_kabletop_cache(script_hash) {
			Ok(_)    => Variant::default(),
			Err(err) => err.to_variant()
		}
	}

	#[export]
	fn reply_p2p_message(&self, _owner: &Node, message: String, callback: Ref<FuncRef>) {
		let name = message.clone();
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
				if result.get_type() != VariantType::Dictionary {
					panic!("variant {:?} isn't dictionary in p2p_message {}", result, name);
				}
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
			let (message, parameters) = sync_p2p_message(message.clone(), values).expect(format!("sync {}", message).as_str());
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

	#[export]
	fn register_relay(&self, _owner: &Node, nickname: String, staking_ckb: u64, bet_ckb: u64) -> Variant {
		if self.nfts.len() == 0 {
			return "empty nfts".to_variant();
		}
		if let Err(error) = register_client(nickname, staking_ckb, bet_ckb) {
			error.to_variant()
		} else {
			cache::init(cache::PLAYER_TYPE::TWO);
			cache::set_staking_and_bet_ckb(staking_ckb, bet_ckb);
			cache::set_playing_nfts(into_nfts(self.nfts.clone()));
			Variant::default()
		}
	}

	#[export]
	fn unregister_relay(&self, _owner: &Node) -> Variant {
		if let Err(error) = unregister_client() {
			error.to_variant()
		} else {
			cache::clear();
			Variant::default()
		}
	}

	#[export]
	fn connect_client_via_relay(&self, _: &Node, client_id: i32, nickname: String, staking_ckb: u64, bet_ckb: u64) -> Variant {
		if let Err(error) = connect_client(client_id, nickname, staking_ckb, bet_ckb) {
			error.to_variant()
		} else {
			Variant::default()
		}
	}

	#[export]
	fn disconnect_client_via_replay(&self, _owner: &Node) -> Variant {
		if let Err(error) = disconnect_client() {
			error.to_variant()
		} else {
			Variant::default()
		}
	}

	#[export]
	fn fetch_clients_from_relay(&self, _owner: &Node, callback: Ref<FuncRef>) {
		thread::spawn(move || match fetch_clients() {
			Err(error)  => FUNCREFS.lock().unwrap().push((callback, vec![false.to_variant(), error.to_variant()])),
			Ok(clients) => {
				let array = VariantArray::new();
				for client in clients {
					let value = Dictionary::new();
					value.insert("id",          client.id);
					value.insert("nickname",    client.nickname);
					value.insert("staking_ckb", client.staking_ckb);
					value.insert("bet_ckb",     client.bet_ckb);
					array.push(value.into_shared());
				}
				FUNCREFS.lock().unwrap().push((callback, vec![true.to_variant(), array.into_shared().to_variant()]))
			}
		});
	}
}

fn init(handle: InitHandle) {
    handle.add_class::<Kabletop>();
	init_panic_hook();
}

godot_init!(init);
