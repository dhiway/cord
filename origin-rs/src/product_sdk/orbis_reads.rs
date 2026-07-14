//! Exact finalized-hash, metadata-decoded Orbis runtime API reads.
//!
//! This binding never calls `at_latest` for data and never decodes raw SCALE. The requested hash
//! selects the runtime API context; Subxt validates method/argument shape from metadata and decodes
//! each response into a concrete `DecodeAsType` wire type.

use async_trait::async_trait;
use orbis_identity_personhood_runtime_api as identity_api;
use orbis_storage_runtime_api as storage_api;
use pallet_bulletin_transaction_storage_runtime_api as bulletin_api;
use pallet_orbis_attestation_runtime_api as att_api;
use pallet_orbis_dotns_runtime_api as dotns_api;
use scale_value::{Composite, Value};
use subxt::{metadata::DecodeWithMetadata, runtime_api::StaticPayload};

use super::{
	domains::{
		attestation::{
			AttestationQuery, AttestationRead, AttestationResponse,
			AttestationView as DomainAttestationView,
			ExternalStatusView as DomainExternalStatusView, IndexPolicy as DomainIndexPolicy,
			LiveStatus, SchemaStatus as DomainSchemaStatus, SchemaView as DomainSchemaView,
		},
		common::{
			AccountId, AgreementId, AttestationId, BucketId, ChallengeId, ContainerId,
			ContentCommitment, ContentHash, DomainResult, DriveId, FinalizedPage, FinalizedValue,
			Hash32, NameId, ObjectId, ProofCommitment, ProviderReference, ReservationId, SchemaId,
			SubjectId, UniquenessCommitment, Validate,
		},
		dotns::{
			Address, DotnsQuery, DotnsRead, DotnsResponse, Label, NameStatus as DomainNameStatus,
			NameView as DomainNameView, TextValue,
		},
		drive::{DriveName, DriveQuery, DriveRead, DriveResponse, DriveStatus, DriveView},
		identity_personhood::{
			AttestationAllowanceView, IdentityPersonhoodQuery, IdentityPersonhoodRead,
			IdentityPersonhoodResponse, IdentityStatusView, PersonhoodStatusView,
		},
		s3::{
			BucketName, BucketStatus, BucketView, ObjectKey, ObjectVersionView, ObjectView,
			S3Query, S3Read, S3Response,
		},
		storage::{
			AccountAuthorization as DomainAccountAuthorization, ActiveResourceReservation,
			BulletinRef as DomainBulletinRef, DecimalU64, ResourceClosure,
			ResourceReservationLink as DomainResourceReservationLink, ResourceReservationTombstone,
			ResourceReservationView, StorageActor, StorageQuery, StorageRead, StorageResponse,
			TransactionRef,
		},
		storage_provider::{
			AgreementStatus, AgreementView, ChallengeStatus, ChallengeView, CheckpointView,
			DeletionAcknowledgementView, Endpoint, ProviderRootView, ProviderStatus, ProviderView,
			ServiceKey, StorageProviderQuery, StorageProviderRead, StorageProviderResponse,
		},
	},
	transport::{FinalizedReadBinding, OrbisNativeClient},
	NativeError, NativeErrorCode,
};
use crate::{
	config::OrbisConfig,
	types::account::{account_id_from_subxt, account_id_to_ss58},
};

type RuntimeAccountId = subxt::utils::AccountId32;
type RuntimeHash = subxt::utils::H256;
type RuntimeBlockNumber = u32;
type RuntimeSubjectId = origin_primitives::identifier::Ss58Identifier;

/// Concrete metadata-aware read binding for Orbis.
#[derive(Clone)]
pub struct OrbisFinalizedReadBinding {
	client: subxt::OnlineClient<OrbisConfig>,
}

impl OrbisFinalizedReadBinding {
	pub fn new(client: subxt::OnlineClient<OrbisConfig>) -> Self {
		Self { client }
	}

	pub fn from_native(client: &OrbisNativeClient) -> Self {
		Self::new(client.online().clone())
	}

	pub fn client(&self) -> &subxt::OnlineClient<OrbisConfig> {
		&self.client
	}

	async fn call_at<T: DecodeWithMetadata>(
		&self,
		finalized_hash: &Hash32,
		trait_name: &'static str,
		method_name: &'static str,
		args: Vec<Value>,
	) -> DomainResult<T> {
		let block_hash = runtime_hash(finalized_hash)?;
		self.ensure_not_ahead_of_finality(block_hash).await?;
		let payload = StaticPayload::<Composite<()>, T>::new(
			trait_name,
			method_name,
			Composite::unnamed(args),
		);
		self.client
			.runtime_api()
			.at(block_hash)
			.call(payload)
			.await
			.map_err(runtime_api_error)
	}

	async fn ensure_not_ahead_of_finality(&self, requested_hash: RuntimeHash) -> DomainResult<()> {
		let requested = self.client.blocks().at(requested_hash).await.map_err(runtime_api_error)?;
		let finalized = self.client.blocks().at_latest().await.map_err(runtime_api_error)?;
		let requested_number = requested.number();
		let finalized_number = finalized.number();
		if requested_number > finalized_number {
			return Err(NativeError::new(
				NativeErrorCode::InconsistentSnapshot,
				"requested Orbis block is not finalized",
			));
		}
		let mut canonical_hash = finalized.hash();
		let mut canonical_header = finalized.header().clone();
		let mut canonical_number = finalized_number;
		while canonical_number > requested_number {
			canonical_hash = canonical_header.parent_hash;
			canonical_header = self
				.client
				.backend()
				.block_header(canonical_hash)
				.await
				.map_err(runtime_api_error)?
				.ok_or_else(|| {
					NativeError::new(
						NativeErrorCode::InconsistentSnapshot,
						"finalized Orbis ancestry is unavailable",
					)
				})?;
			canonical_number = canonical_header.number;
		}
		if !canonical_hash_at_height_matches(
			requested_hash,
			requested_number,
			canonical_hash,
			canonical_number,
		) {
			return Err(NativeError::new(
				NativeErrorCode::InconsistentSnapshot,
				"requested Orbis block is not on finalized ancestry",
			));
		}
		Ok(())
	}

	async fn agreement_page(
		&self,
		hash: &Hash32,
		method: &'static str,
		first: Value,
		page: &super::domains::PageRequest,
	) -> DomainResult<StorageProviderResponse> {
		let response: storage_api::Page<
			storage_api::AgreementInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
		> = self.call_at(hash, "StorageProviderApi", method, page_args(first, page)).await?;
		Ok(StorageProviderResponse::Agreements(finalized_page(
			hash,
			response.version,
			response.items.into_iter().map(|item| agreement_id(item.agreement_id)).collect(),
			response.next_cursor,
		)?))
	}

	async fn bucket_call(
		&self,
		hash: &Hash32,
		method: &'static str,
		args: Vec<Value>,
	) -> DomainResult<
		storage_api::Versioned<
			storage_api::BucketInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
		>,
	> {
		self.call_at(hash, "S3RegistryApi", method, args).await
	}
}

