use crate::{error::Error, origin_client::OriginClient};
use scale_value::Value;
use subxt::{dynamic::DecodedValue, ext::scale_decode::DecodeAsType};

/// Thin wrapper to enforce view-only reads for consumers.
#[derive(Clone)]
pub struct ViewApi(pub(crate) OriginClient);

impl ViewApi {
	pub async fn call<T: DecodeAsType + serde::de::DeserializeOwned + 'static>(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<T, Error> {
		self.0.call_view_typed(pallet, function, args).await
	}

	pub async fn raw(
		&self,
		pallet: &str,
		function: &str,
		args: Value,
	) -> Result<DecodedValue, Error> {
		self.0.call_view(pallet, function, args).await
	}
}
