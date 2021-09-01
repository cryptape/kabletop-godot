use std::{
	collections::HashMap, sync::Mutex
};
use crate::p2p::GodotType;

lazy_static! {
	pub static ref GODOT_CACHE: Mutex<GodotCache> = Mutex::new(GodotCache::default());
}

// a cache to store variables for gdscript
pub struct GodotCache {
	pub callbacks: HashMap<String, Box<dyn Fn(HashMap<String, GodotType>) -> HashMap<String, GodotType> + Send + 'static>>
}

impl Default for GodotCache {
	fn default() -> Self {
		GodotCache {
			callbacks: HashMap::new()
		}
	}
}

pub fn set_godot_callback<F>(message: &str, callback: Box<F>)
where 
	F: Fn(HashMap<String, GodotType>) -> HashMap<String, GodotType> + Send + 'static
{
	let message = String::from(message);
	let mut godot = GODOT_CACHE.lock().unwrap();
	if let Some(value) = godot.callbacks.get_mut(&message) {
		*value = callback;
	} else {
		godot.callbacks.insert(message, callback);
	}
}

pub fn unset_godot_callback(message: &str) {
	let mut godot = GODOT_CACHE.lock().unwrap();
	godot.callbacks.remove(&String::from(message));
}
