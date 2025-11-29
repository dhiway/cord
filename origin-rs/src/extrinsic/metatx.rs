use super::builder::DynamicCall;
use crate::{
	client::{
		connection::Connection,
		signer::{Signer, SubxtSignerAdapter},
	},
	config::{build_origin_params, OriginConfig},
	tx::{
		handle::{TxHandle, TxOutcome},
		meta::{
			assemble_meta_tx, build_meta_tx_bare_ext, meta_tx_sign_payload,
			meta_tx_value_from_signed, SignedMetaTx, META_TX_VERSION,
		},
	},
	types::error::OriginSdkError,
};
use tracing::debug;
use scale_value::{Value, ValueDef};
use std::sync::Arc;
use subxt::{config::DefaultExtrinsicParamsBuilder, tx::Payload};

/// Meta-transaction helper (signer + relayer flows).
#[derive(Clone)]
pub struct MetaTxClient {
	connection: Arc<Connection>,
	signer: Option<Arc<dyn Signer>>,
}

impl MetaTxClient {
	pub(crate) fn new(connection: Arc<Connection>, signer: Option<Arc<dyn Signer>>) -> Self {
		Self { connection, signer }
	}

	/// Attach a signer without rebuilding the Origin client.
	pub fn using<S>(&self, signer: S) -> Self
	where
		S: Signer + 'static,
	{
		Self { connection: self.connection.clone(), signer: Some(Arc::new(signer)) }
	}

	/// Build, sign, and return a meta-transaction blob suitable for offline relay.
	pub async fn prepare_and_sign(
		&self,
		call: DynamicCall,
	) -> Result<SignedMetaTx, OriginSdkError> {
		let signer = self.signer.clone().ok_or_else(|| {
			OriginSdkError::InvalidInput("meta-tx signer required for prepare_and_sign".into())
		})?;
		self.prepare_and_sign_with(call, signer).await
	}

	/// Same as `prepare_and_sign` but enables metadata-hash checking with the provided hash.
	pub async fn prepare_and_sign_with_metadata(
		&self,
		call: DynamicCall,
		metadata_hash: [u8; 32],
	) -> Result<SignedMetaTx, OriginSdkError> {
		let signer = self.signer.clone().ok_or_else(|| {
			OriginSdkError::InvalidInput(
				"meta-tx signer required for prepare_and_sign_with_metadata".into(),
			)
		})?;
		self.prepare_and_sign_with_metadata_hash(call, signer, Some(metadata_hash))
			.await
	}

	/// Same as `prepare_and_sign` but with an explicit signer (useful for wallets).
	pub async fn prepare_and_sign_with(
		&self,
		call: DynamicCall,
		signer: Arc<dyn Signer>,
	) -> Result<SignedMetaTx, OriginSdkError> {
		self.prepare_and_sign_with_metadata_hash(call, signer, None).await
	}

	/// Build & sign a meta-tx, optionally supplying a metadata hash (enables CheckMetadataHash).
	pub async fn prepare_and_sign_with_metadata_hash(
		&self,
		call: DynamicCall,
		signer: Arc<dyn Signer>,
		metadata_hash: Option<[u8; 32]>,
	) -> Result<SignedMetaTx, OriginSdkError> {
		let payload = self.dynamic_payload(&call);
		let call_value = payload.clone().into_value();
		let call_bytes = payload
			.encode_call_data(&self.connection.metadata())
			.map_err(|e| OriginSdkError::Encode(e.to_string()))?;

		// Meta-signer nonce drives replay protection for the inner meta-tx.
		let nonce = self
			.connection
			.online()
			.tx()
			.account_nonce(&signer.account_id())
			.await
			.map_err(|e| OriginSdkError::Nonce(e.to_string()))?;

		let bare_ext =
			build_meta_tx_bare_ext(self.connection.online(), nonce, metadata_hash).await?;
		let sign_bytes = meta_tx_sign_payload(META_TX_VERSION, &call_bytes, &bare_ext);
		debug!(
			target: "origin-sdk::meta-tx",
			"meta-tx preimage blake2_256={:?} call_len={} implicit_len={} mode={:?}",
			sp_core::blake2_256(&sign_bytes),
			call_bytes.len(),
			bare_ext.implicit_bytes().len(),
			bare_ext.metadata
		);
		let signature = signer.sign_payload(&sign_bytes).await;

		let meta_tx = assemble_meta_tx(
			META_TX_VERSION,
			&call_bytes,
			call_value,
			bare_ext,
			&signer.account_id(),
			&signature,
		);

		Ok(meta_tx)
	}

