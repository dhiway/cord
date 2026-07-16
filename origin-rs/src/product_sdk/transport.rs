// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Transport binding for native product-SDK domain intents.
//!
//! Writes use Subxt dynamic payloads, so pallet and call shape are resolved from live metadata and
//! no pallet index is embedded here. Exact finalized runtime-API reads use
//! [`crate::product_sdk::OrbisFinalizedReadBinding`]; the older Origin view client is intentionally
//! not used because it selects only the latest block.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use scale_value::{Composite, Value};
use subxt::{config::DefaultExtrinsicParamsBuilder, tx::DynamicPayload};
use tokio::sync::Mutex;

use crate::{
	client::{
		signer::{OriginSigner, SubxtSignerAdapter},
		OriginClient,
	},
	config::{build_orbis_params, OrbisConfig},
	product_sdk::{
		domains::{
			attestation::{
				AttestationCommand, AttestationInput, AttestationRead, AttestationResponse,
				DelegatedIntent, DelegatedRevokeIntent, IndexPolicy, SchemaStatus, Signature,
				SignatureScheme, SignedDelegatedIssue, SignedDelegatedRevoke,
			},
			common::{AccountId, DomainResult, Hash32, SubmitAndFinalize, Validate},
			drive::{DriveCommand, DriveRead, DriveResponse},
			identity_personhood::{
				IdentityData, IdentityInfo, IdentityPersonhoodCommand, IdentityPersonhoodRead,
				IdentityPersonhoodResponse, Judgement,
			},
			names::{NamesCommand, NamesRead, NamesResponse},
			s3::{S3Command, S3Read, S3Response},
			storage::{
				CidConfig, HashingAlgorithm as StorageHashingAlgorithm, StorageCommand,
				StorageRead, StorageResponse, TransactionRef,
			},
			storage_provider::{
				ProviderStatus, StorageProviderCommand, StorageProviderRead,
				StorageProviderResponse,
			},
		},
		NativeError, NativeErrorCode, NativeLifecycle, NativeLifecycleState,
	},
	types::{account::ss58_to_account_id, error::OriginSdkError},
};

/// Pluggable exact runtime-API reader.
///
/// An implementation must invoke at `query.finalized_block_hash`, decode the matching versioned
/// runtime-API response, preserve that same hash in its result, and reject version drift.
#[async_trait]
pub trait FinalizedReadBinding: Send + Sync {
	async fn identity_personhood(
		&self,
		query: &IdentityPersonhoodRead,
	) -> DomainResult<IdentityPersonhoodResponse>;
	async fn attestation(&self, query: &AttestationRead) -> DomainResult<AttestationResponse>;
	async fn names(&self, query: &NamesRead) -> DomainResult<NamesResponse>;
	async fn storage_provider(
		&self,
		query: &StorageProviderRead,
	) -> DomainResult<StorageProviderResponse>;
	async fn drive(&self, query: &DriveRead) -> DomainResult<DriveResponse>;
	async fn s3(&self, query: &S3Read) -> DomainResult<S3Response>;
	async fn orbis_storage(&self, query: &StorageRead) -> DomainResult<StorageResponse>;
}

/// Fail-closed reader used when an exact pinned-hash runtime-API binding was not installed.
#[derive(Clone, Copy, Debug, Default)]
pub struct MissingFinalizedReadBinding;

#[async_trait]
impl FinalizedReadBinding for MissingFinalizedReadBinding {
	async fn identity_personhood(
		&self,
		_query: &IdentityPersonhoodRead,
	) -> DomainResult<IdentityPersonhoodResponse> {
		Err(read_binding_required())
	}

	async fn attestation(&self, _query: &AttestationRead) -> DomainResult<AttestationResponse> {
		Err(read_binding_required())
	}

	async fn names(&self, _query: &NamesRead) -> DomainResult<NamesResponse> {
		Err(read_binding_required())
	}

	async fn storage_provider(
		&self,
		_query: &StorageProviderRead,
	) -> DomainResult<StorageProviderResponse> {
		Err(read_binding_required())
	}

	async fn drive(&self, _query: &DriveRead) -> DomainResult<DriveResponse> {
		Err(read_binding_required())
	}

	async fn s3(&self, _query: &S3Read) -> DomainResult<S3Response> {
		Err(read_binding_required())
	}

	async fn orbis_storage(&self, _query: &StorageRead) -> DomainResult<StorageResponse> {
		Err(read_binding_required())
	}
}

/// Native domain adapter over the existing Origin client, signer, tx queue and finality watcher.
pub struct NativeDomainTransport<R = MissingFinalizedReadBinding> {
	client: OriginClient,
	signer: OriginSigner,
	reads: R,
}

impl NativeDomainTransport<MissingFinalizedReadBinding> {
	pub fn new(client: OriginClient, signer: OriginSigner) -> Self {
		Self { client, signer, reads: MissingFinalizedReadBinding }
	}
}

impl<R> NativeDomainTransport<R> {
	pub fn with_reads<R2>(self, reads: R2) -> NativeDomainTransport<R2> {
		NativeDomainTransport { client: self.client, signer: self.signer, reads }
	}

	pub fn client(&self) -> &OriginClient {
		&self.client
	}

	pub fn signer(&self) -> &OriginSigner {
		&self.signer
	}
}

impl<R: FinalizedReadBinding> NativeDomainTransport<R> {
	pub async fn read_identity_personhood(
		&self,
		query: &IdentityPersonhoodRead,
	) -> DomainResult<IdentityPersonhoodResponse> {
		query.validate()?;
		self.reads.identity_personhood(query).await
	}

	pub async fn read_attestation(
		&self,
		query: &AttestationRead,
	) -> DomainResult<AttestationResponse> {
		query.validate()?;
		self.reads.attestation(query).await
	}

	pub async fn read_names(&self, query: &NamesRead) -> DomainResult<NamesResponse> {
		query.validate()?;
		self.reads.names(query).await
	}

	pub async fn read_storage_provider(
		&self,
		query: &StorageProviderRead,
	) -> DomainResult<StorageProviderResponse> {
		query.validate()?;
		self.reads.storage_provider(query).await
	}

	pub async fn read_drive(&self, query: &DriveRead) -> DomainResult<DriveResponse> {
		query.validate()?;
		self.reads.drive(query).await
	}

	pub async fn read_s3(&self, query: &S3Read) -> DomainResult<S3Response> {
		query.validate()?;
		self.reads.s3(query).await
	}

	pub async fn read_storage(&self, query: &StorageRead) -> DomainResult<StorageResponse> {
		query.validate()?;
		self.reads.orbis_storage(query).await
	}