fn canonical_hash_at_height_matches<Hash: Eq>(
	requested_hash: Hash,
	requested_number: u32,
	canonical_hash: Hash,
	canonical_number: u32,
) -> bool {
	requested_number == canonical_number && requested_hash == canonical_hash
}

#[cfg(test)]
mod finalized_ancestry_tests {
	use super::canonical_hash_at_height_matches;

	#[test]
	fn rejects_same_height_fork_and_wrong_height() {
		assert!(canonical_hash_at_height_matches([1u8; 32], 7, [1u8; 32], 7));
		assert!(!canonical_hash_at_height_matches([1u8; 32], 7, [2u8; 32], 7));
		assert!(!canonical_hash_at_height_matches([1u8; 32], 7, [1u8; 32], 8));
	}
}

#[async_trait]
impl FinalizedReadBinding for OrbisFinalizedReadBinding {
	async fn identity_personhood(
		&self,
		read: &IdentityPersonhoodRead,
	) -> DomainResult<IdentityPersonhoodResponse> {
		read.validate()?;
		let hash = &read.finalized_block_hash;
		match &read.query {
			IdentityPersonhoodQuery::IdentityStatus { account } => {
				let response: identity_api::Versioned<identity_api::IdentityStatus> = self
					.call_at(
						hash,
						"IdentityPersonhoodApi",
						"identity_status",
						vec![account_arg(account)?],
					)
					.await?;
				let value = response.value;
				Ok(IdentityPersonhoodResponse::IdentityStatus(finalized_value(
					hash,
					response.version,
					Some(IdentityStatusView {
						registered: value.registered,
						judgement_count: value.judgement_count,
						requested: value.requested,
						reasonable: value.reasonable,
						known_good: value.known_good,
						out_of_date: value.out_of_date,
						low_quality: value.low_quality,
						erroneous: value.erroneous,
					}),
				)?))
			},
			IdentityPersonhoodQuery::PersonhoodStatus { account } => {
				let response: identity_api::Versioned<identity_api::PersonhoodStatus> = self
					.call_at(
						hash,
						"IdentityPersonhoodApi",
						"personhood_status",
						vec![account_arg(account)?],
					)
					.await?;
				let value = response.value;
				Ok(IdentityPersonhoodResponse::PersonhoodStatus(finalized_value(
					hash,
					response.version,
					Some(PersonhoodStatusView {
						full_personal_id: value.full_personal_id,
						full_recognized: value.full_recognized,
						lite_recognized: value.lite_recognized,
					}),
				)?))
			},
			IdentityPersonhoodQuery::AttestationAllowance { account } => {
				let response: identity_api::Versioned<identity_api::AttestationAllowance> = self
					.call_at(
						hash,
						"IdentityPersonhoodApi",
						"attestation_allowance",
						vec![account_arg(account)?],
					)
					.await?;
				Ok(IdentityPersonhoodResponse::AttestationAllowance(finalized_value(
					hash,
					response.version,
					Some(AttestationAllowanceView { remaining: response.value.remaining }),
				)?))
			},
		}
	}


	async fn attestation(&self, read: &AttestationRead) -> DomainResult<AttestationResponse> {
		read.validate()?;
		let hash = &read.finalized_block_hash;
		match &read.query {
			AttestationQuery::SchemaById { schema } => {
				type Wire = att_api::Versioned<
					att_api::ClientSchemaView<RuntimeAccountId, RuntimeBlockNumber, RuntimeHash>,
				>;
				let response: Wire = self
					.call_at(
						hash,
						"AttestationApi",
						"schema_by_id",
						vec![hash_arg(schema.as_hash())?],
					)
					.await?;
				Ok(AttestationResponse::Schema(finalized_value(
					hash,
					response.version,
					response.value.map(schema_view).transpose()?,
				)?))
			},
			AttestationQuery::AttestationById { attestation } => {
				type Wire = att_api::Versioned<
					att_api::AttestationView<RuntimeAccountId, RuntimeBlockNumber, RuntimeHash>,
				>;
				let response: Wire = self
					.call_at(
						hash,
						"AttestationApi",
						"attestation_by_id",
						vec![hash_arg(attestation.as_hash())?],
					)
					.await?;
				Ok(AttestationResponse::Attestation(finalized_value(
					hash,
					response.version,
					response.value.map(attestation_view).transpose()?,
				)?))
			},
			AttestationQuery::LiveStatus { attestation } => {
				let response: att_api::AttestationLiveStatus<RuntimeBlockNumber> = self
					.call_at(
						hash,
						"AttestationApi",
						"attestation_live_status",
						vec![hash_arg(attestation.as_hash())?],
					)
					.await?;
				let version = response.version;
				let value = LiveStatus {
					exists: response.exists,
					live: response.live,
					evaluated_at: response.evaluated_at,
					expiry: response.expiry,
					revoked_at: response.revoked_at,
				};
				Ok(AttestationResponse::LiveStatus(finalized_value(hash, version, Some(value))?))
			},
			AttestationQuery::CreatorSchemas { creator, page } => {
				let response: att_api::ClientIdPage<RuntimeHash> = self
					.call_at(
						hash,
						"AttestationApi",
						"creator_schemas",
						page_args(account_arg(creator)?, page),
					)
					.await?;
				Ok(AttestationResponse::Schemas(finalized_page(
					hash,
					response.version,
					response.items.into_iter().map(schema_id).collect(),
					response.next_cursor,
				)?))
			},
			AttestationQuery::IssuerAttestations { issuer, page } => {
				let response: att_api::ClientIdPage<RuntimeHash> = self
					.call_at(
						hash,
						"AttestationApi",
						"issuer_attestations",
						page_args(account_arg(issuer)?, page),
					)
					.await?;
				Ok(AttestationResponse::Attestations(finalized_page(
					hash,
					response.version,
					response.items.into_iter().map(attestation_id).collect(),
					response.next_cursor,
				)?))
			},
			AttestationQuery::SubjectSchemaAttestations { subject_commitment, schema, page } => {
				let mut args =
					vec![hash_arg(subject_commitment.as_hash())?, hash_arg(schema.as_hash())?];
				args.extend(page_tail(page));
				let response: att_api::ClientIdPage<RuntimeHash> = self
					.call_at(hash, "AttestationApi", "subject_schema_attestations", args)
					.await?;
				Ok(AttestationResponse::Attestations(finalized_page(
					hash,
					response.version,
					response.items.into_iter().map(attestation_id).collect(),
					response.next_cursor,
				)?))
			},
			AttestationQuery::NextDelegatedNonce { issuer } => {
				let response: att_api::DelegatedNonce = self
					.call_at(
						hash,
						"AttestationApi",
						"next_delegated_nonce",
						vec![account_arg(issuer)?],
					)
					.await?;
				Ok(AttestationResponse::NextDelegatedNonce(finalized_value(
					hash,
					response.version,
					Some(response.next_nonce),
				)?))
			},
			AttestationQuery::SchemaCount => {
				let response: att_api::RegistryCount =
					self.call_at(hash, "AttestationApi", "schema_count", vec![]).await?;
				Ok(AttestationResponse::SchemaCount(finalized_value(
					hash,
					response.version,
					Some(response.count),
				)?))
			},
			AttestationQuery::AttestationCount => {
				let response: att_api::RegistryCount =
					self.call_at(hash, "AttestationApi", "attestation_count", vec![]).await?;
				Ok(AttestationResponse::AttestationCount(finalized_value(
					hash,
					response.version,
					Some(response.count),
				)?))
			},
			AttestationQuery::NextIssuanceNonce { issuer } => {
				let response: att_api::IssuanceNonce = self
					.call_at(
						hash,
						"AttestationApi",
						"next_issuance_nonce",
						vec![account_arg(issuer)?],
					)
					.await?;
				Ok(AttestationResponse::NextIssuanceNonce(finalized_value(
					hash,
					response.version,
					Some(response.next_nonce),
				)?))
			},
			AttestationQuery::ExternalStatus { issuer, status_commitment } => {
				type Wire = att_api::Versioned<
					att_api::ExternalStatusView<RuntimeAccountId, RuntimeBlockNumber, RuntimeHash>,
				>;
				let response: Wire = self
					.call_at(
						hash,
						"AttestationApi",
						"external_status",
						vec![account_arg(issuer)?, hash_arg(status_commitment.as_hash())?],
					)
					.await?;
				Ok(AttestationResponse::ExternalStatus(finalized_value(
					hash,
					response.version,
					response.value.map(external_status_view).transpose()?,
				)?))
			},
		}
	}

