use codec::Decode;
use color_eyre::eyre::{eyre, Context, Result};
use hex::encode as hex_encode;
use sp_core::{blake2_256, sr25519, Pair as CryptoPair};
use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidityError};
use std::sync::Arc;
use subxt::{
	backend::{
		legacy::rpc_methods::{DryRunDecodeError, DryRunResult, LegacyRpcMethods},
		rpc::RpcClient,
	},
	blocks::ExtrinsicEvents,
	config::DefaultExtrinsicParamsBuilder,
	tx::TxStatus,
	OnlineClient,
};
use url::Url;

use crate::{
	chain::{build_cord_params, CordConfig},
	pair_signer::PairSigner,
};

pub type Hash = subxt::utils::H256;

pub struct SubmitResult {
	pub block_hash: Hash,
	pub events: ExtrinsicEvents<CordConfig>,
}

const LOCAL_METADATA: &[u8] = include_bytes!("../metadata/cord.scale");

async fn ensure_metadata_alignment(rpc: &RpcClient) -> Result<()> {
	let remote = LegacyRpcMethods::<CordConfig>::new(rpc.clone())
		.state_get_metadata(None)
		.await
		.wrap_err("failed to download runtime metadata from node")?
		.into_raw();
	let remote_hash = blake2_256(&remote);
	let local_hash = blake2_256(LOCAL_METADATA);
	if remote_hash != local_hash {
		return Err(eyre!(
			"runtime metadata mismatch. local hash {LOCAL}, node hash {REMOTE}. \\nRun `cargo run -p cord-subxt-anchor --bin dump_metadata -- --output examples/subxt-anchor/metadata/cord.scale` and rebuild/restart the node to realign.",
			LOCAL = hex_encode(local_hash),
			REMOTE = hex_encode(remote_hash)
		));
	}
	Ok(())
}

pub struct ExampleContext {
	pub client: Arc<OnlineClient<CordConfig>>,
	rpc: Arc<RpcClient>,
	pub signer: PairSigner,
	pub account_id: subxt::utils::AccountId32,
}

impl ExampleContext {
	pub async fn connect(url: Url, suri: &str) -> Result<Self> {
		let rpc = RpcClient::from_insecure_url(url.as_str())
			.await
			.wrap_err_with(|| format!("failed to connect to {}", url))?;
		ensure_metadata_alignment(&rpc).await?;
		let client = OnlineClient::<CordConfig>::from_rpc_client(rpc.clone())
			.await
			.wrap_err("failed to initialize Subxt client")?;
		let pair = sr25519::Pair::from_string(suri, None).wrap_err("invalid signer seed")?;
		let signer = PairSigner::new(pair);
		let account_id = signer.account_id().clone();
		Ok(Self { client: Arc::new(client), rpc: Arc::new(rpc), signer, account_id })
	}

	async fn next_nonce(&self) -> Result<u64> {
		LegacyRpcMethods::<CordConfig>::new((*self.rpc).clone())
			.system_account_next_index(&self.account_id)
			.await
			.wrap_err("failed to fetch account nonce")
	}