	pub async fn submit_attestation(
		&self,
		intent: &SubmitAndFinalize<AttestationCommand>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_attestation_command(&intent.command)?).await
	}

	pub async fn submit_identity_personhood(
		&self,
		intent: &SubmitAndFinalize<IdentityPersonhoodCommand>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_identity_personhood_command(&intent.command)?).await
	}

	pub async fn submit_names(
		&self,
		intent: &SubmitAndFinalize<NamesCommand>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_names_command(&intent.command)?).await
	}

	pub async fn submit_storage_provider(
		&self,
		intent: &SubmitAndFinalize<StorageProviderCommand>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_storage_provider_command(&intent.command)?).await
	}

	pub async fn submit_drive(
		&self,
		intent: &SubmitAndFinalize<DriveCommand>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_drive_command(&intent.command)?).await
	}

	pub async fn submit_s3(
		&self,
		intent: &SubmitAndFinalize<S3Command>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_s3_command(&intent.command)?).await
	}

	pub async fn submit_storage(
		&self,
		intent: &SubmitAndFinalize<StorageCommand>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_storage_command(&intent.command)?).await
	}

	async fn submit<C: Validate>(
		&self,
		intent: &SubmitAndFinalize<C>,
		payload: DynamicPayload,
	) -> DomainResult<NativeLifecycle> {
		intent.validate()?;
		ensure_signer(&intent.signer, &self.signer)?;
		let handle = self
			.client
			.tx()
			.using(self.signer.clone())
			.submit(payload)
			.await
			.map_err(map_sdk_error)?;
		let outcome = handle.wait_finalized().await.map_err(map_sdk_error)?;
		let block = outcome.block.ok_or_else(|| {
			NativeError::new(NativeErrorCode::RuntimeRejected, "missing finalized block")
		})?;
		let lifecycle = NativeLifecycle {
			version: 1,
			intent_id: intent.intent_id.clone(),
			state: NativeLifecycleState::Finalized,
			block_hash: Some(format!("{block:#x}")),
			extrinsic_hash: Some(format!("{:#x}", outcome.hash)),
			error: None,
		};
		lifecycle.validate()?;
		Ok(lifecycle)
	}
}

/// Orbis-native client and transaction lane.
///
/// Origin and Orbis expose different signed-extension tuples. Keeping a separate client prevents
/// an Origin parameter set from ever being used to sign an Orbis extrinsic. Submissions are
/// serialized per account through [`OrbisTxPipeline`], and the nonce is read again from the exact
/// finalized state before each signed transaction.
#[derive(Clone)]
pub struct OrbisNativeClient {
	online: subxt::OnlineClient<OrbisConfig>,
	tx: OrbisTxPipeline,
}

impl OrbisNativeClient {
	pub async fn connect(endpoint: impl AsRef<str>) -> DomainResult<Self> {
		let online = subxt::OnlineClient::<OrbisConfig>::from_url(endpoint.as_ref())
			.await
			.map_err(map_subxt_connection_error)?;
		Ok(Self::from_online(online))
	}

	pub fn from_online(online: subxt::OnlineClient<OrbisConfig>) -> Self {
		let tx = OrbisTxPipeline::new(online.clone());
		Self { online, tx }
	}

	pub fn online(&self) -> &subxt::OnlineClient<OrbisConfig> {
		&self.online
	}

	async fn submit_and_finalize(
		&self,
		signer: &OriginSigner,
		intent_id: &str,
		payload: DynamicPayload,
	) -> DomainResult<NativeLifecycle> {
		self.tx.submit_and_finalize(signer, intent_id, payload).await
	}
}

/// Dedicated Orbis nonce/sign/submit/finality pipeline.
///
/// The account lock is held until finalization. This makes the finalized-state nonce lookup
/// deterministic without reusing the Origin nonce cache or risking gaps after a rejected call;
/// unrelated accounts continue concurrently.
#[derive(Clone)]
pub struct OrbisTxPipeline {
	client: subxt::OnlineClient<OrbisConfig>,
	account_locks: Arc<Mutex<HashMap<[u8; 32], Arc<Mutex<()>>>>>,
}

impl OrbisTxPipeline {
	pub fn new(client: subxt::OnlineClient<OrbisConfig>) -> Self {
		Self { client, account_locks: Arc::new(Mutex::new(HashMap::new())) }
	}

	async fn account_lock(&self, account: &origin_primitives::AccountId) -> Arc<Mutex<()>> {
		let key: [u8; 32] = account.clone().into();
		let mut locks = self.account_locks.lock().await;
		locks.entry(key).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
	}

	pub async fn submit_and_finalize(
		&self,
		signer: &OriginSigner,
		intent_id: &str,
		payload: DynamicPayload,
	) -> DomainResult<NativeLifecycle> {
		let account = signer.account_id();
		let account_lock = self.account_lock(&account).await;
		let _account_guard = account_lock.lock().await;

		let nonce =
			self.client.tx().account_nonce(&account).await.map_err(map_subxt_nonce_error)?;
		let params =
			build_orbis_params(DefaultExtrinsicParamsBuilder::<OrbisConfig>::new().nonce(nonce));
		let adapter = SubxtSignerAdapter::new(Arc::new(signer.clone()));
		let signed = self
			.client
			.tx()
			.create_signed(&payload, &adapter, params)
			.await
			.map_err(map_subxt_tx_error)?;
		let mut progress = signed.submit_and_watch().await.map_err(map_subxt_tx_error)?;
		let extrinsic_hash = progress.extrinsic_hash();

		loop {
			match progress.next().await {
				Some(Ok(subxt::tx::TxStatus::InFinalizedBlock(in_block))) => {
					let block_hash = in_block.block_hash();
					in_block.wait_for_success().await.map_err(map_subxt_tx_error)?;
					let lifecycle = NativeLifecycle {
						version: 1,
						intent_id: intent_id.to_owned(),
						state: NativeLifecycleState::Finalized,
						block_hash: Some(format!("{block_hash:#x}")),
						extrinsic_hash: Some(format!("{extrinsic_hash:#x}")),
						error: None,
					};
					lifecycle.validate()?;
					return Ok(lifecycle);
				},
				Some(Ok(subxt::tx::TxStatus::Error { message }))
				| Some(Ok(subxt::tx::TxStatus::Invalid { message }))
				| Some(Ok(subxt::tx::TxStatus::Dropped { message })) => {
					return Err(NativeError::new(NativeErrorCode::RuntimeRejected, message));
				},
				Some(Ok(_)) => {},
				Some(Err(error)) => return Err(map_subxt_tx_error(error)),
				None => {
					return Err(NativeError::new(
						NativeErrorCode::RuntimeRejected,
						"Orbis transaction status stream ended before finalization",
					));
				},
			}
		}
	}
}

/// Explicit authority seam for calls that require a governed `Sudo` origin.
///
/// Implementations must wrap the supplied pallet call in the runtime's governed `Sudo` dispatch,
/// sign it with the configured authority, and wait for successful finalization. The default
/// binding refuses every privileged command; this SDK never submits a root call as an ordinary
/// signed extrinsic.
#[async_trait]
pub trait GovernedSudoBinding: Send + Sync {
	async fn sudo_and_finalize(
		&self,
		client: &OrbisNativeClient,
		intent_id: &str,
		call: DynamicPayload,
	) -> DomainResult<NativeLifecycle>;
}

