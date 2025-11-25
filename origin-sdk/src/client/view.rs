use std::sync::Arc;

use codec::{Decode, Encode};
use sp_runtime::traits::SaturatedConversion;

use super::{connection::Connection, signer::Signer};
use crate::types::{auth, error::OriginSdkError};
use scale_value::Composite;

type Auth = origin_primitives::Authorization<
	origin_primitives::AccountId,
	Vec<u8>,
	origin_primitives::Signature,
>;

/// Minimal view caller: encodes args, hits the pallet view, and decodes into a concrete type.
#[derive(Clone)]
pub struct ViewClient {
	connection: Arc<Connection>,
}

impl ViewClient {
	pub(crate) fn new(connection: Arc<Connection>) -> Self {
		Self { connection }
	}

	/// Generic view invocation: caller is responsible for providing the exact arguments
	/// (including Authorization when required). The result is decoded directly into `T`.
	pub async fn call<T>(
		&self,
		pallet: &str,
		function: &str,
		args: Vec<Vec<u8>>,
	) -> Result<T, OriginSdkError>
	where
		T: Decode,
	{
		let metadata = self.connection.metadata();
		let pallet_meta = metadata
			.pallet_by_name(pallet)
			.ok_or_else(|| OriginSdkError::View(format!("pallet {pallet} not found")))?;
		let vf = pallet_meta
			.view_functions()
			.find(|vf| vf.name() == function)
			.ok_or_else(|| OriginSdkError::View(format!("view {pallet}.{function} not found")))?;

		let query_id = *vf.query_id();
		let inputs: Vec<_> = vf.inputs().collect();
		if inputs.len() != args.len() {
			return Err(OriginSdkError::InvalidInput(format!(
				"expected {} args, got {}",
				inputs.len(),
				args.len()
			)));
		}

		let mut values = Vec::with_capacity(args.len());
		for (bytes, input) in args.into_iter().zip(inputs) {
			let mut cursor = &bytes[..];
			let val = scale_value::scale::decode_as_type(&mut cursor, input.ty, metadata.types())
				.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
			values.push(val.remove_context());
		}

		let comp = Composite::unnamed(values);
		let payload = subxt::dynamic::view_function_call(query_id, comp);

		let api = self
			.connection
			.online()
			.view_functions()
			.at_latest()
			.await
			.map_err(|e| OriginSdkError::View(e.to_string()))?;

		let thunk = api.call(payload).await.map_err(|e| OriginSdkError::View(e.to_string()))?;
		let bytes = thunk.into_encoded();

		T::decode(&mut &bytes[..])
			.map_err(|e| OriginSdkError::Decode(format!("{pallet}.{function} decode: {e}")))
	}

	/// Build the Authorization envelope expected by pallet view functions.
	pub async fn authorization_for<S: Signer>(
		&self,
		signer: &S,
		pallet: &str,
		function: &str,
	) -> Result<Auth, OriginSdkError> {
		let reference_block = self
			.connection
			.online()
			.blocks()
			.at_latest()
			.await
			.map_err(|e| OriginSdkError::View(e.to_string()))?
			.number()
			.saturated_into::<u32>();

		Ok(auth::build_authorization(signer, pallet, function, reference_block).await)
	}
}