	/// Submit a previously signed meta-transaction using the configured signer as relayer.
	pub async fn submit_signed(&self, signed: SignedMetaTx) -> Result<TxHandle, OriginSdkError> {
		let relayer = self.signer.clone().ok_or_else(|| {
			OriginSdkError::InvalidInput("relayer signer required for submit_signed".into())
		})?;
		self.submit_signed_with(signed, relayer).await
	}

	/// Submit a signed meta-transaction, charging fees to `relayer`.
	pub async fn submit_signed_with(
		&self,
		signed: SignedMetaTx,
		relayer: Arc<dyn Signer>,
	) -> Result<TxHandle, OriginSdkError> {
		let dispatch_call = subxt::dynamic::tx(
			"MetaTx",
			"dispatch",
			vec![meta_tx_value_from_signed(&self.connection.metadata(), &signed)?],
		);

		let nonce = self
			.connection
			.online()
			.tx()
			.account_nonce(&relayer.account_id())
			.await
			.map_err(|e| OriginSdkError::Nonce(e.to_string()))?;

		let params =
			build_origin_params(DefaultExtrinsicParamsBuilder::<OriginConfig>::new().nonce(nonce));

		let adapter = SubxtSignerAdapter::new(relayer);
		let progress = self
			.connection
			.online()
			.tx()
			.sign_and_submit_then_watch(&dispatch_call, &adapter, params)
			.await
			.map_err(|e| OriginSdkError::Tx(e.to_string()))?;
		Ok(TxHandle::from_progress(progress))
	}

	/// Convenience: meta-signer signs, then the same account relays and pays fees.
	pub async fn sign_and_submit(&self, call: DynamicCall) -> Result<TxHandle, OriginSdkError> {
		let signed = self.prepare_and_sign(call).await?;
		self.submit_signed(signed).await
	}

	/// Convenience: wait for inclusion and ensure MetaTx::Dispatched == Ok.
	pub async fn sign_submit_and_wait_checked(
		&self,
		call: DynamicCall,
	) -> Result<TxOutcome, OriginSdkError> {
		let handle = self.sign_and_submit(call).await?;
		let outcome = handle.wait_finalized().await?;
		self.ensure_dispatched_ok(&outcome)?;
		Ok(outcome)
	}

	fn dynamic_payload(&self, call: &DynamicCall) -> subxt::tx::DynamicPayload {
		subxt::dynamic::tx(call.pallet.as_str(), call.function.as_str(), call.args.clone())
	}

	fn ensure_dispatched_ok(&self, outcome: &TxOutcome) -> Result<(), OriginSdkError> {
		if let Some(ev) = outcome
			.events
			.iter()
			.find(|e| e.pallet == "MetaTx" && e.variant == "Dispatched")
		{
			if let Some(first) = ev.fields.get(0) {
				match decode_dispatch_result(first) {
					Ok(true) => {},
					Ok(false) =>
						return Err(OriginSdkError::MetaTx("meta-tx dispatched with error".into())),
					Err(e) => return Err(OriginSdkError::MetaTx(format!("meta-tx decode: {e}"))),
				}
			}
		}
		Ok(())
	}
}

fn decode_dispatch_result(v: &Value) -> Result<bool, String> {
	if let Value { value: ValueDef::Variant(var), .. } = v {
		return Ok(var.name == "Ok");
	}
	Err("unexpected dispatch result shape".into())
}