/// Fail-closed governed-authority binding.
#[derive(Clone, Copy, Debug, Default)]
pub struct MissingGovernedSudoBinding;

#[async_trait]
impl GovernedSudoBinding for MissingGovernedSudoBinding {
	async fn sudo_and_finalize(
		&self,
		_client: &OrbisNativeClient,
		_intent_id: &str,
		_call: DynamicPayload,
	) -> DomainResult<NativeLifecycle> {
		Err(NativeError::new(
			NativeErrorCode::NotAuthorized,
			"privileged Orbis command requires an explicit governed Sudo binding",
		))
	}
}

/// Product-domain transport bound to Orbis signed extensions and finality.
pub struct OrbisDomainTransport<R = MissingFinalizedReadBinding, G = MissingGovernedSudoBinding> {
	client: OrbisNativeClient,
	signer: OriginSigner,
	reads: R,
	governance: G,
}

impl OrbisDomainTransport<MissingFinalizedReadBinding, MissingGovernedSudoBinding> {
	pub fn new(client: OrbisNativeClient, signer: OriginSigner) -> Self {
		Self {
			client,
			signer,
			reads: MissingFinalizedReadBinding,
			governance: MissingGovernedSudoBinding,
		}
	}
}

impl
	OrbisDomainTransport<crate::product_sdk::OrbisFinalizedReadBinding, MissingGovernedSudoBinding>
{
	/// Construct the normal Orbis product transport with exact finalized runtime-API reads.
	pub fn new_exact(client: OrbisNativeClient, signer: OriginSigner) -> Self {
		let reads = crate::product_sdk::OrbisFinalizedReadBinding::from_native(&client);
		Self { client, signer, reads, governance: MissingGovernedSudoBinding }
	}
}

impl<R, G> OrbisDomainTransport<R, G> {
	pub fn with_reads<R2>(self, reads: R2) -> OrbisDomainTransport<R2, G> {
		OrbisDomainTransport {
			client: self.client,
			signer: self.signer,
			reads,
			governance: self.governance,
		}
	}

	pub fn with_governance<G2>(self, governance: G2) -> OrbisDomainTransport<R, G2> {
		OrbisDomainTransport {
			client: self.client,
			signer: self.signer,
			reads: self.reads,
			governance,
		}
	}

	pub fn client(&self) -> &OrbisNativeClient {
		&self.client
	}

	pub fn signer(&self) -> &OriginSigner {
		&self.signer
	}
}

impl<R: FinalizedReadBinding, G: GovernedSudoBinding> OrbisDomainTransport<R, G> {
	pub async fn read_identity_personhood(
		&self,
		query: &IdentityPersonhoodRead,
	) -> DomainResult<IdentityPersonhoodResponse> {
		query.validate()?;
		self.reads.identity_personhood(query).await
	}

	pub async fn read_attestation(
		&self,
		query: &AttestationRead,
	) -> DomainResult<AttestationResponse> {
		query.validate()?;
		self.reads.attestation(query).await
	}

	pub async fn read_names(&self, query: &NamesRead) -> DomainResult<NamesResponse> {
		query.validate()?;
		self.reads.names(query).await
	}

	pub async fn read_storage_provider(
		&self,
		query: &StorageProviderRead,
	) -> DomainResult<StorageProviderResponse> {
		query.validate()?;
		self.reads.storage_provider(query).await
	}

	pub async fn read_drive(&self, query: &DriveRead) -> DomainResult<DriveResponse> {
		query.validate()?;
		self.reads.drive(query).await
	}

	pub async fn read_s3(&self, query: &S3Read) -> DomainResult<S3Response> {
		query.validate()?;
		self.reads.s3(query).await
	}

	pub async fn read_storage(&self, query: &StorageRead) -> DomainResult<StorageResponse> {
		query.validate()?;
		self.reads.orbis_storage(query).await
	}

	pub async fn submit_attestation(
		&self,
		intent: &SubmitAndFinalize<AttestationCommand>,
	) -> DomainResult<NativeLifecycle> {
		let privileged = is_privileged_attestation(&intent.command);
		self.submit(intent, prepare_attestation_command(&intent.command)?, privileged)
			.await
	}

	pub async fn submit_identity_personhood(
		&self,
		intent: &SubmitAndFinalize<IdentityPersonhoodCommand>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_identity_personhood_command(&intent.command)?, false)
			.await
	}

	pub async fn submit_names(
		&self,
		intent: &SubmitAndFinalize<NamesCommand>,
	) -> DomainResult<NativeLifecycle> {
		let privileged = is_privileged_names(&intent.command);
		self.submit(intent, prepare_names_command(&intent.command)?, privileged).await
	}

	pub async fn submit_storage_provider(
		&self,
		intent: &SubmitAndFinalize<StorageProviderCommand>,
	) -> DomainResult<NativeLifecycle> {
		let privileged = is_privileged_storage_provider(&intent.command);
		self.submit(intent, prepare_storage_provider_command(&intent.command)?, privileged)
			.await
	}

	pub async fn submit_drive(
		&self,
		intent: &SubmitAndFinalize<DriveCommand>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_drive_command(&intent.command)?, false).await
	}

	pub async fn submit_s3(
		&self,
		intent: &SubmitAndFinalize<S3Command>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_s3_command(&intent.command)?, false).await
	}

	pub async fn submit_storage(
		&self,
		intent: &SubmitAndFinalize<StorageCommand>,
	) -> DomainResult<NativeLifecycle> {
		self.submit(intent, prepare_storage_command(&intent.command)?, false).await
	}

	async fn submit<C: Validate>(
		&self,
		intent: &SubmitAndFinalize<C>,
		payload: DynamicPayload,
		privileged: bool,
	) -> DomainResult<NativeLifecycle> {
		intent.validate()?;
		ensure_signer(&intent.signer, &self.signer)?;
		if privileged {
			self.governance
				.sudo_and_finalize(&self.client, &intent.intent_id, payload)
				.await
		} else {
			self.client.submit_and_finalize(&self.signer, &intent.intent_id, payload).await
		}
	}
}

fn is_privileged_attestation(command: &AttestationCommand) -> bool {
	matches!(
		command,
		AttestationCommand::SetEmergencyPause { .. }
			| AttestationCommand::ForceSchemaStatus { .. }
			| AttestationCommand::ForceRevoke { .. }
	)
}

fn is_privileged_names(command: &NamesCommand) -> bool {
	matches!(
		command,
		NamesCommand::SetPaused { .. }
			| NamesCommand::ForceTransfer { .. }
			| NamesCommand::ForceRevoke { .. }
			| NamesCommand::SetRegistrar { .. }
	)
}