	async fn dotns(&self, read: &DotnsRead) -> DomainResult<DotnsResponse> {
		read.validate()?;
		let hash = &read.finalized_block_hash;
		match &read.query {
			DotnsQuery::LabelPolicyVersion => {
				let response: u16 =
					self.call_at(hash, "DotnsApi", "label_policy_version", Vec::new()).await?;
				Ok(DotnsResponse::LabelPolicyVersion(finalized_value(
					hash,
					dotns_api::RESPONSE_VERSION,
					Some(response),
				)?))
			},
			DotnsQuery::NameById { name } => {
				type Wire = dotns_api::Versioned<
					dotns_api::ClientNameView<RuntimeAccountId, RuntimeBlockNumber, RuntimeHash>,
				>;
				let response: Wire = self
					.call_at(hash, "DotnsApi", "name_by_id", vec![hash_arg(name.as_hash())?])
					.await?;
				Ok(DotnsResponse::Name(finalized_value(
					hash,
					response.version,
					response.value.map(name_view).transpose()?,
				)?))
			},
			DotnsQuery::RootByLabel { label } => {
				let response: dotns_api::Versioned<RuntimeHash> = self
					.call_at(
						hash,
						"DotnsApi",
						"root_name_by_normalized_label",
						vec![Value::from_bytes(label.as_str().as_bytes())],
					)
					.await?;
				Ok(DotnsResponse::NameId(finalized_value(
					hash,
					response.version,
					response.value.map(name_id),
				)?))
			},
			DotnsQuery::OwnerNames { owner, page } => {
				let response: dotns_api::ClientOwnerNamesPage<RuntimeHash> = self
					.call_at(hash, "DotnsApi", "owner_names", page_args(account_arg(owner)?, page))
					.await?;
				Ok(DotnsResponse::Names(finalized_page(
					hash,
					response.version,
					response.names.into_iter().map(name_id).collect(),
					response.next_cursor,
				)?))
			},
			DotnsQuery::Controllers { name } => {
				let response: dotns_api::Versioned<Vec<RuntimeAccountId>> = self
					.call_at(hash, "DotnsApi", "controllers", vec![hash_arg(name.as_hash())?])
					.await?;
				Ok(DotnsResponse::Controllers(finalized_value(
					hash,
					response.version,
					response
						.value
						.map(|items| items.iter().map(account_id).collect())
						.transpose()?,
				)?))
			},
			DotnsQuery::ResolveAddress { name } => {
				let response: dotns_api::Versioned<Vec<u8>> = self
					.call_at(hash, "DotnsApi", "resolve_address", vec![hash_arg(name.as_hash())?])
					.await?;
				Ok(DotnsResponse::Address(finalized_value(
					hash,
					response.version,
					response.value.map(Address::new).transpose()?,
				)?))
			},
			DotnsQuery::ResolveSubject { name } => {
				let response: dotns_api::Versioned<RuntimeSubjectId> = self
					.call_at(hash, "DotnsApi", "resolve_subject", vec![hash_arg(name.as_hash())?])
					.await?;
				Ok(DotnsResponse::Subject(finalized_value(
					hash,
					response.version,
					response.value.map(subject_id).transpose()?,
				)?))
			},
			DotnsQuery::ResolveAttestation { name } => {
				let response: dotns_api::Versioned<RuntimeHash> = self
					.call_at(
						hash,
						"DotnsApi",
						"resolve_attestation",
						vec![hash_arg(name.as_hash())?],
					)
					.await?;
				Ok(DotnsResponse::Attestation(finalized_value(
					hash,
					response.version,
					response.value.map(attestation_id),
				)?))
			},
			DotnsQuery::ResolveContent { name } => {
				let response: dotns_api::Versioned<[u8; 32]> = self
					.call_at(hash, "DotnsApi", "resolve_content", vec![hash_arg(name.as_hash())?])
					.await?;
				Ok(DotnsResponse::Content(finalized_value(
					hash,
					response.version,
					response.value.map(content_id),
				)?))
			},
			DotnsQuery::ResolveText { name, key } => {
				let response: dotns_api::Versioned<Vec<u8>> = self
					.call_at(
						hash,
						"DotnsApi",
						"resolve_text",
						vec![hash_arg(name.as_hash())?, Value::from_bytes(key.as_bytes())],
					)
					.await?;
				Ok(DotnsResponse::Text(finalized_value(
					hash,
					response.version,
					response.value.map(TextValue::new).transpose()?,
				)?))
			},
			DotnsQuery::PrimaryName { owner } => {
				let response: dotns_api::Versioned<RuntimeHash> = self
					.call_at(hash, "DotnsApi", "primary_name", vec![account_arg(owner)?])
					.await?;
				Ok(DotnsResponse::NameId(finalized_value(
					hash,
					response.version,
					response.value.map(name_id),
				)?))
			},
			DotnsQuery::NameStatus { name } => {
				let response: dotns_api::NameStatus<RuntimeBlockNumber> = self
					.call_at(hash, "DotnsApi", "name_status", vec![hash_arg(name.as_hash())?])
					.await?;
				let version = response.version;
				let value = DomainNameStatus {
					exists: response.exists,
					active: response.active,
					expires_at: response.expires_at,
				};
				Ok(DotnsResponse::Status(finalized_value(hash, version, Some(value))?))
			},
		}
	}

