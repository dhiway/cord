use subxt::dynamic;

/// Structured call descriptor.
#[derive(Clone, Debug)]
pub struct DynamicCall {
	pub pallet: String,
	pub function: String,
	pub args: Vec<dynamic::Value>,
}

/// Fluent builder for dynamic extrinsics.
pub struct DynamicCallBuilder;

impl DynamicCallBuilder {
	pub fn new() -> Self {
		Self
	}

	pub fn call(
		&self,
		pallet: &str,
		function: &str,
		args: Vec<dynamic::Value>,
	) -> DynamicCall {
		DynamicCall { pallet: pallet.into(), function: function.into(), args }
	}
}