#[cfg(test)]
mod names_origin_tests {
	use super::*;
	use crate::product_sdk::domains::names::Label;

	fn name() -> crate::product_sdk::domains::common::NameId {
		crate::product_sdk::domains::common::NameId::new(format!("0x{}", "11".repeat(32)))
			.expect("valid name ID")
	}

	#[test]
	fn names_commands_follow_the_runtime_origin_contract() {
		let registrar = AccountId::new("registrar").expect("valid account DTO");
		assert!(is_privileged_names(&NamesCommand::SetRegistrar {
			registrar: registrar.clone(),
			enabled: true,
		}));

		// These calls accept either IdentityAdminOrigin or a signed scoped registrar in the
		// runtime. The routine product command must preserve the signed registrar route;
		// administrative emergency dispatch remains available through governance tooling.
		assert!(!is_privileged_names(&NamesCommand::ReserveName {
			parent: None,
			label: Label::new("system").expect("valid label"),
			beneficiary: Some(registrar),
			expires_at: None,
		}));
		assert!(!is_privileged_names(&NamesCommand::ClearReservation { name: name() }));
		assert!(!is_privileged_names(&NamesCommand::SetLabelProtection {
			label: Label::new("system").expect("valid label"),
			protected: true,
		}));
	}
}

fn is_privileged_storage_provider(command: &StorageProviderCommand) -> bool {
	matches!(
		command,
		StorageProviderCommand::RegisterProvider { .. }
			| StorageProviderCommand::UpdateProvider { .. }
			| StorageProviderCommand::SetProviderStatus { .. }
			| StorageProviderCommand::RemoveProvider { .. }
			| StorageProviderCommand::IssueChallenge { .. }
	)
}

/// Exact metadata source for each native identity/personhood write.
///
/// `CancelJudgement` intentionally binds to the pallet's `cancel_request` call; the product name
/// describes the user outcome while the source name remains metadata-exact.
pub const fn identity_personhood_command_source(
	command: &IdentityPersonhoodCommand,
) -> (&'static str, &'static str) {
	match command {
		IdentityPersonhoodCommand::SetIdentity { .. } => ("People", "set_identity"),
		IdentityPersonhoodCommand::ClearIdentity => ("People", "clear_identity"),
		IdentityPersonhoodCommand::RequestJudgement { .. } => ("People", "request_judgement"),
		IdentityPersonhoodCommand::CancelJudgement { .. } => ("People", "cancel_request"),
		IdentityPersonhoodCommand::ProvideJudgement { .. } => ("People", "provide_judgement"),
		IdentityPersonhoodCommand::AttestLitePerson { .. } => ("PeopleLite", "attest"),
	}
}

/// Prepare one of the six native People/PeopleLite writes using live metadata encoding.
pub fn prepare_identity_personhood_command(
	command: &IdentityPersonhoodCommand,
) -> DomainResult<DynamicPayload> {
	command.validate()?;
	let (pallet, call) = identity_personhood_command_source(command);
	let args = match command {
		IdentityPersonhoodCommand::SetIdentity { info } => vec![identity_info_value(info)?],
		IdentityPersonhoodCommand::ClearIdentity => vec![],
		IdentityPersonhoodCommand::RequestJudgement { registrar }
		| IdentityPersonhoodCommand::CancelJudgement { registrar } => vec![account_value(registrar)?],
		IdentityPersonhoodCommand::ProvideJudgement { target, judgement, identity_hash } => vec![
			lookup_account_value(target)?,
			judgement_value(*judgement),
			hash_value(identity_hash)?,
		],
		IdentityPersonhoodCommand::AttestLitePerson {
			candidate,
			candidate_signature,
			ring_vrf_key,
			proof_of_ownership,
		} => vec![
			account_value(candidate)?,
			signature_value(candidate_signature)?,
			hash_value(ring_vrf_key)?,
			Value::from_bytes(proof_of_ownership.raw_bytes()?),
			option_value(None),
		],
	};
	Ok(subxt::dynamic::tx(pallet, call, args))
}

fn identity_info_value(identity: &IdentityInfo) -> DomainResult<Value> {
	identity.validate()?;
	let additional = identity
		.additional
		.iter()
		.map(|field| {
			Ok(Value::unnamed_composite(vec![
				identity_data_value(&field.key)?,
				identity_data_value(&field.value)?,
			]))
		})
		.collect::<DomainResult<Vec<_>>>()?;
	Ok(Value::named_composite(vec![
		("additional", Value::from(additional)),
		("display", identity_data_value(&identity.display)?),
		("legal", identity_data_value(&identity.legal)?),
		("web", identity_data_value(&identity.web)?),
		("email", identity_data_value(&identity.email)?),
		("image", identity_data_value(&identity.image)?),
	]))
}

fn identity_data_value(data: &IdentityData) -> DomainResult<Value> {
	data.validate()?;
	Ok(match data {
		IdentityData::None => Value::variant("None", Composite::unnamed(vec![])),
		IdentityData::Raw { value } => Value::variant(
			format!("Raw{}", value.len()),
			Composite::unnamed(vec![Value::from_bytes(value.as_bytes())]),
		),
		IdentityData::BlakeTwo256 { hash } => {
			Value::variant("BlakeTwo256", Composite::unnamed(vec![hash_value(hash)?]))
		},
		IdentityData::Sha256 { hash } => {
			Value::variant("Sha256", Composite::unnamed(vec![hash_value(hash)?]))
		},
		IdentityData::Keccak256 { hash } => {
			Value::variant("Keccak256", Composite::unnamed(vec![hash_value(hash)?]))
		},
		IdentityData::ShaThree256 { hash } => {
			Value::variant("ShaThree256", Composite::unnamed(vec![hash_value(hash)?]))
		},
	})
}

fn judgement_value(judgement: Judgement) -> Value {
	let variant = match judgement {
		Judgement::Reasonable => "Reasonable",
		Judgement::KnownGood => "KnownGood",
		Judgement::OutOfDate => "OutOfDate",
		Judgement::LowQuality => "LowQuality",
		Judgement::Erroneous => "Erroneous",
	};
	Value::variant(variant, Composite::unnamed(vec![]))
}

fn lookup_account_value(account: &AccountId) -> DomainResult<Value> {
	Ok(Value::variant("Id", Composite::unnamed(vec![account_value(account)?])))
}

