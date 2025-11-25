use super::builder::DynamicCall;
use crate::{
	client::{connection::Connection, signer::Signer},
	types::error::OriginSdkError,
};
use scale_value::{Value, ValueDef};
use sp_runtime::MultiSignature;
use std::sync::Arc;
use subxt::tx::Payload as _;
/// Prepared meta-transaction payload ready for signing or relaying.
#[derive(Clone, Debug)]
pub struct PreparedMetaTx {
	pub call: DynamicCall,
	pub payload: Vec<u8>,
	pub nonce: u64,
	pub spec_version: u32,
	pub genesis_hash: subxt::utils::H256,
}

/// Signed meta-transaction blob (call + detached signature + signer id).
#[derive(Clone, Debug)]
pub struct SignedMetaTx {
	pub call: DynamicCall,
	pub payload: Vec<u8>,
	pub signer: origin_primitives::AccountId,
	pub signature: MultiSignature,
	pub nonce: u64,
	pub spec_version: u32,
	pub genesis_hash: subxt::utils::H256,
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
	pub async fn prepare(&self, call: DynamicCall) -> Result<PreparedMetaTx, OriginSdkError> {
		let signer = self.signer.clone().ok_or_else(|| {
			OriginSdkError::InvalidInput("meta-tx signer required for prepare".into())
		})?;
		let account = signer.account_id();
		self.prepare_with_account(call, account).await
	}

	/// Prepare including the account nonce (if provided) to improve replay safety.
	pub async fn prepare_with_account(
		&self,
		call: DynamicCall,
		account: origin_primitives::AccountId,
	) -> Result<PreparedMetaTx, OriginSdkError> {
		let wrapped = self.wrap(call)?;
		let mut payload = self.encode_call(&wrapped)?;
		let genesis_hash = self.connection.online().genesis_hash();
		let spec_version = self.connection.online().runtime_version().spec_version;
		payload.extend_from_slice(genesis_hash.as_ref());
		payload.extend_from_slice(&spec_version.to_le_bytes());

		let nonce = self
			.connection
			.online()
			.tx()
			.account_nonce(&account)
			.await
			.map_err(|e| OriginSdkError::MetaTx(e.to_string()))?;
		payload.extend_from_slice(&nonce.to_le_bytes());

		Ok(PreparedMetaTx { call: wrapped, payload, nonce, spec_version, genesis_hash })
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
			nonce: prepared.nonce,
			spec_version: prepared.spec_version,
			genesis_hash: prepared.genesis_hash,
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
		let submit = crate::client::submit::SubmitClient::new(self.connection.clone(), signer);
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

	/// Sign, submit, wait finalized, and assert MetaTx::Dispatched succeeded.
	pub async fn sign_submit_and_wait_checked(
		&self,
		call: DynamicCall,
	) -> Result<crate::client::submit::TxOutcome, OriginSdkError> {
		let handle = self.sign_and_submit(call).await?;
		let outcome = handle.wait_finalized().await?;
		self.ensure_dispatched_ok(&outcome)?;
		Ok(outcome)
	}

	/// Meta-signer signs, relayer submits/pays fees.
	pub async fn sign_and_submit_with_relayer(
		&self,
		call: DynamicCall,
		meta_signer: Arc<dyn Signer>,
		relayer: Arc<dyn Signer>,
	) -> Result<crate::client::submit::TxHandle, OriginSdkError> {
		let wrapped = self.wrap(call)?;
		let payload = subxt::dynamic::tx(wrapped.pallet, wrapped.function, wrapped.args.clone());
		let submit = crate::client::submit::SubmitClient::new(self.connection.clone(), relayer);
		let _signed = meta_signer
			.sign_payload(
				&payload
					.encode_call_data(&self.connection.metadata())
					.map_err(|e| OriginSdkError::Encode(e.to_string()))?,
			)
			.await;
		submit.submit_payload(payload).await
	}

	fn ensure_dispatched_ok(
		&self,
		outcome: &crate::client::submit::TxOutcome,
	) -> Result<(), OriginSdkError> {
		if let Some(ev) = outcome
			.events
			.iter()
			.find(|e| e.pallet == "MetaTx" && e.variant == "Dispatched")
		{
			if let Some(first) = ev.fields.get(0) {
				match decode_dispatch_result(first) {
					Ok(true) => {},
					Ok(false) => {
						return Err(OriginSdkError::MetaTx("meta-tx dispatched with error".into()))
					},
					Err(e) => return Err(OriginSdkError::MetaTx(format!("meta-tx decode: {e}"))),
				}
			}
		}
		Ok(())
	}

	fn encode_call(&self, call: &DynamicCall) -> Result<Vec<u8>, OriginSdkError> {
		let payload =
			subxt::dynamic::tx(call.pallet.as_str(), call.function.as_str(), call.args.clone());
		payload
			.encode_call_data(&self.connection.metadata())
			.map_err(|e| OriginSdkError::Encode(e.to_string()))
	}
}

fn decode_dispatch_result(v: &Value) -> Result<bool, String> {
	if let Value { value: ValueDef::Variant(var), .. } = v {
		return Ok(var.name == "Ok");
	}
	Err("unexpected dispatch result shape".into())
}