	async fn bulletin_storage(&self, read: &StorageRead) -> DomainResult<StorageResponse> {
		read.validate()?;
		let hash = &read.finalized_block_hash;
		match &read.query {
			StorageQuery::AccountAuthorization { account } => {
				let response: Option<bulletin_api::AccountAuthorization<RuntimeBlockNumber>> = self
					.call_at(
						hash,
						"BulletinTransactionStorageApi",
						"account_authorization",
						vec![account_arg(account)?],
					)
					.await?;
				Ok(StorageResponse::AccountAuthorization(finalized_value(
					hash,
					1,
					response.map(account_authorization),
				)?))
			},
			StorageQuery::CanStore { account, data_len } => {
				let response: bool = self
					.call_at(
						hash,
						"BulletinTransactionStorageApi",
						"can_store",
						vec![account_arg(account)?, Value::u128(*data_len as u128)],
					)
					.await?;
				Ok(StorageResponse::CanStore(finalized_value(hash, 1, Some(response))?))
			},
			StorageQuery::CanRenew { account, entry } => {
				let response: bool = self
					.call_at(
						hash,
						"BulletinTransactionStorageApi",
						"can_renew",
						vec![account_arg(account)?, transaction_ref_arg(entry)?],
					)
					.await?;
				Ok(StorageResponse::CanRenew(finalized_value(hash, 1, Some(response))?))
			},
			StorageQuery::StoredContentProvenance { reference } => {
				let response: Option<bulletin_api::ClientStorageActor<RuntimeAccountId>> = self
					.call_at(
						hash,
						"BulletinTransactionStorageApi",
						"stored_content_provenance",
						vec![bulletin_ref_arg(reference)],
					)
					.await?;
				Ok(StorageResponse::StoredContentProvenance(finalized_value(
					hash,
					1,
					response.map(storage_actor).transpose()?,
				)?))
			},
			StorageQuery::ResourceReservation { reservation_id } => {
				let response: Option<
					bulletin_api::ClientResourceReservationView<
						RuntimeAccountId,
						RuntimeBlockNumber,
					>,
				> = self
					.call_at(
						hash,
						"BulletinTransactionStorageApi",
						"resource_reservation",
						vec![Value::u128(reservation_id.as_u64()? as u128)],
					)
					.await?;
				Ok(StorageResponse::ResourceReservation(finalized_value(
					hash,
					1,
					response.map(resource_reservation).transpose()?,
				)?))
			},
			StorageQuery::ResourceReservationLink { reservation_id, content_hash } => {
				let response: Option<
					bulletin_api::ClientResourceReservationLink<
						RuntimeAccountId,
						RuntimeBlockNumber,
					>,
				> = self
					.call_at(
						hash,
						"BulletinTransactionStorageApi",
						"resource_reservation_link",
						vec![
							Value::u128(reservation_id.as_u64()? as u128),
							hash_arg(content_hash.as_hash())?,
						],
					)
					.await?;
				Ok(StorageResponse::ResourceReservationLink(finalized_value(
					hash,
					1,
					response.map(resource_reservation_link).transpose()?,
				)?))
			},
			StorageQuery::ResourceProviderRef { reservation_id } => {
				let response: Option<[u8; 32]> = self
					.call_at(
						hash,
						"BulletinTransactionStorageApi",
						"resource_provider_ref",
						vec![Value::u128(reservation_id.as_u64()? as u128)],
					)
					.await?;
				Ok(StorageResponse::ResourceProviderRef(finalized_value(
					hash,
					1,
					response.map(|value| ProviderReference(Hash32::from_bytes(value))),
				)?))
			},
		}
	}

	async fn storage_provider(
		&self,
		read: &StorageProviderRead,
	) -> DomainResult<StorageProviderResponse> {
		read.validate()?;
		let hash = &read.finalized_block_hash;
		match &read.query {
			StorageProviderQuery::ProviderById { provider } => {
				let response: storage_api::Versioned<
					storage_api::ProviderInfo<RuntimeBlockNumber>,
				> = self
					.call_at(hash, "StorageProviderApi", "provider", vec![account_arg(provider)?])
					.await?;
				let value =
					response.value.map(|info| provider_view(provider.clone(), info)).transpose()?;
				Ok(StorageProviderResponse::Provider(finalized_value(
					hash,
					response.version,
					value,
				)?))
			},
			StorageProviderQuery::Providers { page } => {
				let response: storage_api::Page<(
					RuntimeAccountId,
					storage_api::ProviderInfo<RuntimeBlockNumber>,
				)> = self
					.call_at(hash, "StorageProviderApi", "providers", page_tail(page).into())
					.await?;
				let items = response
					.items
					.iter()
					.map(|(account, _)| account_id(account))
					.collect::<DomainResult<Vec<_>>>()?;
				Ok(StorageProviderResponse::Providers(finalized_page(
					hash,
					response.version,
					items,
					response.next_cursor,
				)?))
			},
			StorageProviderQuery::AgreementById { agreement } => {
				let response: storage_api::Versioned<
					storage_api::AgreementInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(
						hash,
						"StorageProviderApi",
						"agreement",
						vec![hash_arg(agreement.as_hash())?],
					)
					.await?;
				Ok(StorageProviderResponse::Agreement(finalized_value(
					hash,
					response.version,
					response.value.map(agreement_view).transpose()?,
				)?))
			},
			StorageProviderQuery::ProviderAgreements { provider, page } => {
				self.agreement_page(hash, "provider_agreements", account_arg(provider)?, page)
					.await
			},
			StorageProviderQuery::OwnerAgreements { owner, page } => {
				self.agreement_page(hash, "owner_agreements", account_arg(owner)?, page).await
			},
			StorageProviderQuery::ContainerAgreements { container, page } => {
				self.agreement_page(
					hash,
					"container_agreements",
					hash_arg(container.as_hash())?,
					page,
				)
				.await
			},
			StorageProviderQuery::AgreementNonce { owner } => {
				let nonce: u64 = self
					.call_at(
						hash,
						"StorageProviderApi",
						"agreement_nonce",
						vec![account_arg(owner)?],
					)
					.await?;
				Ok(StorageProviderResponse::AgreementNonce(finalized_value(
					hash,
					storage_api::RESPONSE_VERSION,
					Some(nonce),
				)?))
			},
			StorageProviderQuery::ChallengeById { challenge } => {
				let response: storage_api::Versioned<
					storage_api::ChallengeInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(
						hash,
						"StorageProviderApi",
						"challenge",
						vec![hash_arg(challenge.as_hash())?],
					)
					.await?;
				Ok(StorageProviderResponse::Challenge(finalized_value(
					hash,
					response.version,
					response.value.map(challenge_view).transpose()?,
				)?))
			},
			StorageProviderQuery::ChallengesAt { block, page } => {
				let mut args = vec![Value::u128(*block as u128)];
				args.extend(page_tail(page));
				let response: storage_api::Page<
					storage_api::ChallengeInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
				> = self.call_at(hash, "StorageProviderApi", "challenges_at", args).await?;
				Ok(StorageProviderResponse::Challenges(finalized_page(
					hash,
					response.version,
					response
						.items
						.into_iter()
						.map(|item| challenge_id(item.challenge_id))
						.collect(),
					response.next_cursor,
				)?))
			},
			StorageProviderQuery::OpenChallengeCount { agreement } => {
				let count: u32 = self
					.call_at(
						hash,
						"StorageProviderApi",
						"open_challenge_count",
						vec![hash_arg(agreement.as_hash())?],
					)
					.await?;
				Ok(StorageProviderResponse::OpenChallengeCount(finalized_value(
					hash,
					storage_api::RESPONSE_VERSION,
					Some(count),
				)?))
			},
			StorageProviderQuery::CanAcceptCapacity { provider, additional_bytes } => {
				let accepted: bool = self
					.call_at(
						hash,
						"StorageProviderApi",
						"can_accept_capacity",
						vec![account_arg(provider)?, Value::u128(*additional_bytes as u128)],
					)
					.await?;
				Ok(StorageProviderResponse::CanAcceptCapacity(finalized_value(
					hash,
					storage_api::RESPONSE_VERSION,
					Some(accepted),
				)?))
			},
			StorageProviderQuery::ProviderCheckpoint { provider } => {
				let response: storage_api::Versioned<
					storage_api::CheckpointInfo<RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(hash, "StorageProviderApi", "checkpoint", vec![account_arg(provider)?])
					.await?;
				Ok(StorageProviderResponse::Checkpoint(finalized_value(
					hash,
					response.version,
					response.value.map(checkpoint_view),
				)?))
			},
			StorageProviderQuery::ProviderRoot { provider } => {
				let response: storage_api::Versioned<
					storage_api::ProviderRootInfo<RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(
						hash,
						"StorageProviderApi",
						"provider_root",
						vec![account_arg(provider)?],
					)
					.await?;
				Ok(StorageProviderResponse::ProviderRoot(finalized_value(
					hash,
					response.version,
					response.value.map(|info| ProviderRootView {
						sequence: info.sequence,
						root: ProofCommitment(domain_hash(info.root)),
						leaf_count: info.leaf_count,
						committed_at: info.committed_at,
					}),
				)?))
			},
			StorageProviderQuery::DeletionAcknowledgement { agreement } => {
				let response: storage_api::Versioned<
					storage_api::DeletionAcknowledgementInfo<
						RuntimeAccountId,
						RuntimeHash,
						RuntimeBlockNumber,
					>,
				> = self
					.call_at(
						hash,
						"StorageProviderApi",
						"deletion_acknowledgement",
						vec![hash_arg(agreement.as_hash())?],
					)
					.await?;
				Ok(StorageProviderResponse::DeletionAcknowledgement(finalized_value(
					hash,
					response.version,
					response.value.map(deletion_acknowledgement_view).transpose()?,
				)?))
			},
			StorageProviderQuery::ResourceProviderRef { reservation_id } => {
				let response: Option<[u8; 32]> = self
					.call_at(
						hash,
						"BulletinTransactionStorageApi",
						"resource_provider_ref",
						vec![Value::u128(reservation_id.as_u64()? as u128)],
					)
					.await?;
				Ok(StorageProviderResponse::ResourceProviderRef(finalized_value(
					hash,
					1,
					response.map(|value| ProviderReference(Hash32::from_bytes(value))),
				)?))
			},
		}
	}