/// Prepare an attestation call for metadata-derived encoding without submitting it.
pub fn prepare_attestation_command(command: &AttestationCommand) -> DomainResult<DynamicPayload> {
	let (call, args) = match command {
		AttestationCommand::CreateSchema {
			definition,
			authorized_issuers,
			revocable,
			unique,
			index_policy,
		} => (
			"create_schema",
			vec![
				Value::from_bytes(definition),
				Value::from(
					authorized_issuers
						.iter()
						.map(account_value)
						.collect::<DomainResult<Vec<_>>>()?,
				),
				Value::bool(*revocable),
				Value::bool(*unique),
				index_policy_value(*index_policy),
			],
		),
		AttestationCommand::SetSchemaStatus { schema, status } => {
			("set_schema_status", vec![hash_value(schema.as_hash())?, schema_status(*status)])
		},
		AttestationCommand::Issue { input } => ("issue", vec![attestation_input(input)?]),
		AttestationCommand::IssueDelegated { intent, signature } => {
			("issue_delegated", vec![delegated_intent(intent)?, signature_value(signature)?])
		},
		AttestationCommand::IssueBatch { inputs } => (
			"issue_batch",
			vec![Value::from(
				inputs.iter().map(attestation_input).collect::<DomainResult<Vec<_>>>()?,
			)],
		),
		AttestationCommand::Revoke { attestation } => {
			("revoke", vec![hash_value(attestation.as_hash())?])
		},
		AttestationCommand::RevokeDelegated { intent, signature } => (
			"revoke_delegated",
			vec![delegated_revoke_intent(intent)?, signature_value(signature)?],
		),
		AttestationCommand::IssueDelegatedBatch { items } => (
			"issue_delegated_batch",
			vec![Value::from(
				items.iter().map(signed_delegated_issue).collect::<DomainResult<Vec<_>>>()?,
			)],
		),
		AttestationCommand::RevokeBatch { attestations } => (
			"revoke_batch",
			vec![Value::from(
				attestations
					.iter()
					.map(|id| hash_value(id.as_hash()))
					.collect::<DomainResult<Vec<_>>>()?,
			)],
		),
		AttestationCommand::RevokeDelegatedBatch { items } => (
			"revoke_delegated_batch",
			vec![Value::from(
				items.iter().map(signed_delegated_revoke).collect::<DomainResult<Vec<_>>>()?,
			)],
		),
		AttestationCommand::RevokeExternalStatus { status_commitment } => {
			("revoke_external_status", vec![hash_value(status_commitment.as_hash())?])
		},
		AttestationCommand::RevokeExternalStatusBatch { status_commitments } => (
			"revoke_external_status_batch",
			vec![Value::from(
				status_commitments
					.iter()
					.map(|commitment| hash_value(commitment.as_hash()))
					.collect::<DomainResult<Vec<_>>>()?,
			)],
		),
		AttestationCommand::SetEmergencyPause { paused } => {
			("set_emergency_pause", vec![Value::bool(*paused)])
		},
		AttestationCommand::ForceSchemaStatus { schema, status } => {
			("force_schema_status", vec![hash_value(schema.as_hash())?, schema_status(*status)])
		},
		AttestationCommand::ForceRevoke { attestation } => {
			("force_revoke", vec![hash_value(attestation.as_hash())?])
		},
	};
	Ok(subxt::dynamic::tx("Attestation", call, args))
}

/// Prepare an Orbis Names call for metadata-derived encoding without submitting it.
pub fn prepare_names_command(command: &NamesCommand) -> DomainResult<DynamicPayload> {
	let (call, args) = match command {
		NamesCommand::Commit { commitment } => ("commit", vec![hash_value(commitment.as_hash())?]),
		NamesCommand::CancelCommitment { commitment } => {
			("cancel_commitment", vec![hash_value(commitment.as_hash())?])
		},
		NamesCommand::PruneExpiredCommitment { owner, commitment } => (
			"prune_expired_commitment",
			vec![account_value(owner)?, hash_value(commitment.as_hash())?],
		),
		NamesCommand::Register { parent, label, salt } => (
			"register",
			vec![
				option_hash(parent.as_ref().map(|id| id.as_hash()))?,
				Value::from_bytes(label.as_str().as_bytes()),
				Value::from_bytes(salt.as_bytes()),
			],
		),
		NamesCommand::Renew { name, additional_blocks } => {
			("renew", vec![hash_value(name.as_hash())?, Value::u128(*additional_blocks as u128)])
		},
		NamesCommand::Transfer { name, new_owner } => {
			("transfer", vec![hash_value(name.as_hash())?, account_value(new_owner)?])
		},
		NamesCommand::AddController { name, controller } => {
			("add_controller", vec![hash_value(name.as_hash())?, account_value(controller)?])
		},
		NamesCommand::RemoveController { name, controller } => {
			("remove_controller", vec![hash_value(name.as_hash())?, account_value(controller)?])
		},
		NamesCommand::SetAddress { name, address } => (
			"set_address",
			vec![
				hash_value(name.as_hash())?,
				option_value(address.as_ref().map(|value| Value::from_bytes(value.as_bytes()))),
			],
		),
		NamesCommand::SetSubject { name, subject } => (
			"set_subject",
			vec![
				hash_value(name.as_hash())?,
				option_value(subject.as_ref().map(|id| Value::from_bytes(id.as_str().as_bytes()))),
			],
		),
		NamesCommand::SetAttestation { name, attestation } => (
			"set_attestation",
			vec![
				hash_value(name.as_hash())?,
				option_hash(attestation.as_ref().map(|id| id.as_hash()))?,
			],
		),
		NamesCommand::SetContent { name, content } => (
			"set_content",
			vec![
				hash_value(name.as_hash())?,
				option_hash(content.as_ref().map(|id| id.as_hash()))?,
			],
		),
		NamesCommand::SetText { name, key, value } => (
			"set_text",
			vec![
				hash_value(name.as_hash())?,
				Value::from_bytes(key.as_bytes()),
				option_value(value.as_ref().map(|value| Value::from_bytes(value.as_bytes()))),
			],
		),
		NamesCommand::SetPrimaryName { name } => {
			("set_primary_name", vec![option_hash(name.as_ref().map(|id| id.as_hash()))?])
		},
		NamesCommand::Release { name } => ("release", vec![hash_value(name.as_hash())?]),
		NamesCommand::RemoveExpiredName { name } => {
			("remove_expired_name", vec![hash_value(name.as_hash())?])
		},
		NamesCommand::ReserveName { parent, label, beneficiary, expires_at } => (
			"reserve_name",
			vec![
				option_hash(parent.as_ref().map(|id| id.as_hash()))?,
				Value::from_bytes(label.as_str().as_bytes()),
				option_result(beneficiary.as_ref().map(account_value))?,
				option_value(expires_at.map(|value| Value::u128(value as u128))),
			],
		),
		NamesCommand::ClearReservation { name } => {
			("clear_reservation", vec![hash_value(name.as_hash())?])
		},
		NamesCommand::SetLabelProtection { label, protected } => (
			"set_label_protection",
			vec![Value::from_bytes(label.as_str().as_bytes()), Value::bool(*protected)],
		),
		NamesCommand::SetPaused { paused } => ("set_paused", vec![Value::bool(*paused)]),
		NamesCommand::ForceTransfer { name, new_owner } => {
			("force_transfer", vec![hash_value(name.as_hash())?, account_value(new_owner)?])
		},
		NamesCommand::ForceRevoke { name } => ("force_revoke", vec![hash_value(name.as_hash())?]),
		NamesCommand::SetRegistrar { registrar, enabled } => {
			("set_registrar", vec![account_value(registrar)?, Value::bool(*enabled)])
		},
	};
	Ok(subxt::dynamic::tx("Names", call, args))
}

