mod protocol;

pub mod server;
pub mod client;
pub use protocol::{
	reply::hook, GodotType
};