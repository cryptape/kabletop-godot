use gdnative::prelude::*;
use gdnative::api::*;
use kabletop_godot_util::{
	lua::highlevel::Lua, cache, ckb::*, p2p::{
		client, hook
	}
};
use std::thread;

mod helper;
use helper::*;

#[derive(NativeClass)]
#[inherit(Node)]
#[register_with(Self::register_signals)]
struct Kabletop {
	entry: String,
	nfts:  Vec<String>
}

#[gdnative::methods]
impl Kabletop {
    fn new(_owner: &Node) -> Self {
		// turn all println! to godot_print!
		*kabletop_godot_util::USE_GODOT.lock().unwrap() = true;
		// set hook
		hook::add("sync_operation", |operation| {
			let value = String::from_utf8(operation.clone()).unwrap();
			run_code(value);
		});
		hook::add("switch_round", |signature| {
			randomseed(signature);
		});
		// instance kabletop godot object
        Kabletop {
			entry: String::new(),
			nfts:  vec![]
		}
    }

	fn register_signals(builder: &ClassBuilder<Self>) {
        builder.add_signal(Signal {
            name: "disconnect",
            args: &[]
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
    }

    #[export]
    fn _ready(&mut self, owner: TRef<Node>) {
        godot_print!("welcome to the kabletop world!");
		*EMITOR.lock().unwrap() = Some(owner.claim());
		update_owned_nfts();
		update_box_status();
    }

	#[export]
	fn _process(&mut self, _owner: &Node, _delta: f32) {
		let mut events = EVENTS.lock().unwrap();
		for (name, value) in &*events {
			let emitor = EMITOR.lock().unwrap();
			if name == "owned_nfts_updated" {
				self.nfts = vec![];
			}
			unsafe {
				emitor
					.as_ref()
					.unwrap()
					.assume_safe()
					.emit_signal(name, value.as_slice());
			}
		}
		(*events).clear();
	}

	#[export]
	fn set_entry(&mut self, _owner: &Node, entry: String) {
		self.entry = entry;
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
			nfts.into_shared()
		} else {
			Dictionary::new_shared()
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
		} else {
			godot_print!("no cards selected");
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
	fn get_owned_nfts(&self, _owner: &Node) -> Option<Variant> {
		(*NFTS.lock().unwrap()).clone()
	}

	#[export]
	fn get_box_status(&self, _owner: &Node) -> Dictionary {
		let map = Dictionary::new();
		let status = *STATUS.lock().unwrap();
		map.insert("count", status.0.to_variant());
		map.insert("ready", status.1.to_variant());
		map.into_shared()
	}

	#[export]
	fn create_channel(&self, _owner: &Node, socket: String) -> bool {
		if self.nfts.len() == 0 {
			return false;
		}
		cache::init(cache::PLAYER_TYPE::ONE);
		cache::set_playing_nfts(into_nfts(self.nfts.clone()));
		client::connect(socket.as_str(), || push_event("disconnect", vec![]));
		let tx_hash = client::open_kabletop_channel();

		// create lua vm
		let clone = cache::get_clone();
		let mut ckb_time: i64 = 0;
		for i in 0..8 {
			ckb_time = (ckb_time << 8) | (clone.script_hash[i] as i64 >> 1);
		}
		let mut ckb_clock: i64 = 0;
		for i in 8..16 {
			ckb_clock = (ckb_clock << 8) | (clone.script_hash[i] as i64 >> 1);
		}
		let lua = Lua::new(ckb_time, ckb_clock);
		lua.inject_nfts(from_nfts(clone.user_nfts), from_nfts(clone.opponent_nfts));
		lua.boost(self.entry.clone());
		if let Some(old) = LUA.lock().unwrap().as_ref() {
			old.close();
		}
		*LUA.lock().unwrap() = Some(lua);

		// set first randomseed
		randomseed(&tx_hash);
		true
	}

	#[export]
	fn run(&self, _owner: &Node, code: String, terminal: bool) {
		run_code(code.clone());
		thread::spawn(move || {
			client::sync_operation(code);
			if terminal {
				let signature = client::switch_round();
				randomseed(&signature);
			}
		});
	}
}

fn init(handle: InitHandle) {
    handle.add_class::<Kabletop>();
	init_panic_hook();
}

godot_init!(init);