/// Prepare a provider or TransactionStorage attachment call without submitting it.
pub fn prepare_storage_provider_command(
	command: &StorageProviderCommand,
) -> DomainResult<DynamicPayload> {
	let (pallet, call, args) = match command {
		StorageProviderCommand::RegisterProvider {
			provider,
			endpoint,
			service_key,
			capacity_bytes,
		} => (
			"StorageProvider",
			"register_provider",
			vec![
				account_value(provider)?,
				Value::from_bytes(endpoint.as_bytes()),
				Value::from_bytes(service_key.as_bytes()),
				Value::u128(*capacity_bytes as u128),
			],
		),
		StorageProviderCommand::UpdateProvider {
			provider,
			endpoint,
			service_key,
			capacity_bytes,
		} => (
			"StorageProvider",
			"update_provider",
			vec![
				account_value(provider)?,
				Value::from_bytes(endpoint.as_bytes()),
				Value::from_bytes(service_key.as_bytes()),
				Value::u128(*capacity_bytes as u128),
			],
		),
		StorageProviderCommand::SetProviderStatus { provider, status } => (
			"StorageProvider",
			"set_provider_status",
			vec![account_value(provider)?, provider_status(*status)],
		),
		StorageProviderCommand::RemoveProvider { provider } => {
			("StorageProvider", "remove_provider", vec![account_value(provider)?])
		},
		StorageProviderCommand::Heartbeat => ("StorageProvider", "heartbeat", vec![]),
		StorageProviderCommand::ProposeAgreement {
			provider,
			container,
			content_commitment,
			reservation_ref,
			bytes,
			expires_at,
		} => (
			"StorageProvider",
			"propose_agreement",
			vec![
				account_value(provider)?,
				hash_value(container.as_hash())?,
				hash_value(content_commitment.as_hash())?,
				option_value(
					reservation_ref
						.as_ref()
						.map(|id| id.as_u64().map(|value| Value::u128(value as u128)))
						.transpose()?,
				),
				Value::u128(*bytes as u128),
				Value::u128(*expires_at as u128),
			],
		),
		StorageProviderCommand::AcceptAgreement { agreement } => {
			("StorageProvider", "accept_agreement", vec![hash_value(agreement.as_hash())?])
		},
		StorageProviderCommand::CancelAgreement { agreement } => {
			("StorageProvider", "cancel_agreement", vec![hash_value(agreement.as_hash())?])
		},
		StorageProviderCommand::IssueChallenge { agreement, expected_commitment, due_at } => (
			"StorageProvider",
			"issue_challenge",
			vec![
				hash_value(agreement.as_hash())?,
				hash_value(expected_commitment.as_hash())?,
				Value::u128(*due_at as u128),
			],
		),
		StorageProviderCommand::SubmitCheckpoint { challenge, proof_commitment } => (
			"StorageProvider",
			"submit_checkpoint",
			vec![hash_value(challenge.as_hash())?, hash_value(proof_commitment.as_hash())?],
		),
		StorageProviderCommand::TimeoutChallenge { challenge } => {
			("StorageProvider", "timeout_challenge", vec![hash_value(challenge.as_hash())?])
		},
		StorageProviderCommand::RequestRenewal { agreement, expires_at } => (
			"StorageProvider",
			"request_renewal",
			vec![hash_value(agreement.as_hash())?, Value::u128(*expires_at as u128)],
		),
		StorageProviderCommand::AcceptRenewal { agreement } => {
			("StorageProvider", "accept_renewal", vec![hash_value(agreement.as_hash())?])
		},
		StorageProviderCommand::ExpireAgreement { agreement } => {
			("StorageProvider", "expire_agreement", vec![hash_value(agreement.as_hash())?])
		},
		StorageProviderCommand::PruneAgreement { agreement } => {
			("StorageProvider", "prune_agreement", vec![hash_value(agreement.as_hash())?])
		},
		StorageProviderCommand::AcknowledgeDeletion {
			agreement,
			content_commitment,
			tombstone_root,
			root_sequence,
			leaf_index,
			leaf_count,
			inclusion_proof,
		} => (
			"StorageProvider",
			"acknowledge_deletion",
			vec![
				hash_value(agreement.as_hash())?,
				hash_value(content_commitment.as_hash())?,
				hash_value(tombstone_root.as_hash())?,
				Value::u128(*root_sequence as u128),
				Value::u128(*leaf_index as u128),
				Value::u128(*leaf_count as u128),
				Value::unnamed_composite(
					inclusion_proof
						.iter()
						.map(|hash| hash_value(hash.as_hash()))
						.collect::<DomainResult<Vec<_>>>()?,
				),
			],
		),
		StorageProviderCommand::AcknowledgeManifestDeletion {
			manifest,
			evidence_hash,
			service_key,
			signature,
		} => (
			"StorageProvider",
			"acknowledge_manifest_deletion",
			vec![
				hash_value(manifest.as_hash())?,
				hash_value(evidence_hash.as_hash())?,
				Value::from_bytes(service_key.as_bytes()),
				Value::from_bytes(signature),
			],
		),
		StorageProviderCommand::CommitProviderRoot { sequence, appended_leaves } => (
			"StorageProvider",
			"commit_provider_root",
			vec![
				Value::u128(*sequence as u128),
				Value::unnamed_composite(
					appended_leaves
						.iter()
						.map(|hash| hash_value(hash.as_hash()))
						.collect::<DomainResult<Vec<_>>>()?,
				),
			],
		),
		StorageProviderCommand::AttachProvider { reservation_id, provider_ref } => (
			"TransactionStorage",
			"attach_provider",
			vec![
				Value::u128(reservation_id.as_u64()? as u128),
				hash_value(provider_ref.as_hash())?,
			],
		),
	};
	Ok(subxt::dynamic::tx(pallet, call, args))
}

