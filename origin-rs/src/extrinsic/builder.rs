use scale_value::Composite;
use subxt::dynamic::Value;

pub type DynamicTxPayload = subxt::tx::DynamicPayload<Composite<()>>;

/// Structured call descriptor.
#[derive(Clone, Debug)]
pub struct DynamicCall {
	pub pallet: String,
	pub function: String,
	pub args: Vec<Value>,
}

impl DynamicCall {
	/// Convert into a Subxt dynamic payload (re-usable across helpers).
	pub fn to_payload(&self) -> DynamicTxPayload {
		subxt::dynamic::tx(
			self.pallet.as_str(),
			self.function.as_str(),
			Composite::Unnamed(self.args.clone()),
		)
	}
}

/// Fluent builder for dynamic extrinsics.
pub struct DynamicCallBuilder;

impl DynamicCallBuilder {
	pub fn new() -> Self {
		Self
	}

	pub fn call(&self, pallet: &str, function: &str, args: Vec<Value>) -> DynamicCall {
		DynamicCall { pallet: pallet.into(), function: function.into(), args }
	}
}
