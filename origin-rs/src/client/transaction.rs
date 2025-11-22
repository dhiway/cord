use crate::{
	client::signer::{OriginSigner, SubxtSignerAdapter},
	error::Error,
	origin_client::OriginClient,
	params,
};
use subxt::{blocks::ExtrinsicEvents, dynamic, tx::TxProgress, OnlineClient};

pub type Progress = TxProgress<params::config::OriginConfig, OnlineClient<params::config::OriginConfig>>;
pub type FinalizedEvents = ExtrinsicEvents<params::config::OriginConfig>;

/// Describes a call to be batched (pallet + function + dynamic args).
#[derive(Clone, Debug)]
pub struct BatchCall {
	pub pallet: String,
	pub call: String,
	pub args: Vec<dynamic::Value>,
}

/// High-level extrinsic pipeline with nonce-safe submission and optional event resolution.
#[derive(Clone)]
pub struct TransactionClient {
	client: OriginClient,
}

impl TransactionClient {
	pub fn new(client: OriginClient) -> Self {
		Self { client }
	}

	pub async fn submit(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
		signer: &impl OriginSigner,
	) -> Result<Progress, Error> {
		let adapter = SubxtSignerAdapter::new(signer.clone());
		self.client.submit_dynamic_call(pallet, call, args, &adapter).await
	}

	pub async fn submit_and_wait(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
		signer: &impl OriginSigner,
	) -> Result<FinalizedEvents, Error> {
		let progress = self.submit(pallet, call, args, signer).await?;
		progress.wait_for_finalized_success().await.map_err(Error::from)
	}

	/// Submit and return all decoded events emitted by the extrinsic once finalized.
	pub async fn submit_and_collect_events(
		&self,
		pallet: &str,
		call: &str,
		args: Vec<dynamic::Value>,
		signer: &impl OriginSigner,
	) -> Result<Vec<crate::origin_client::DynamicEvent>, Error> {
		let progress = self.submit(pallet, call, args, signer).await?;
		let finalized = progress.wait_for_finalized_success().await.map_err(Error::from)?;
		let mut out = Vec::new();
		for ev in finalized.iter() {
			if let Ok(ev) = ev {
				if let Ok(fields) = ev.field_values() {
					out.push(crate::origin_client::DynamicEvent {
						block_hash: Default::default(),
						block_number: 0,
						pallet: ev.pallet_name().to_string(),
						variant: ev.variant_name().to_string(),
						fields,
					});
				}
			}
		}
		Ok(out)
	}

	/// Build a Utility.batch call from the provided calls and submit it.
	pub async fn submit_batch(
		&self,
		calls: Vec<BatchCall>,
		_signer: &impl OriginSigner,
	) -> Result<Progress, Error> {
		// Future improvement: encode nested dynamic calls once subxt exposes a helper for Call values.
		let _ = calls;
		Err(Error::Params(
			"batch extrinsics are not wired yet in the view-only client".into(),
		))
	}
}