/// Prepare a Orbis Storage TransactionStorage call using live metadata.
pub fn prepare_storage_command(command: &StorageCommand) -> DomainResult<DynamicPayload> {
	command.validate()?;
	let (call, args) = match command {
		StorageCommand::Store { content_base64 } => {
			("store", vec![Value::from_bytes(content_base64.decode()?)])
		},
		StorageCommand::StoreWithCidConfig { cid_config, content_base64 } => (
			"store_with_cid_config",
			vec![cid_config_value(cid_config)?, Value::from_bytes(content_base64.decode()?)],
		),
		StorageCommand::StoreReserved { reservation_id, cid_config, content_base64 } => (
			"store_reserved",
			vec![
				Value::u128(reservation_id.as_u64()? as u128),
				cid_config_value(cid_config)?,
				Value::from_bytes(content_base64.decode()?),
			],
		),
		StorageCommand::RenewReserved { reservation_id, content_hash } => (
			"renew_reserved",
			vec![
				Value::u128(reservation_id.as_u64()? as u128),
				hash_value(content_hash.as_hash())?,
			],
		),
		StorageCommand::AttachProvider { reservation_id, provider_ref } => (
			"attach_provider",
			vec![
				Value::u128(reservation_id.as_u64()? as u128),
				hash_value(provider_ref.as_hash())?,
			],
		),
		StorageCommand::Renew { entry } => ("renew", vec![transaction_ref_value(entry)?]),
		StorageCommand::ForceRenew { entry } => {
			("force_renew", vec![transaction_ref_value(entry)?])
		},
		StorageCommand::EnableAutoRenew { content_hash } => {
			("enable_auto_renew", vec![hash_value(content_hash.as_hash())?])
		},
		StorageCommand::DisableAutoRenew { content_hash } => {
			("disable_auto_renew", vec![hash_value(content_hash.as_hash())?])
		},
	};
	Ok(subxt::dynamic::tx("TransactionStorage", call, args))
}

/// Prepare a Drive call for metadata-derived encoding without submitting it.
pub fn prepare_drive_command(command: &DriveCommand) -> DomainResult<DynamicPayload> {
	let (call, args) = match command {
		DriveCommand::Create { name, root_storage_ref } => (
			"create_drive",
			vec![
				Value::from_bytes(name.as_bytes()),
				option_hash(root_storage_ref.as_ref().map(|id| id.as_hash()))?,
			],
		),
		DriveCommand::UpdateRoot { drive, expected_version, root_storage_ref } => (
			"update_root",
			vec![
				hash_value(drive.as_hash())?,
				Value::u128(*expected_version as u128),
				option_hash(root_storage_ref.as_ref().map(|id| id.as_hash()))?,
			],
		),
		DriveCommand::SetController { drive, controller, enabled } => (
			"set_controller",
			vec![hash_value(drive.as_hash())?, account_value(controller)?, Value::bool(*enabled)],
		),
		DriveCommand::Transfer { drive, new_owner } => {
			("transfer_drive", vec![hash_value(drive.as_hash())?, account_value(new_owner)?])
		},
		DriveCommand::Archive { drive } => ("archive_drive", vec![hash_value(drive.as_hash())?]),
	};
	Ok(subxt::dynamic::tx("Drive", call, args))
}

/// Prepare an S3 call for metadata-derived encoding without submitting it.
pub fn prepare_s3_command(command: &S3Command) -> DomainResult<DynamicPayload> {
	let (call, args) = match command {
		S3Command::CreateBucket { name } => {
			("create_bucket", vec![Value::from_bytes(name.as_str().as_bytes())])
		},
		S3Command::SetController { bucket, expected_bucket_version, controller, enabled } => (
			"set_controller",
			vec![
				hash_value(bucket.as_hash())?,
				Value::u128(*expected_bucket_version as u128),
				account_value(controller)?,
				Value::bool(*enabled),
			],
		),
		S3Command::TransferBucket { bucket, expected_bucket_version, new_owner } => (
			"transfer_bucket",
			vec![
				hash_value(bucket.as_hash())?,
				Value::u128(*expected_bucket_version as u128),
				account_value(new_owner)?,
			],
		),
		S3Command::SetArchived { bucket, expected_bucket_version, archived } => (
			"set_archived",
			vec![
				hash_value(bucket.as_hash())?,
				Value::u128(*expected_bucket_version as u128),
				Value::bool(*archived),
			],
		),
		S3Command::SetVersioning { bucket, expected_bucket_version, enabled } => (
			"set_versioning",
			vec![
				hash_value(bucket.as_hash())?,
				Value::u128(*expected_bucket_version as u128),
				Value::bool(*enabled),
			],
		),
		S3Command::PutObject { bucket, key, content, expected_object_version } => (
			"put_object",
			vec![
				hash_value(bucket.as_hash())?,
				Value::from_bytes(key.as_bytes()),
				hash_value(content.as_hash())?,
				option_value(expected_object_version.map(|value| Value::u128(value as u128))),
			],
		),
		S3Command::DeleteObject { bucket, key, expected_object_version } => (
			"delete_object",
			vec![
				hash_value(bucket.as_hash())?,
				Value::from_bytes(key.as_bytes()),
				Value::u128(*expected_object_version as u128),
			],
		),
		S3Command::DeleteBucket { bucket, expected_bucket_version } => (
			"delete_bucket",
			vec![hash_value(bucket.as_hash())?, Value::u128(*expected_bucket_version as u128)],
		),
	};
	Ok(subxt::dynamic::tx("S3", call, args))
}

fn attestation_input(input: &AttestationInput) -> DomainResult<Value> {
	Ok(Value::unnamed_composite(vec![
		hash_value(input.schema.as_hash())?,
		hash_value(input.subject_commitment.as_hash())?,
		hash_value(input.payload_commitment.as_hash())?,
		hash_value(input.status_commitment.as_hash())?,
		option_hash(input.parent.as_ref().map(|id| id.as_hash()))?,
		option_value(input.expiry.map(|value| Value::u128(value as u128))),
		option_hash(input.uniqueness_commitment.as_ref().map(|id| id.as_hash()))?,
		Value::bool(input.revocable),
	]))
}

fn delegated_intent(intent: &DelegatedIntent) -> DomainResult<Value> {
	Ok(Value::unnamed_composite(vec![
		hash_value(&intent.genesis_hash)?,
		Value::u128(intent.spec_version as u128),
		Value::variant("Issue", Composite::unnamed(vec![])),
		account_value(&intent.issuer)?,
		account_value(&intent.delegate)?,
		hash_value(intent.schema.as_hash())?,
		hash_value(intent.subject_commitment.as_hash())?,
		hash_value(intent.payload_commitment.as_hash())?,
		hash_value(intent.status_commitment.as_hash())?,
		option_hash(intent.parent.as_ref().map(|id| id.as_hash()))?,
		option_value(intent.expiry.map(|value| Value::u128(value as u128))),
		option_hash(intent.uniqueness_commitment.as_ref().map(|id| id.as_hash()))?,
		Value::bool(intent.revocable),
		Value::u128(intent.nonce as u128),
		Value::u128(intent.deadline as u128),
	]))
}

fn delegated_revoke_intent(intent: &DelegatedRevokeIntent) -> DomainResult<Value> {
	Ok(Value::unnamed_composite(vec![
		hash_value(&intent.genesis_hash)?,
		Value::u128(intent.spec_version as u128),
		Value::variant("Revoke", Composite::unnamed(vec![])),
		account_value(&intent.revoker)?,
		account_value(&intent.delegate)?,
		hash_value(intent.attestation.as_hash())?,
		Value::u128(intent.nonce as u128),
		Value::u128(intent.deadline as u128),
	]))
}

