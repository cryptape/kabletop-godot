use gdnative::prelude::*;
use kabletop_godot_util::{
	lua::highlevel::Lua, cache, lua, ckb::{
		client, hook
	}
};
use std::sync::Mutex;

lazy_static::lazy_static! {
	static ref EMITOR: Mutex<Option<Ref<Node>>> = Mutex::new(None);
	static ref LUA:    Mutex<Option<Lua>> = Mutex::new(None);
}

#[derive(NativeClass)]
#[inherit(Node)]
#[register_with(Self::register_signals)]
struct Kabletop {
	entry: String
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
		// instance kabletop godot object
        Kabletop {
			entry: String::new()
		}
    }

	fn register_signals(builder: &ClassBuilder<Self>) {
        builder.add_signal(Signal {
            name: "lua_events",
            args: &[SignalArgument {
                name: "events",
                default: Vec::<Variant>::new().to_variant(),
                export_info: ExportInfo::new(VariantType::VariantArray),
                usage: PropertyUsage::DEFAULT
            }]
        });
        builder.add_signal(Signal {
            name: "disconnect",
            args: &[]
        });
    }

    #[export]
    fn _ready(&self, owner: TRef<Node>) {
        godot_print!("welcome to the kabletop world!");
		*EMITOR.lock().unwrap() = Some(owner.claim());
    }

	#[export]
	fn set_entry(&mut self, _owner: &Node, entry: String) {
		self.entry = entry;
	}

	#[export]
	fn create_channel(&self, _owner: &Node, socket: String, nfts: Vec<String>) {
		cache::init(cache::PLAYER_TYPE::ONE);
		cache::set_playing_nfts(into_nfts(nfts));
		client::connect(socket.as_str(), || push_event("disconnect", None));
		client::open_kabletop_channel();

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
	}

	#[export]
	fn run(&self, _owner: &Node, code: String, terminal: bool) {
		run_code(code.clone());
		if terminal {
			client::switch_round();
		} else {
			client::sync_operation(code);
		}
	}
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
		push_event("lua_events", Some(events.to_variant()));
	}
}

fn push_event(name: &str, value: Option<Variant>) {
	let emitor = EMITOR.lock().unwrap();
	if let Some(value) = value {
		unsafe {
			emitor
				.as_ref()
				.unwrap()
				.assume_safe()
				.emit_signal(name, &[value]);
		}
	} else {
		unsafe {
			emitor
				.as_ref()
				.unwrap()
				.assume_safe()
				.emit_signal(name, &[]);
		}
	}
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