	async fn drive(&self, read: &DriveRead) -> DomainResult<DriveResponse> {
		read.validate()?;
		let hash = &read.finalized_block_hash;
		match &read.query {
			DriveQuery::DriveById { drive } => {
				let response: storage_api::Versioned<
					storage_api::DriveInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(hash, "DriveRegistryApi", "drive", vec![hash_arg(drive.as_hash())?])
					.await?;
				Ok(DriveResponse::Drive(finalized_value(
					hash,
					response.version,
					response.value.map(drive_view).transpose()?,
				)?))
			},
			DriveQuery::OwnerDrives { owner, page } => {
				let response: storage_api::Page<
					storage_api::DriveInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(
						hash,
						"DriveRegistryApi",
						"drives",
						page_args(account_arg(owner)?, page),
					)
					.await?;
				Ok(DriveResponse::Drives(finalized_page(
					hash,
					response.version,
					response.items.into_iter().map(|item| drive_id(item.drive_id)).collect(),
					response.next_cursor,
				)?))
			},
			DriveQuery::Controllers { drive, page } => {
				let response: storage_api::Page<RuntimeAccountId> = self
					.call_at(
						hash,
						"DriveRegistryApi",
						"controllers",
						page_args(hash_arg(drive.as_hash())?, page),
					)
					.await?;
				let items =
					response.items.iter().map(account_id).collect::<DomainResult<Vec<_>>>()?;
				Ok(DriveResponse::Controllers(finalized_page(
					hash,
					response.version,
					items,
					response.next_cursor,
				)?))
			},
			DriveQuery::NextDriveNonce { owner } => {
				let nonce: u64 = self
					.call_at(
						hash,
						"DriveRegistryApi",
						"next_drive_nonce",
						vec![account_arg(owner)?],
					)
					.await?;
				Ok(DriveResponse::NextDriveNonce(finalized_value(
					hash,
					storage_api::RESPONSE_VERSION,
					Some(nonce),
				)?))
			},
			DriveQuery::IsDriveOwner { owner, drive } => {
				let is_owner: bool = self
					.call_at(
						hash,
						"DriveRegistryApi",
						"is_drive_owner",
						vec![account_arg(owner)?, hash_arg(drive.as_hash())?],
					)
					.await?;
				Ok(DriveResponse::IsDriveOwner(finalized_value(
					hash,
					storage_api::RESPONSE_VERSION,
					Some(is_owner),
				)?))
			},
		}
	}