fn signed_delegated_issue(item: &SignedDelegatedIssue) -> DomainResult<Value> {
	Ok(Value::unnamed_composite(vec![
		delegated_intent(&item.intent)?,
		signature_value(&item.signature)?,
	]))
}

fn signed_delegated_revoke(item: &SignedDelegatedRevoke) -> DomainResult<Value> {
	Ok(Value::unnamed_composite(vec![
		delegated_revoke_intent(&item.intent)?,
		signature_value(&item.signature)?,
	]))
}

fn signature_value(signature: &Signature) -> DomainResult<Value> {
	let variant = match signature.scheme {
		SignatureScheme::Sr25519 => "Sr25519",
		SignatureScheme::Ed25519 => "Ed25519",
		SignatureScheme::Ecdsa => "Ecdsa",
	};
	Ok(Value::variant(variant, Composite::unnamed(vec![Value::from_bytes(signature.raw_bytes()?)])))
}

fn schema_status(status: SchemaStatus) -> Value {
	let variant = match status {
		SchemaStatus::Active => "Active",
		SchemaStatus::Paused => "Paused",
		SchemaStatus::Retired => "Retired",
	};
	Value::variant(variant, Composite::unnamed(vec![]))
}

fn index_policy_value(policy: IndexPolicy) -> Value {
	let variant = match policy {
		IndexPolicy::None => "None",
		IndexPolicy::Issuer => "Issuer",
		IndexPolicy::SubjectAndSchema => "SubjectAndSchema",
		IndexPolicy::IssuerAndSubjectSchema => "IssuerAndSubjectSchema",
	};
	Value::variant(variant, Composite::unnamed(vec![]))
}

fn provider_status(status: ProviderStatus) -> Value {
	let variant = match status {
		ProviderStatus::Active => "Active",
		ProviderStatus::Suspended => "Suspended",
	};
	Value::variant(variant, Composite::unnamed(vec![]))
}

fn cid_config_value(config: &CidConfig) -> DomainResult<Value> {
	let hashing = match config.hashing {
		StorageHashingAlgorithm::Blake2b256 => "Blake2b256",
		StorageHashingAlgorithm::Sha2_256 => "Sha2_256",
		StorageHashingAlgorithm::Keccak256 => "Keccak256",
	};
	Ok(Value::named_composite(vec![
		("codec", Value::u128(config.codec.as_u64()? as u128)),
		("hashing", Value::variant(hashing, Composite::unnamed(vec![]))),
	]))
}

fn transaction_ref_value(reference: &TransactionRef) -> DomainResult<Value> {
	reference.validate()?;
	Ok(match reference {
		TransactionRef::Position { block, index } => Value::variant(
			"Position",
			Composite::named(vec![
				("block", Value::u128(*block as u128)),
				("index", Value::u128(*index as u128)),
			]),
		),
		TransactionRef::ContentHash { content_hash } => Value::variant(
			"ContentHash",
			Composite::unnamed(vec![hash_value(content_hash.as_hash())?]),
		),
	})
}

fn option_hash(value: Option<&Hash32>) -> DomainResult<Value> {
	match value {
		Some(value) => Ok(option_value(Some(hash_value(value)?))),
		None => Ok(option_value(None)),
	}
}

fn option_result(value: Option<DomainResult<Value>>) -> DomainResult<Value> {
	match value {
		Some(value) => Ok(option_value(Some(value?))),
		None => Ok(option_value(None)),
	}
}

fn option_value(value: Option<Value>) -> Value {
	match value {
		Some(value) => Value::variant("Some", Composite::unnamed(vec![value])),
		None => Value::variant("None", Composite::unnamed(vec![])),
	}
}

fn hash_value(value: &Hash32) -> DomainResult<Value> {
	value.validate()?;
	let raw = hex::decode(&value.as_str()[2..])
		.map_err(|_| NativeError::new(NativeErrorCode::InvalidInput, "invalid hash hex"))?;
	Ok(Value::from_bytes(raw))
}

fn account_value(value: &AccountId) -> DomainResult<Value> {
	value.validate()?;
	let account = ss58_to_account_id(value.as_str())
		.map_err(|error| NativeError::new(NativeErrorCode::InvalidInput, error.to_string()))?;
	let bytes: &[u8; 32] = account.as_ref();
	Ok(Value::from_bytes(bytes))
}

fn ensure_signer(expected: &AccountId, signer: &OriginSigner) -> DomainResult<()> {
	let expected = ss58_to_account_id(expected.as_str())
		.map_err(|error| NativeError::new(NativeErrorCode::InvalidInput, error.to_string()))?;
	let actual = signer.account_id();
	let actual_bytes: &[u8; 32] = actual.as_ref();
	let expected_bytes: &[u8; 32] = expected.as_ref();
	if actual_bytes != expected_bytes {
		return Err(NativeError::new(
			NativeErrorCode::NotAuthorized,
			"command signer does not match the submit-and-finalize intent",
		));
	}
	Ok(())
}

fn map_sdk_error(error: OriginSdkError) -> NativeError {
	match error {
		OriginSdkError::Timeout => {
			NativeError::new(NativeErrorCode::Timeout, "native operation timed out").retryable()
		},
		OriginSdkError::InvalidInput(message) => {
			NativeError::new(NativeErrorCode::InvalidInput, message)
		},
		OriginSdkError::Metadata(message) => {
			NativeError::new(NativeErrorCode::MetadataMismatch, message)
		},
		OriginSdkError::Encode(message) | OriginSdkError::Decode(message) => {
			NativeError::new(NativeErrorCode::UnsupportedRuntime, message)
		},
		OriginSdkError::Connection(message) | OriginSdkError::View(message) => {
			NativeError::new(NativeErrorCode::ContentUnavailable, message).retryable()
		},
		other => NativeError::new(NativeErrorCode::RuntimeRejected, other.to_string()),
	}
}

fn map_subxt_connection_error(error: subxt::Error) -> NativeError {
	NativeError::new(NativeErrorCode::ContentUnavailable, error.to_string()).retryable()
}

fn map_subxt_nonce_error(error: subxt::Error) -> NativeError {
	NativeError::new(
		NativeErrorCode::ContentUnavailable,
		format!("failed to read finalized Orbis account nonce: {error}"),
	)
	.retryable()
}

fn map_subxt_tx_error(error: subxt::Error) -> NativeError {
	NativeError::new(NativeErrorCode::RuntimeRejected, error.to_string())
}

fn read_binding_required() -> NativeError {
	NativeError::new(
		NativeErrorCode::UnsupportedSurface,
		"exact finalized runtime-API reads require an explicit Orbis finalized-read binding",
	)
}
