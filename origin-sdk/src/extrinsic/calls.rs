use subxt::dynamic::Value;

use super::builder::DynamicCall;

/// Strongly-typed helpers for common Origin pallet calls (dynamic payloads).
pub struct Calls;

impl Calls {
	pub fn entity_set_info(&self, args: Vec<Value>) -> DynamicCall {
		DynamicCall { pallet: "Entity".into(), function: "set_info".into(), args }
	}

	pub fn registry_create(&self, args: Vec<Value>) -> DynamicCall {
		DynamicCall { pallet: "Register".into(), function: "create".into(), args }
	}

	pub fn packet_issue(&self, args: Vec<Value>) -> DynamicCall {
		DynamicCall { pallet: "Packet".into(), function: "issue".into(), args }
	}
}
