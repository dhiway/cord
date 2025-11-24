use super::builder::DynamicCall;
use crate::{
	client::{connection::Connection, signer::Signer},
	types::error::OriginSdkError,
};
use sp_runtime::MultiSignature;
use std::sync::Arc;
use subxt::{tx::Payload as _, utils::AccountId32};

/// Prepared meta-transaction payload ready for signing or relaying.
#[derive(Clone, Debug)]
pub struct PreparedMetaTx {
	pub call: DynamicCall,
	pub payload: Vec<u8>,
}

/// Signed meta-transaction blob (call + detached signature + signer id).
#[derive(Clone, Debug)]
pub struct SignedMetaTx {
	pub call: DynamicCall,
	pub payload: Vec<u8>,
	pub signer: AccountId32,
	pub signature: MultiSignature,
}

/// Meta-transaction helper (scaffold).
#[derive(Clone)]
pub struct MetaTxClient {
	connection: Arc<Connection>,
	signer: Option<Arc<dyn Signer>>,
}

impl MetaTxClient {
	pub(crate) fn new(connection: Arc<Connection>, signer: Option<Arc<dyn Signer>>) -> Self {
		Self { connection, signer }
	}

	/// Wrap and encode a call, returning a payload suitable for detached signing.
	pub fn prepare(&self, call: DynamicCall) -> Result<PreparedMetaTx, OriginSdkError> {
		let wrapped = self.wrap(call)?;
		let payload = self.encode_call(&wrapped)?;
		Ok(PreparedMetaTx { call: wrapped, payload })
	}

	/// Sign a prepared payload with a supplied meta-signer.
	pub async fn sign_prepared_with(
		&self,
		prepared: PreparedMetaTx,
		signer: Arc<dyn Signer>,
	) -> Result<SignedMetaTx, OriginSdkError> {
		let signature = signer.sign_payload(&prepared.payload).await;
		Ok(SignedMetaTx {
			call: prepared.call,
			payload: prepared.payload,
			signer: signer.account_id(),
			signature,
		})
	}

	/// Sign a prepared payload using the SDK's configured signer.
	pub async fn sign_prepared(
		&self,
		prepared: PreparedMetaTx,
	) -> Result<SignedMetaTx, OriginSdkError> {
		let signer = self.signer.clone().ok_or_else(|| {
			OriginSdkError::InvalidInput("signer is required for meta-tx signing".into())
		})?;
		self.sign_prepared_with(prepared, signer).await
	}

	/// Wrap a call into MetaTx::submit(pallet, function, args_bytes).
	pub fn wrap(&self, call: DynamicCall) -> Result<DynamicCall, OriginSdkError> {
		let inner_value = subxt::dynamic::tx(call.pallet, call.function, call.args).into_value();
		let wrapped = DynamicCall {
			pallet: "MetaTx".into(),
			function: "submit".into(),
			args: vec![inner_value],
		};
		Ok(wrapped)
	}

	/// Prepare a payload for off-chain relay (currently returns the dynamic call descriptor).
	pub fn prepare_payload(&self, call: DynamicCall) -> Result<DynamicCall, OriginSdkError> {
		self.wrap(call)
	}

	/// Sign and submit a meta-transaction via MetaTx pallet using provided signer (meta key).
	pub async fn sign_and_submit_with(
		&self,
		call: DynamicCall,
		signer: Arc<dyn Signer>,
	) -> Result<crate::client::submit::TxHandle, OriginSdkError> {
		let wrapped = self.wrap(call)?;
		let payload = subxt::dynamic::tx(wrapped.pallet, wrapped.function, wrapped.args);
		let submit =
			crate::client::submit::SubmitClient::new(self.connection.clone(), Some(signer));
		submit.submit_payload(payload).await
	}

	/// Convenience: use the SDK signer to sign + submit locally.
	pub async fn sign_and_submit(
		&self,
		call: DynamicCall,
	) -> Result<crate::client::submit::TxHandle, OriginSdkError> {
		let signer = self.signer.clone().ok_or_else(|| {
			OriginSdkError::InvalidInput("signer is required for meta-tx submit".into())
		})?;
		self.sign_and_submit_with(call, signer).await
	}

	fn encode_call(&self, call: &DynamicCall) -> Result<Vec<u8>, OriginSdkError> {
		let payload =
			subxt::dynamic::tx(call.pallet.as_str(), call.function.as_str(), call.args.clone());
		payload
			.encode_call_data(&self.connection.metadata())
			.map_err(|e| OriginSdkError::Encode(e.to_string()))
	}
}