	async fn s3(&self, read: &S3Read) -> DomainResult<S3Response> {
		read.validate()?;
		let hash = &read.finalized_block_hash;
		match &read.query {
			S3Query::BucketById { bucket } => {
				let response =
					self.bucket_call(hash, "bucket", vec![hash_arg(bucket.as_hash())?]).await?;
				Ok(S3Response::Bucket(finalized_value(
					hash,
					response.version,
					response.value.map(bucket_view).transpose()?,
				)?))
			},
			S3Query::BucketByName { name } => {
				let response = self
					.bucket_call(
						hash,
						"bucket_by_name",
						vec![Value::from_bytes(name.as_str().as_bytes())],
					)
					.await?;
				let id = response.value.map(|bucket| bucket_id(bucket.bucket_id));
				Ok(S3Response::BucketId(finalized_value(hash, response.version, id)?))
			},
			S3Query::OwnerBuckets { owner, page } => {
				let response: storage_api::Page<
					storage_api::BucketInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(hash, "S3RegistryApi", "buckets", page_args(account_arg(owner)?, page))
					.await?;
				Ok(S3Response::Buckets(finalized_page(
					hash,
					response.version,
					response.items.into_iter().map(|item| bucket_id(item.bucket_id)).collect(),
					response.next_cursor,
				)?))
			},
			S3Query::BucketObjects { bucket, page } => {
				let response: storage_api::Page<Vec<u8>> = self
					.call_at(
						hash,
						"S3RegistryApi",
						"object_keys",
						page_args(hash_arg(bucket.as_hash())?, page),
					)
					.await?;
				let items = response
					.items
					.into_iter()
					.map(ObjectKey::new)
					.collect::<DomainResult<Vec<_>>>()?;
				Ok(S3Response::Objects(finalized_page(
					hash,
					response.version,
					items,
					response.next_cursor,
				)?))
			},
			S3Query::ObjectByKey { bucket, key } => {
				let response: storage_api::Versioned<
					storage_api::ObjectInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(
						hash,
						"S3RegistryApi",
						"object",
						vec![hash_arg(bucket.as_hash())?, Value::from_bytes(key.as_bytes())],
					)
					.await?;
				Ok(S3Response::Object(finalized_value(
					hash,
					response.version,
					response.value.map(object_view).transpose()?,
				)?))
			},
			S3Query::ObjectHistory { bucket, key, page } => {
				let mut args = vec![hash_arg(bucket.as_hash())?, Value::from_bytes(key.as_bytes())];
				args.extend(page_tail(page));
				let response: storage_api::Page<
					storage_api::ObjectVersionInfo<RuntimeAccountId, RuntimeBlockNumber>,
				> = self.call_at(hash, "S3RegistryApi", "object_history", args).await?;
				let items = response
					.items
					.into_iter()
					.map(object_version_view)
					.collect::<DomainResult<Vec<_>>>()?;
				Ok(S3Response::ObjectHistory(finalized_page(
					hash,
					response.version,
					items,
					response.next_cursor,
				)?))
			},
			S3Query::ObjectId { bucket, key } => {
				let response: storage_api::Versioned<RuntimeHash> = self
					.call_at(
						hash,
						"S3RegistryApi",
						"object_id",
						vec![hash_arg(bucket.as_hash())?, Value::from_bytes(key.as_bytes())],
					)
					.await?;
				Ok(S3Response::ObjectId(finalized_value(
					hash,
					response.version,
					response.value.map(object_id),
				)?))
			},
			S3Query::IsBucketOwner { owner, bucket } => {
				let is_owner: bool = self
					.call_at(
						hash,
						"S3RegistryApi",
						"is_bucket_owner",
						vec![account_arg(owner)?, hash_arg(bucket.as_hash())?],
					)
					.await?;
				Ok(S3Response::IsBucketOwner(finalized_value(
					hash,
					storage_api::RESPONSE_VERSION,
					Some(is_owner),
				)?))
			},
		}
	}
}

fn schema_view(
	view: att_api::ClientSchemaView<RuntimeAccountId, RuntimeBlockNumber, RuntimeHash>,
) -> DomainResult<DomainSchemaView> {
	Ok(DomainSchemaView {
		schema: schema_id(view.schema),
		creator: account_id(&view.creator)?,
		definition: view.definition,
		definition_commitment: domain_hash(view.definition_commitment),
		status: match view.status {
			att_api::SchemaStatus::Active => DomainSchemaStatus::Active,
			att_api::SchemaStatus::Paused => DomainSchemaStatus::Paused,
			att_api::SchemaStatus::Retired => DomainSchemaStatus::Retired,
		},
		revocable: view.revocable,
		unique: view.unique,
		index_policy: match view.index_policy {
			att_api::IndexPolicy::None => DomainIndexPolicy::None,
			att_api::IndexPolicy::Issuer => DomainIndexPolicy::Issuer,
			att_api::IndexPolicy::SubjectAndSchema => DomainIndexPolicy::SubjectAndSchema,
			att_api::IndexPolicy::IssuerAndSubjectSchema => {
				DomainIndexPolicy::IssuerAndSubjectSchema
			},
		},
		authorized_issuers: view
			.authorized_issuers
			.iter()
			.map(account_id)
			.collect::<DomainResult<Vec<_>>>()?,
		created_at: view.created_at,
	})
}

fn attestation_view(
	view: att_api::AttestationView<RuntimeAccountId, RuntimeBlockNumber, RuntimeHash>,
) -> DomainResult<DomainAttestationView> {
	Ok(DomainAttestationView {
		attestation: attestation_id(view.attestation),
		issuer: account_id(&view.issuer)?,
		schema: schema_id(view.schema),
		subject_commitment: super::domains::SubjectCommitment(domain_hash(view.subject_commitment)),
		payload_commitment: super::domains::PayloadCommitment(domain_hash(view.payload_commitment)),
		status_commitment: super::domains::StatusCommitment(domain_hash(view.status_commitment)),
		parent: view.parent.map(attestation_id),
		expiry: view.expiry,
		uniqueness_commitment: view
			.uniqueness_commitment
			.map(|hash| UniquenessCommitment(domain_hash(hash))),
		revocable: view.revocable,
		issuance_nonce: view.issuance_nonce,
		issued_at: view.issued_at,
		revoked_at: view.revoked_at,
		revoked_by: view.revoked_by.as_ref().map(account_id).transpose()?,
	})
}

fn external_status_view(
	view: att_api::ExternalStatusView<RuntimeAccountId, RuntimeBlockNumber, RuntimeHash>,
) -> DomainResult<DomainExternalStatusView> {
	Ok(DomainExternalStatusView {
		key: domain_hash(view.key),
		issuer: account_id(&view.issuer)?,
		status_commitment: super::domains::StatusCommitment(domain_hash(view.status_commitment)),
		revoked_at: view.revoked_at,
	})
}

fn name_view(
	view: dotns_api::ClientNameView<RuntimeAccountId, RuntimeBlockNumber, RuntimeHash>,
) -> DomainResult<DomainNameView> {
	let label = String::from_utf8(view.label).map_err(|_| {
		NativeError::new(NativeErrorCode::UnsupportedRuntime, "DotNS label is not UTF-8")
	})?;
	Ok(DomainNameView {
		name: name_id(view.name),
		parent: view.parent.map(name_id),
		label: Label::new(label)?,
		owner: account_id(&view.owner)?,
		expires_at: view.expires_at,
		depth: view.depth,
	})
}

fn account_authorization(
	authorization: bulletin_api::AccountAuthorization<RuntimeBlockNumber>,
) -> DomainAccountAuthorization {
	DomainAccountAuthorization {
		expires_at: authorization.expires_at,
		bytes_allowance: DecimalU64::from_u64(authorization.bytes_allowance),
		bytes_used: DecimalU64::from_u64(authorization.bytes_used),
		bytes_permanent_used: DecimalU64::from_u64(authorization.bytes_permanent_used),
		transactions_allowance: authorization.transactions_allowance,
		transactions_used: authorization.transactions_used,
	}
}

