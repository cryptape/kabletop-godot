use gdnative::prelude::*;
use kabletop_godot_util::{
	lua::highlevel::Lua, cache, lua, p2p::{
		client, hook
	}, ckb::{
		owned_nfts, discard_nfts, purchase_nfts, reveal_nfts, H256
	}
};
use std::{
	sync::Mutex, thread, convert::TryInto
};

lazy_static::lazy_static! {
	static ref EMITOR: Mutex<Option<Ref<Node>>> = Mutex::new(None);
	static ref EVENTS: Mutex<Vec<(String, Vec<Variant>)>> = Mutex::new(vec![]);
	static ref LUA:    Mutex<Option<Lua>> = Mutex::new(None);
	static ref NFTS:   Mutex<Option<Variant>> = Mutex::new(None);
}

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
            name: "owned_updated",
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
            name: "transaction_sent",
            args: &[
				SignalArgument {
					name: "uuid",
					default: Variant::from_u64(0),
					export_info: ExportInfo::new(VariantType::I64),
					usage: PropertyUsage::DEFAULT
				},
            	SignalArgument {
					name: "error",
					default: Variant::default(),
					export_info: ExportInfo::new(VariantType::GodotString),
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
    }

	#[export]
	fn _process(&mut self, _owner: &Node, _delta: f32) {
		let mut events = EVENTS.lock().unwrap();
		for (name, value) in &*events {
			let emitor = EMITOR.lock().unwrap();
			if name == "owned_updated" {
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
	fn delete_nfts(&mut self, _owner: &Node, nfts: Dictionary) -> u32 {
		let nfts = nfts
			.iter()
			.map(|(nft, count)| vec![nft.to_string(); count.to_u64() as usize])
			.collect::<Vec<_>>()
			.concat();
		let mut uuid = 0;
		if nfts.len() > 0 {
			uuid = discard_nfts(&nfts, handle_transaction(update_owned_nfts));
		} else {
			godot_print!("no cards selected");
		}
		uuid
	}

	#[export]
	fn purchase_nfts(&self, _owner: &Node, count: u8) -> u32 {
		purchase_nfts(count, handle_transaction(|| {}))
	}

	#[export]
	fn reveal_nfts(&self, _owner: &Node) -> u32 {
		reveal_nfts(handle_transaction(|| {}))
	}

	#[export]
	fn get_owned_nfts(&self, _owner: &Node) -> Option<Variant> {
		(*NFTS.lock().unwrap()).clone()
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

fn randomseed(seed: &[u8]) {
	let seed = {
		assert!(seed.len() >= 16);
		&seed[..16]
	};
	let seed_1 = i64::from_le_bytes(seed[..8].try_into().unwrap());
	let seed_2 = i64::from_le_bytes(seed[8..].try_into().unwrap());
	run_code(format!("math.randomseed({}, {})", seed_1, seed_2));
}

fn run_code(code: String) {
	let events = LUA
		.lock()
		.unwrap()
		.as_ref()
		.expect("no kabletop channel is opened")
		.run(code.clone())
		.iter()
		.map(|event| {
			let mut params = vec![];
			for field in event {
				match field {
					lua::ffi::lua_Event::Number(value)      => params.push(value.to_variant()),
					lua::ffi::lua_Event::String(value)      => params.push(value.to_variant()),
					lua::ffi::lua_Event::NumberTable(value) => params.push(value.to_variant())
				}
			}
			params
		})
		.collect::<Vec<Vec<_>>>();
	if events.len() > 0 {
		push_event("lua_events", vec![events.to_variant()]);
	}
}

fn handle_transaction<F: Fn() + 'static + Send>(f: F) -> Box<dyn Fn(u32, Result<H256, String>) + 'static + Send> {
	return Box::new(move |uuid: u32, result: Result<H256, String>| {
		match result {
			Ok(hash) => {
				godot_print!("uuid = {}, tx_hash = {:?}", uuid, hash);
				push_event("transaction_sent", vec![uuid.to_variant(), Variant::default()]);
				f();
			},
			Err(err) => {
				push_event("transaction_sent", vec![uuid.to_variant(), err.to_string().to_variant()]);
			}
		}
	})
}

fn update_owned_nfts() {
	thread::spawn(|| {
		let nfts = {
			let nfts = Dictionary::new();
			for (nft, count) in owned_nfts().expect("get owned nfts") {
				nfts.insert(nft, count.to_variant());
			}
			nfts.into_shared()
		};
		*NFTS.lock().unwrap() = Some(nfts.to_variant());
		push_event("owned_updated", vec![nfts.to_variant()]);
	});
}

fn push_event(name: &str, value: Vec<Variant>) {
	EVENTS
		.lock()
		.unwrap()
		.push((String::from(name), value));
}

fn into_nfts(value: Vec<String>) -> Vec<[u8; 20]> {
	value
		.iter()
		.map(|v| {
			let mut hash = [0u8; 20];
			let bytes = hex::decode(v).expect("decode blake160 hashcode");
			hash.clone_from_slice(bytes.as_slice());
			hash
		})
		.collect::<_>()
}

fn from_nfts(value: Vec<[u8; 20]>) -> Vec<String> {
	value
		.iter()
		.map(|v| hex::encode(v))
		.collect::<_>()
}

fn init_panic_hook() {
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let loc_string;
        if let Some(location) = panic_info.location() {
            loc_string = format!("file '{}' at line {}", location.file(), location.line());
        } else {
            loc_string = "unknown location".to_owned()
        }

        let error_message;
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            error_message = format!("[RUST] {}: panic occurred: {:?}", loc_string, s);
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            error_message = format!("[RUST] {}: panic occurred: {:?}", loc_string, s);
        } else {
            error_message = format!("[RUST] {}: unknown panic occurred", loc_string);
        }
        godot_error!("{}", error_message);
        (*(old_hook.as_ref()))(panic_info);

        unsafe {
            if let Some(gd_panic_hook) = gdnative::api::utils::autoload::<gdnative::api::Node>("rust_panic_hook") {
                gd_panic_hook.call("rust_panic_hook", &[GodotString::from_str(error_message).to_variant()]);
            }
        }
    }));
}

fn init(handle: InitHandle) {
    handle.add_class::<Kabletop>();
	init_panic_hook();
}

godot_init!(init);