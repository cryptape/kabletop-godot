use std::{
	collections::HashMap, sync::Mutex
};
use crate::p2p::GodotType;

lazy_static! {
	pub static ref GODOT_CACHE: Mutex<GodotCache> = Mutex::new(GodotCache::default());
}

// a cache to store variables for gdscript
pub struct GodotCache {
	pub callbacks: HashMap<String, Box<dyn Fn(String, HashMap<String, GodotType>) -> HashMap<String, GodotType> + Send + 'static>>
}

impl Default for GodotCache {
	fn default() -> Self {
		GodotCache {
			callbacks: HashMap::new()
		}
	}
}

pub fn set_godot_callback<F>(message: String, callback: Box<F>)
where 
	F: Fn(String, HashMap<String, GodotType>) -> HashMap<String, GodotType> + Send + 'static
{
	let mut godot = GODOT_CACHE.lock().unwrap();
	if let Some(value) = godot.callbacks.get_mut(&message) {
		*value = callback;
	} else {
		godot.callbacks.insert(message, callback);
	}
}