fn storage_actor(
	actor: bulletin_api::ClientStorageActor<RuntimeAccountId>,
) -> DomainResult<StorageActor> {
	Ok(match actor {
		bulletin_api::ClientStorageActor::Account(account) => {
			StorageActor::Account { account: account_id(&account)? }
		},
		bulletin_api::ClientStorageActor::Root => StorageActor::Root,
		bulletin_api::ClientStorageActor::Preimage(content_hash) => {
			StorageActor::Preimage { content_hash: ContentHash(Hash32::from_bytes(content_hash)) }
		},
		bulletin_api::ClientStorageActor::AutoRenew(account) => {
			StorageActor::AutoRenew { account: account_id(&account)? }
		},
	})
}

fn resource_reservation(
	view: bulletin_api::ClientResourceReservationView<RuntimeAccountId, RuntimeBlockNumber>,
) -> DomainResult<ResourceReservationView> {
	Ok(match view {
		bulletin_api::ClientResourceReservationView::Active(active) => {
			ResourceReservationView::Active(ActiveResourceReservation {
				owner: account_id(&active.owner)?,
				purpose_digest: ContentHash(Hash32::from_bytes(active.purpose_digest)),
				bytes_remaining: DecimalU64::from_u64(active.bytes_remaining),
				transactions_remaining: active.transactions_remaining,
				created_at: active.created_at,
				expires_at: active.expires_at,
			})
		},
		bulletin_api::ClientResourceReservationView::Tombstone(tombstone) => {
			let outcome = match tombstone.outcome {
				bulletin_api::ClientResourceClosure::Cancelled => ResourceClosure::Cancelled,
				bulletin_api::ClientResourceClosure::Expired => ResourceClosure::Expired,
				bulletin_api::ClientResourceClosure::Exhausted => ResourceClosure::Exhausted,
			};
			ResourceReservationView::Tombstone(ResourceReservationTombstone {
				owner: account_id(&tombstone.owner)?,
				purpose_digest: ContentHash(Hash32::from_bytes(tombstone.purpose_digest)),
				final_bytes_remaining: DecimalU64::from_u64(tombstone.final_bytes_remaining),
				final_transactions_remaining: tombstone.final_transactions_remaining,
				outcome,
				closed_at: tombstone.closed_at,
			})
		},
	})
}

fn resource_reservation_link(
	link: bulletin_api::ClientResourceReservationLink<RuntimeAccountId, RuntimeBlockNumber>,
) -> DomainResult<DomainResourceReservationLink> {
	Ok(DomainResourceReservationLink {
		reservation_id: ReservationId::from_u64(link.reservation_id),
		content_hash: ContentHash(Hash32::from_bytes(link.content_hash)),
		bulletin_ref: DomainBulletinRef {
			block: link.bulletin_ref.block,
			transaction_index: link.bulletin_ref.transaction_index,
		},
		owner: account_id(&link.owner)?,
		size: link.size,
		retention_boundary: link.retention_boundary,
	})
}

fn provider_view(
	provider: AccountId,
	info: storage_api::ProviderInfo<RuntimeBlockNumber>,
) -> DomainResult<ProviderView> {
	Ok(ProviderView {
		provider,
		endpoint: Endpoint::new(info.endpoint)?,
		service_key: ServiceKey::new(info.service_key)?,
		capacity_bytes: info.capacity_bytes,
		allocated_bytes: info.allocated_bytes,
		pending_bytes: info.pending_bytes,
		status: match info.status {
			storage_api::ProviderStatus::Active => ProviderStatus::Active,
			storage_api::ProviderStatus::Suspended => ProviderStatus::Suspended,
		},
		last_heartbeat: info.last_heartbeat,
		reputation: info.reputation,
	})
}

fn agreement_view(
	info: storage_api::AgreementInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
) -> DomainResult<AgreementView> {
	Ok(AgreementView {
		agreement: agreement_id(info.agreement_id),
		owner: account_id(&info.owner)?,
		provider: account_id(&info.provider)?,
		container: container_id(info.container_ref),
		content_commitment: ContentCommitment(domain_hash(info.content_commitment)),
		reservation_ref: info.reservation_ref.map(ReservationId::from_u64),
		bytes: info.bytes,
		created_at: info.created_at,
		expires_at: info.expires_at,
		pending_expiry: info.pending_expiry,
		status: match info.status {
			storage_api::AgreementStatus::Proposed => AgreementStatus::Proposed,
			storage_api::AgreementStatus::Active => AgreementStatus::Active,
			storage_api::AgreementStatus::Cancelled => AgreementStatus::Cancelled,
			storage_api::AgreementStatus::Expired => AgreementStatus::Expired,
		},
	})
}

fn challenge_view(
	info: storage_api::ChallengeInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
) -> DomainResult<ChallengeView> {
	Ok(ChallengeView {
		challenge: challenge_id(info.challenge_id),
		provider: account_id(&info.provider)?,
		agreement: agreement_id(info.agreement_id),
		expected_commitment: ProofCommitment(domain_hash(info.expected_commitment)),
		due_at: info.due_at,
		proof_commitment: info.proof_commitment.map(|hash| ProofCommitment(domain_hash(hash))),
		status: match info.status {
			storage_api::ChallengeStatus::Open => ChallengeStatus::Open,
			storage_api::ChallengeStatus::Proved => ChallengeStatus::Proved,
			storage_api::ChallengeStatus::TimedOut => ChallengeStatus::TimedOut,
		},
	})
}

fn checkpoint_view(
	info: storage_api::CheckpointInfo<RuntimeHash, RuntimeBlockNumber>,
) -> CheckpointView {
	CheckpointView {
		challenge: challenge_id(info.challenge_id),
		proof_commitment: ProofCommitment(domain_hash(info.proof_commitment)),
		recorded_at: info.recorded_at,
	}
}

fn deletion_acknowledgement_view(
	info: storage_api::DeletionAcknowledgementInfo<
		RuntimeAccountId,
		RuntimeHash,
		RuntimeBlockNumber,
	>,
) -> DomainResult<DeletionAcknowledgementView> {
	Ok(DeletionAcknowledgementView {
		provider: account_id(&info.provider)?,
		content_commitment: ContentCommitment(domain_hash(info.content_commitment)),
		tombstone_root: ProofCommitment(domain_hash(info.tombstone_root)),
		root_sequence: info.root_sequence,
		leaf_index: info.leaf_index,
		leaf_count: info.leaf_count,
		proof_commitment: ProofCommitment(domain_hash(info.proof_commitment)),
		acknowledged_at: info.acknowledged_at,
	})
}

fn drive_view(
	info: storage_api::DriveInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
) -> DomainResult<DriveView> {
	let status = match info.status {
		storage_api::ContainerStatus::Active => DriveStatus::Active,
		storage_api::ContainerStatus::Archived => DriveStatus::Archived,
		storage_api::ContainerStatus::Deleted => {
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedRuntime,
				"Drive runtime API returned deleted status",
			));
		},
	};
	Ok(DriveView {
		drive: drive_id(info.drive_id),
		owner: account_id(&info.owner)?,
		name: DriveName::new(info.name)?,
		root_storage_ref: info
			.root_storage_ref
			.map(|hash| ContentCommitment(Hash32::from_bytes(hash))),
		version: info.version,
		status,
		created_at: info.created_at,
		updated_at: info.updated_at,
	})
}

