use subxt::dynamic;
use super::builder::DynamicCall;

/// Strongly typed call helpers for Origin pallets (scaffold only).
pub struct Calls;

impl Calls {
	pub fn entity_set_info(
		&self,
		payload: Vec<dynamic::Value>,
	) -> DynamicCall {
		DynamicCall { pallet: "Entity".into(), function: "set_info".into(), args: payload }
	}
}