	pub async fn submit<T>(&self, label: &str, payload: &T) -> Result<SubmitResult>
	where
		T: subxt::tx::Payload + Sync,
	{
		let mut attempts = 0;
		let mut explicit_nonce: Option<u64> = None;
		'outer: loop {
			let mut builder = DefaultExtrinsicParamsBuilder::<CordConfig>::new().tip(0u128);
			if let Some(nonce) = explicit_nonce {
				builder = builder.nonce(nonce);
			}
			let params = build_cord_params(builder);
			let mut tx_client = self.client.tx();
			let signed = tx_client
				.create_signed(payload, &self.signer, params)
				.await
				.wrap_err_with(|| format!("prepare {label} extrinsic"))?;
			let encoded = signed.encoded().to_vec();

			if std::env::var_os("ANCHOR_DUMP_EXTR").is_some() {
				let hex_payload = hex_encode(&encoded);
				tracing::info!(target: "anchor", "prepared {label} extrinsic 0x{hex_payload}");
			}

			let mut progress = match signed.submit_and_watch().await {
				Ok(stream) => stream,
				Err(err) => {
					if let Some(outcome) = self.inspect_invalid_extrinsic(&encoded).await {
						if outcome.should_retry() && attempts < 3 {
							attempts += 1;
							let nonce = self.next_nonce().await?;
							explicit_nonce = Some(nonce);
							tracing::warn!(target = "anchor", "{label} extrinsic submit error ({outcome}); retry {attempts} with nonce {nonce}");
							continue 'outer;
						}
						let suffix = outcome.describe();
						return Err(eyre!("submit {label} extrinsic: {err}{suffix}"));
					}
					return Err(eyre!("submit {label} extrinsic: {err}"));
				},
			};

			while let Some(status) = progress.next().await {
				let status = status.wrap_err_with(|| format!("{label} extrinsic status"))?;
				match status {
					TxStatus::InBestBlock(in_block) | TxStatus::InFinalizedBlock(in_block) => {
						let block_hash: Hash = in_block.block_hash().into();
						let events = in_block
							.wait_for_success()
							.await
							.wrap_err_with(|| format!("{label} extrinsic failed"))?;
						return Ok(SubmitResult { block_hash, events });
					},
					TxStatus::Invalid { message }
						if message.contains("InvalidTransaction::Stale") && attempts < 3 =>
					{
						attempts += 1;
						let nonce = self.next_nonce().await?;
						explicit_nonce = Some(nonce);
						tracing::warn!(target = "anchor", "{label} extrinsic had stale nonce; retry {attempts} with nonce {nonce}");
						continue 'outer;
					},
					TxStatus::Error { message }
					| TxStatus::Invalid { message }
					| TxStatus::Dropped { message } => {
						if let Some(outcome) = self.inspect_invalid_extrinsic(&encoded).await {
							if outcome.should_retry() && attempts < 3 {
								attempts += 1;
								let nonce = self.next_nonce().await?;
								explicit_nonce = Some(nonce);
								tracing::warn!(target = "anchor", "{label} extrinsic invalid ({outcome}); retry {attempts} with nonce {nonce}");
								continue 'outer;
							}
							let composed = format!("{message}{}", outcome.describe());
							return Err(eyre!("{label} extrinsic error: {composed}"));
						}
						return Err(eyre!("{label} extrinsic error: {message}"));
					},
					_ => continue,
				}
			}
			return Err(eyre!("{label} extrinsic stream ended before inclusion"));
		}
	}

	async fn inspect_invalid_extrinsic(&self, encoded: &[u8]) -> Option<DryRunOutcome> {
		let methods = LegacyRpcMethods::<CordConfig>::new((*self.rpc).clone());
		let output = methods.dry_run(encoded, None).await.ok()?;
		match output.into_dry_run_result() {
			Ok(DryRunResult::TransactionValidityError) => {
				if output.0.get(0) == Some(&1) {
					let mut cursor = &output.0[1..];
					match TransactionValidityError::decode(&mut cursor) {
						Ok(TransactionValidityError::Invalid(InvalidTransaction::Stale)) => {
							Some(DryRunOutcome::Stale)
						},
						Ok(err) => {
							Some(DryRunOutcome::Detail(format!("transaction validity: {err:?}")))
						},
						Err(decode_err) => Some(DryRunOutcome::Detail(format!(
							"transaction validity (decode error: {decode_err:?})"
						))),
					}
				} else {
					Some(DryRunOutcome::Detail(
						"transaction validity error (unable to decode detail)".into(),
					))
				}
			},
			Ok(DryRunResult::DispatchError(bytes)) => Some(DryRunOutcome::Detail(format!(
				"dispatch error bytes: 0x{}",
				hex_encode(bytes)
			))),
			Ok(DryRunResult::Success) => None,
			Err(decode_err) => Some(DryRunOutcome::Detail(match decode_err {
				DryRunDecodeError::WrongNumberOfBytes => {
					"dry-run decode error: wrong number of bytes".into()
				},
				DryRunDecodeError::InvalidBytes => {
					"dry-run decode error: invalid byte layout".into()
				},
			})),
		}
	}
}

enum DryRunOutcome {
	Stale,
	Detail(String),
}

impl DryRunOutcome {
	fn should_retry(&self) -> bool {
		matches!(self, DryRunOutcome::Stale)
	}

	fn describe(&self) -> String {
		match self {
			DryRunOutcome::Stale => " (transaction validity: InvalidTransaction::Stale)".into(),
			DryRunOutcome::Detail(msg) => format!(" ({msg})"),
		}
	}
}

impl core::fmt::Display for DryRunOutcome {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			DryRunOutcome::Stale => {
				write!(f, "transaction validity: InvalidTransaction::Stale")
			},
			DryRunOutcome::Detail(msg) => write!(f, "{msg}"),
		}
	}
}