fn bucket_view(
	info: storage_api::BucketInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
) -> DomainResult<BucketView> {
	let name = String::from_utf8(info.name).map_err(|_| {
		NativeError::new(NativeErrorCode::UnsupportedRuntime, "S3 bucket name is not UTF-8")
	})?;
	Ok(BucketView {
		bucket: bucket_id(info.bucket_id),
		name: BucketName::new(name)?,
		owner: account_id(&info.owner)?,
		controllers: info.controllers.iter().map(account_id).collect::<DomainResult<Vec<_>>>()?,
		status: match info.status {
			storage_api::ContainerStatus::Active => BucketStatus::Active,
			storage_api::ContainerStatus::Archived => BucketStatus::Archived,
			storage_api::ContainerStatus::Deleted => BucketStatus::Deleted,
		},
		versioning_enabled: info.versioning_enabled,
		version: info.version,
		live_objects: info.live_objects,
		created_at: info.created_at,
		updated_at: info.updated_at,
	})
}

fn object_view(
	info: storage_api::ObjectInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
) -> DomainResult<ObjectView> {
	Ok(ObjectView {
		object: object_id(info.object_id),
		bucket: bucket_id(info.bucket_id),
		key: ObjectKey::new(info.key)?,
		content: info.content_hash.map(|hash| ContentCommitment(Hash32::from_bytes(hash))),
		version: info.version,
		deleted: info.deleted,
		updated_by: account_id(&info.updated_by)?,
		updated_at: info.updated_at,
	})
}

fn object_version_view(
	info: storage_api::ObjectVersionInfo<RuntimeAccountId, RuntimeBlockNumber>,
) -> DomainResult<ObjectVersionView> {
	Ok(ObjectVersionView {
		content: info.content_hash.map(|hash| ContentCommitment(Hash32::from_bytes(hash))),
		version: info.version,
		deleted: info.deleted,
		updated_by: account_id(&info.updated_by)?,
		updated_at: info.updated_at,
	})
}

fn finalized_value<T>(
	hash: &Hash32,
	version: u16,
	value: Option<T>,
) -> DomainResult<FinalizedValue<T>> {
	ensure_response_version(version)?;
	let response = FinalizedValue { version, finalized_block_hash: hash.clone(), value };
	response.validate_envelope()?;
	Ok(response)
}

fn finalized_page<T>(
	hash: &Hash32,
	version: u16,
	items: Vec<T>,
	next_cursor: Option<u32>,
) -> DomainResult<FinalizedPage<T>> {
	ensure_response_version(version)?;
	let response =
		FinalizedPage { version, finalized_block_hash: hash.clone(), items, next_cursor };
	response.validate_envelope()?;
	Ok(response)
}

fn ensure_response_version(version: u16) -> DomainResult<()> {
	if version != 1 {
		return Err(NativeError::new(
			NativeErrorCode::UnsupportedRuntime,
			format!("unsupported Orbis runtime API response version {version}"),
		));
	}
	Ok(())
}

fn runtime_hash(hash: &Hash32) -> DomainResult<RuntimeHash> {
	let bytes = hash_bytes(hash)?;
	Ok(RuntimeHash::from(bytes))
}

fn hash_arg(hash: &Hash32) -> DomainResult<Value> {
	Ok(Value::from_bytes(hash_bytes(hash)?))
}

fn hash_bytes(hash: &Hash32) -> DomainResult<[u8; 32]> {
	hash.validate()?;
	let raw = hex::decode(&hash.as_str()[2..])
		.map_err(|_| NativeError::new(NativeErrorCode::InvalidInput, "invalid hash hex"))?;
	raw.try_into()
		.map_err(|_| NativeError::new(NativeErrorCode::InvalidInput, "invalid hash length"))
}

fn account_arg(account: &AccountId) -> DomainResult<Value> {
	let account = crate::types::account::ss58_to_account_id(account.as_str())
		.map_err(|error| NativeError::new(NativeErrorCode::InvalidInput, error.to_string()))?;
	let bytes: &[u8; 32] = account.as_ref();
	Ok(Value::from_bytes(bytes))
}

fn bulletin_ref_arg(reference: &DomainBulletinRef) -> Value {
	Value::named_composite(vec![
		("block", Value::u128(reference.block as u128)),
		("transaction_index", Value::u128(reference.transaction_index as u128)),
	])
}

fn transaction_ref_arg(reference: &TransactionRef) -> DomainResult<Value> {
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
			Composite::unnamed(vec![hash_arg(content_hash.as_hash())?]),
		),
	})
}

fn page_args(first: Value, page: &super::domains::PageRequest) -> Vec<Value> {
	let mut args = vec![first];
	args.extend(page_tail(page));
	args
}

fn page_tail(page: &super::domains::PageRequest) -> [Value; 2] {
	[option_u32(page.cursor), Value::u128(page.limit as u128)]
}

fn option_u32(value: Option<u32>) -> Value {
	match value {
		Some(value) => Value::variant("Some", Composite::unnamed(vec![Value::u128(value as u128)])),
		None => Value::variant("None", Composite::unnamed(Vec::new())),
	}
}

fn account_id(account: &RuntimeAccountId) -> DomainResult<AccountId> {
	AccountId::new(account_id_to_ss58(&account_id_from_subxt(account)))
}

fn domain_hash(hash: RuntimeHash) -> Hash32 {
	Hash32::from_bytes(*hash.as_fixed_bytes())
}

fn schema_id(hash: RuntimeHash) -> SchemaId {
	SchemaId(domain_hash(hash))
}

fn attestation_id(hash: RuntimeHash) -> AttestationId {
	AttestationId(domain_hash(hash))
}

fn name_id(hash: RuntimeHash) -> NameId {
	NameId(domain_hash(hash))
}

fn subject_id(identifier: RuntimeSubjectId) -> DomainResult<SubjectId> {
	SubjectId::new(identifier.to_string_lossy())
}

fn content_id(hash: [u8; 32]) -> ContentCommitment {
	ContentCommitment(Hash32::from_bytes(hash))
}

fn agreement_id(hash: RuntimeHash) -> AgreementId {
	AgreementId(domain_hash(hash))
}

fn challenge_id(hash: RuntimeHash) -> ChallengeId {
	ChallengeId(domain_hash(hash))
}

fn container_id(hash: RuntimeHash) -> ContainerId {
	ContainerId(domain_hash(hash))
}

fn drive_id(hash: RuntimeHash) -> DriveId {
	DriveId(domain_hash(hash))
}

fn bucket_id(hash: RuntimeHash) -> BucketId {
	BucketId(domain_hash(hash))
}

fn object_id(hash: RuntimeHash) -> ObjectId {
	ObjectId(domain_hash(hash))
}

fn runtime_api_error(error: subxt::Error) -> NativeError {
	NativeError::new(NativeErrorCode::UnsupportedRuntime, error.to_string())
}
