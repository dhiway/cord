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

//! Exact finalized-hash, metadata-decoded Orbis runtime API reads.
//!
//! This binding never calls `at_latest` for data and never decodes raw SCALE. The requested hash
//! selects the runtime API context; Subxt validates method/argument shape from metadata and decodes
//! each response into a concrete `DecodeAsType` wire type.

use async_trait::async_trait;
use orbis_storage_runtime_api as storage_api;
use pallet_orbis_attestation_runtime_api as att_api;
use pallet_orbis_names_runtime_api as names_api;
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
			AccountId, AgreementId, AttestationId, BucketId, ChallengeId, ContentCommitment,
			DomainResult, DriveId, FinalizedPage, FinalizedValue, Hash32, NameId, ObjectId,
			ProofCommitment, SchemaId, SubjectId, UniquenessCommitment, Validate,
		},
		drive::{DriveName, DriveQuery, DriveRead, DriveResponse, DriveStatus, DriveView},
		names::{
			Address, ContentPublication as DomainContentPublication, Label,
			NameStatus as DomainNameStatus, NameView as DomainNameView, NamesQuery, NamesRead,
			NamesResponse, TextValue,
		},
		s3::{
			BucketName, BucketStatus, BucketView, FinalizedObjectPage, ObjectKey, ObjectKeyPrefix,
			ObjectListCursor, ObjectListRequest, ObjectVersionView, ObjectView, S3Query, S3Read,
			S3Response,
		},
		storage_provider::{
			AgreementStatus, AgreementView, ChallengeStatus, ChallengeView, CheckpointView,
			ChunkLocationView, CommitmentView, Endpoint, ProviderOrganizationView,
			ProviderServiceKeyView, ProviderStatus, ProviderView, ServiceKey, StorageProviderQuery,
			StorageProviderRead, StorageProviderResponse,
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
		Ok(StorageProviderResponse::Agreements(storage_finalized_page(
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
	use super::*;

	#[test]
	fn rejects_same_height_fork_and_wrong_height() {
		assert!(canonical_hash_at_height_matches([1u8; 32], 7, [1u8; 32], 7));
		assert!(!canonical_hash_at_height_matches([1u8; 32], 7, [2u8; 32], 7));
		assert!(!canonical_hash_at_height_matches([1u8; 32], 7, [1u8; 32], 8));
	}

	#[test]
	fn storage_versions_are_v8_without_relaxing_unrelated_v1_envelopes() {
		let hash = Hash32::from_bytes([1; 32]);
		assert!(finalized_value::<()>(&hash, 1, None).is_ok());
		assert!(finalized_value::<()>(&hash, storage_api::RESPONSE_VERSION, None).is_err());
		assert!(storage_finalized_value::<()>(&hash, storage_api::RESPONSE_VERSION, None).is_ok());
		assert!(storage_finalized_value::<()>(&hash, 1, None).is_err());
		assert!(storage_finalized_page::<()>(
			&hash,
			storage_api::RESPONSE_VERSION,
			vec![],
			Some(1)
		)
		.is_err());
	}

	#[test]
	fn object_key_arguments_match_the_v3_runtime_api_shape() {
		let bucket = BucketId::new(format!("0x{}", "11".repeat(32))).unwrap();
		let page = ObjectListRequest {
			prefix: Some(ObjectKeyPrefix::new(b"images/".to_vec()).unwrap()),
			cursor: Some(ObjectListCursor {
				snapshot_version: 7,
				last_key: ObjectKey::new(b"images/a.png".to_vec()).unwrap(),
			}),
			limit: 25,
		};
		let expected = vec![
			Value::from_bytes([0x11; 32]),
			Value::variant("Some", Composite::unnamed(vec![Value::from_bytes(b"images/")])),
			Value::variant(
				"Some",
				Composite::unnamed(vec![Value::named_composite(vec![
					("snapshot_version", Value::u128(7)),
					("last_key", Value::from_bytes(b"images/a.png")),
				])]),
			),
			Value::u128(25),
		];
		assert_eq!(object_keys_args(&bucket, &page).unwrap(), expected);
		assert!(ObjectKeyPrefix::new(Vec::new()).is_ok());
		assert!(ObjectKeyPrefix::new(vec![0; 1_025]).is_err());
		assert!(object_keys_args(
			&bucket,
			&ObjectListRequest { prefix: None, cursor: None, limit: 0 }
		)
		.is_err());
	}

	#[test]
	fn snapshot_object_pages_decode_and_reject_inconsistent_cursors() {
		let hash = Hash32::from_bytes([1; 32]);
		let request = ObjectListRequest {
			prefix: None,
			cursor: Some(ObjectListCursor {
				snapshot_version: 7,
				last_key: ObjectKey::new(b"a".to_vec()).unwrap(),
			}),
			limit: 2,
		};
		let response = storage_api::SnapshotPage {
			version: storage_api::RESPONSE_VERSION,
			items: vec![b"b".to_vec()],
			next_cursor: Some(storage_api::SnapshotCursor {
				snapshot_version: 7,
				last_key: b"b".to_vec(),
			}),
			snapshot_version: 7,
		};
		let page = finalized_object_page(&hash, &request, response.clone()).unwrap();
		assert_eq!(page.snapshot_version, 7);
		assert_eq!(page.items[0].as_bytes(), b"b");
		assert_eq!(page.next_cursor.unwrap().last_key.as_bytes(), b"b");

		let mut stale = response.clone();
		stale.snapshot_version = 8;
		assert_eq!(
			finalized_object_page(&hash, &request, stale).unwrap_err().code,
			NativeErrorCode::InconsistentSnapshot
		);
		let mut wrong_next = response;
		wrong_next.next_cursor.as_mut().unwrap().last_key = b"c".to_vec();
		assert_eq!(
			finalized_object_page(&hash, &request, wrong_next).unwrap_err().code,
			NativeErrorCode::InconsistentSnapshot
		);
	}

	#[test]
	fn s3_list_errors_and_sparse_challenge_cursors_are_deterministic() {
		for (error, code) in [
			(storage_api::S3ListError::BucketNotFound, NativeErrorCode::NotFound),
			(storage_api::S3ListError::BucketDeleted, NativeErrorCode::Conflict),
			(storage_api::S3ListError::CursorStale, NativeErrorCode::InconsistentSnapshot),
			(storage_api::S3ListError::PageLimitInvalid, NativeErrorCode::InvalidInput),
			(storage_api::S3ListError::CursorKeyInvalid, NativeErrorCode::InvalidInput),
		] {
			assert_eq!(s3_list_error(error).code, code);
		}

		let hash = Hash32::from_bytes([1; 32]);
		let page = storage_sparse_finalized_page::<()>(
			&hash,
			storage_api::RESPONSE_VERSION,
			vec![],
			Some(4),
			Some(3),
		)
		.unwrap();
		assert_eq!(page.next_cursor, Some(4));
		assert_eq!(
			storage_sparse_finalized_page::<()>(
				&hash,
				storage_api::RESPONSE_VERSION,
				vec![],
				Some(3),
				Some(3),
			)
			.unwrap_err()
			.code,
			NativeErrorCode::InconsistentSnapshot
		);
	}

	#[test]
	fn current_storage_v8_shapes_map_without_legacy_projection() {
		let provider_account = RuntimeAccountId::from([1; 32]);
		let provider = account_id(&provider_account).unwrap();
		let provider_view = provider_view(
			provider.clone(),
			storage_api::ProviderInfo {
				endpoint: b"https://provider.example".to_vec(),
				organization: storage_api::OrganizationInfo {
					entity_id: b"did:cord:provider".to_vec(),
					attestation_id: RuntimeHash::repeat_byte(2),
					schema_id: RuntimeHash::repeat_byte(3),
					sla_commitment: RuntimeHash::repeat_byte(4),
					sla_version: 5,
					valid_from: 6,
					valid_until: 7,
					rotation_predecessor: Some(RuntimeHash::repeat_byte(8)),
				},
				service_key: storage_api::ServiceKeyInfo {
					active: [9; 32],
					active_version: 10,
					previous: Some([11; 32]),
					pending: Some([12; 32]),
					pending_version: Some(13),
					pending_effective_at: Some(14),
				},
				capacity_bytes: 15,
				allocated_bytes: 16,
				pending_bytes: 17,
				status: storage_api::ProviderStatus::Suspended,
				last_heartbeat: 18,
				overdue_challenges: 19,
				authority_validated_at: Some(20),
			},
		)
		.unwrap();
		assert_eq!(provider_view.provider, provider);
		assert_eq!(provider_view.organization.sla_version, 5);
		assert_eq!(provider_view.service_key.active.as_bytes(), &[9; 32]);
		assert_eq!(provider_view.overdue_challenges, 19);

		let agreement = agreement_view(storage_api::AgreementInfo {
			agreement_id: RuntimeHash::repeat_byte(21),
			owner: RuntimeAccountId::from([22; 32]),
			bucket_id: RuntimeHash::repeat_byte(23),
			primary: RuntimeAccountId::from([24; 32]),
			replicas: vec![RuntimeAccountId::from([25; 32])],
			bytes: 26,
			created_at: 27,
			expires_at: 28,
			release_at: Some(29),
			state_version: 30,
			status: storage_api::AgreementStatus::Suspended,
		})
		.unwrap();
		assert_eq!(agreement.status, AgreementStatus::Suspended);
		assert_eq!(agreement.replicas.len(), 1);
		assert_eq!(agreement.state_version, 30);

		let commitment = storage_api::CommitmentInfo {
			mmr_root: RuntimeHash::repeat_byte(31),
			start_seq: 32,
			leaf_count: 33,
		};
		let challenge = challenge_view(storage_api::ChallengeInfo {
			challenge_id: RuntimeHash::repeat_byte(34),
			bucket_id: RuntimeHash::repeat_byte(35),
			provider: RuntimeAccountId::from([36; 32]),
			expected_commitment: commitment,
			location: storage_api::ChunkLocationInfo { leaf_index: 37, chunk_index: 38 },
			due_at: 39,
			status: storage_api::ChallengeStatus::Open,
		})
		.unwrap();
		assert_eq!(challenge.expected_commitment.start_seq, 32);
		assert_eq!(challenge.location.chunk_index, 38);

		let checkpoint = checkpoint_view(storage_api::CheckpointInfo {
			bucket_id: RuntimeHash::repeat_byte(40),
			commitment,
			checkpoint_block: 41,
			primary_signers: 1,
			commitment_nonce: 42,
			replica_confirmations: vec![RuntimeAccountId::from([43; 32])],
		})
		.unwrap();
		assert_eq!(checkpoint.commitment.leaf_count, 33);
		assert_eq!(checkpoint.replica_confirmations.len(), 1);

		let drive = drive_view(storage_api::DriveInfo {
			drive_id: RuntimeHash::repeat_byte(44),
			owner: RuntimeAccountId::from([45; 32]),
			name: b"archive".to_vec(),
			root_manifest: Some([46; 32]),
			root_provider_commitment: Some([47; 32]),
			version: 48,
			status: storage_api::ContainerStatus::Deleted,
			created_at: 49,
			updated_at: 50,
			controllers: vec![RuntimeAccountId::from([51; 32])],
		})
		.unwrap();
		assert_eq!(drive.status, DriveStatus::Deleted);
		assert!(drive.root_manifest.is_some());
		assert_eq!(drive.controllers.len(), 1);

		let object = object_view(storage_api::ObjectInfo {
			object_id: RuntimeHash::repeat_byte(52),
			bucket_id: RuntimeHash::repeat_byte(53),
			key: b"object".to_vec(),
			content_hash: Some([54; 32]),
			provider_commitment: Some([55; 32]),
			version: 56,
			deleted: false,
			updated_by: RuntimeAccountId::from([57; 32]),
			updated_at: 58,
		})
		.unwrap();
		assert_eq!(
			object.provider_commitment,
			Some(ContentCommitment(Hash32::from_bytes([55; 32])))
		);
		let version = object_version_view(storage_api::ObjectVersionInfo {
			content_hash: Some([59; 32]),
			provider_commitment: Some([60; 32]),
			version: 61,
			deleted: false,
			updated_by: RuntimeAccountId::from([62; 32]),
			updated_at: 63,
		})
		.unwrap();
		assert_eq!(
			version.provider_commitment,
			Some(ContentCommitment(Hash32::from_bytes([60; 32])))
		);
	}
}


#[async_trait]
impl FinalizedReadBinding for OrbisFinalizedReadBinding {
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

	async fn names(&self, read: &NamesRead) -> DomainResult<NamesResponse> {
		read.validate()?;
		let hash = &read.finalized_block_hash;
		match &read.query {
			NamesQuery::LabelPolicyVersion => {
				let response: u16 =
					self.call_at(hash, "NamesApi", "label_policy_version", Vec::new()).await?;
				Ok(NamesResponse::LabelPolicyVersion(finalized_value(
					hash,
					names_api::RESPONSE_VERSION,
					Some(response),
				)?))
			},
			NamesQuery::NameById { name } => {
				type Wire = names_api::Versioned<
					names_api::ClientNameView<RuntimeAccountId, RuntimeBlockNumber, RuntimeHash>,
				>;
				let response: Wire = self
					.call_at(hash, "NamesApi", "name_by_id", vec![hash_arg(name.as_hash())?])
					.await?;
				Ok(NamesResponse::Name(finalized_value(
					hash,
					response.version,
					response.value.map(name_view).transpose()?,
				)?))
			},
			NamesQuery::RootByLabel { label } => {
				let response: names_api::Versioned<RuntimeHash> = self
					.call_at(
						hash,
						"NamesApi",
						"root_name_by_normalized_label",
						vec![Value::from_bytes(label.as_str().as_bytes())],
					)
					.await?;
				Ok(NamesResponse::NameId(finalized_value(
					hash,
					response.version,
					response.value.map(name_id),
				)?))
			},
			NamesQuery::OwnerNames { owner, page } => {
				let response: names_api::ClientOwnerNamesPage<RuntimeHash> = self
					.call_at(hash, "NamesApi", "owner_names", page_args(account_arg(owner)?, page))
					.await?;
				Ok(NamesResponse::Names(finalized_page(
					hash,
					response.version,
					response.names.into_iter().map(name_id).collect(),
					response.next_cursor,
				)?))
			},
			NamesQuery::Controllers { name } => {
				let response: names_api::Versioned<Vec<RuntimeAccountId>> = self
					.call_at(hash, "NamesApi", "controllers", vec![hash_arg(name.as_hash())?])
					.await?;
				Ok(NamesResponse::Controllers(finalized_value(
					hash,
					response.version,
					response
						.value
						.map(|items| items.iter().map(account_id).collect())
						.transpose()?,
				)?))
			},
			NamesQuery::ResolveAddress { name } => {
				let response: names_api::Versioned<Vec<u8>> = self
					.call_at(hash, "NamesApi", "resolve_address", vec![hash_arg(name.as_hash())?])
					.await?;
				Ok(NamesResponse::Address(finalized_value(
					hash,
					response.version,
					response.value.map(Address::new).transpose()?,
				)?))
			},
			NamesQuery::ResolveSubject { name } => {
				let response: names_api::Versioned<RuntimeSubjectId> = self
					.call_at(hash, "NamesApi", "resolve_subject", vec![hash_arg(name.as_hash())?])
					.await?;
				Ok(NamesResponse::Subject(finalized_value(
					hash,
					response.version,
					response.value.map(subject_id).transpose()?,
				)?))
			},
			NamesQuery::ResolveAttestation { name } => {
				let response: names_api::Versioned<RuntimeHash> = self
					.call_at(
						hash,
						"NamesApi",
						"resolve_attestation",
						vec![hash_arg(name.as_hash())?],
					)
					.await?;
				Ok(NamesResponse::Attestation(finalized_value(
					hash,
					response.version,
					response.value.map(attestation_id),
				)?))
			},
			NamesQuery::ResolveContentPublication { name } => {
				let response: names_api::Versioned<names_api::ContentPublication<[u8; 32]>> = self
					.call_at(
						hash,
						"NamesApi",
						"resolve_content_publication",
						vec![hash_arg(name.as_hash())?],
					)
					.await?;
				Ok(NamesResponse::ContentPublication(finalized_value(
					hash,
					response.version,
					response.value.map(|publication| DomainContentPublication {
						content: publication.content.map(content_id),
						revision: publication.revision,
					}),
				)?))
			},
			NamesQuery::ResolveText { name, key } => {
				let response: names_api::Versioned<Vec<u8>> = self
					.call_at(
						hash,
						"NamesApi",
						"resolve_text",
						vec![hash_arg(name.as_hash())?, Value::from_bytes(key.as_bytes())],
					)
					.await?;
				Ok(NamesResponse::Text(finalized_value(
					hash,
					response.version,
					response.value.map(TextValue::new).transpose()?,
				)?))
			},
			NamesQuery::PrimaryName { owner } => {
				let response: names_api::Versioned<RuntimeHash> = self
					.call_at(hash, "NamesApi", "primary_name", vec![account_arg(owner)?])
					.await?;
				Ok(NamesResponse::NameId(finalized_value(
					hash,
					response.version,
					response.value.map(name_id),
				)?))
			},
			NamesQuery::NameStatus { name } => {
				let response: names_api::NameStatus<RuntimeBlockNumber> = self
					.call_at(hash, "NamesApi", "name_status", vec![hash_arg(name.as_hash())?])
					.await?;
				let version = response.version;
				let value = DomainNameStatus {
					exists: response.exists,
					active: response.active,
					expires_at: response.expires_at,
				};
				Ok(NamesResponse::Status(finalized_value(hash, version, Some(value))?))
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
					storage_api::ProviderInfo<RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(hash, "StorageProviderApi", "provider", vec![account_arg(provider)?])
					.await?;
				let value =
					response.value.map(|info| provider_view(provider.clone(), info)).transpose()?;
				Ok(StorageProviderResponse::Provider(storage_finalized_value(
					hash,
					response.version,
					value,
				)?))
			},
			StorageProviderQuery::Providers { page } => {
				let response: storage_api::Page<(
					RuntimeAccountId,
					storage_api::ProviderInfo<RuntimeHash, RuntimeBlockNumber>,
				)> = self
					.call_at(hash, "StorageProviderApi", "providers", page_tail(page).into())
					.await?;
				let items = response
					.items
					.iter()
					.map(|(account, _)| account_id(account))
					.collect::<DomainResult<Vec<_>>>()?;
				Ok(StorageProviderResponse::Providers(storage_finalized_page(
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
				Ok(StorageProviderResponse::Agreement(storage_finalized_value(
					hash,
					response.version,
					response.value.map(agreement_view).transpose()?,
				)?))
			},
			StorageProviderQuery::ProviderAgreements { provider, page } => {
				self.agreement_page(hash, "provider_agreements", account_arg(provider)?, page)
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
				Ok(StorageProviderResponse::AgreementNonce(storage_finalized_value(
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
				Ok(StorageProviderResponse::Challenge(storage_finalized_value(
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
				Ok(StorageProviderResponse::Challenges(storage_sparse_finalized_page(
					hash,
					response.version,
					response
						.items
						.into_iter()
						.map(|item| challenge_id(item.challenge_id))
						.collect(),
					response.next_cursor,
					page.cursor,
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
				Ok(StorageProviderResponse::CanAcceptCapacity(storage_finalized_value(
					hash,
					storage_api::RESPONSE_VERSION,
					Some(accepted),
				)?))
			},
			StorageProviderQuery::BucketCheckpoint { bucket } => {
				let response: storage_api::Versioned<
					storage_api::CheckpointInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(
						hash,
						"StorageProviderApi",
						"checkpoint",
						vec![hash_arg(bucket.as_hash())?],
					)
					.await?;
				Ok(StorageProviderResponse::Checkpoint(storage_finalized_value(
					hash,
					response.version,
					response.value.map(checkpoint_view).transpose()?,
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
				Ok(DriveResponse::Drive(storage_finalized_value(
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
				Ok(DriveResponse::Drives(storage_finalized_page(
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
				Ok(DriveResponse::Controllers(storage_finalized_page(
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
				Ok(DriveResponse::NextDriveNonce(storage_finalized_value(
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
				Ok(DriveResponse::IsDriveOwner(storage_finalized_value(
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
				Ok(S3Response::Bucket(storage_finalized_value(
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
				Ok(S3Response::BucketId(storage_finalized_value(hash, response.version, id)?))
			},
			S3Query::OwnerBuckets { owner, page } => {
				let response: storage_api::Page<
					storage_api::BucketInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
				> = self
					.call_at(hash, "S3RegistryApi", "buckets", page_args(account_arg(owner)?, page))
					.await?;
				Ok(S3Response::Buckets(storage_finalized_page(
					hash,
					response.version,
					response.items.into_iter().map(|item| bucket_id(item.bucket_id)).collect(),
					response.next_cursor,
				)?))
			},
			S3Query::BucketObjects { bucket, page } => {
				let response: Result<storage_api::SnapshotPage<Vec<u8>>, storage_api::S3ListError> =
					self.call_at(
						hash,
						"S3RegistryApi",
						"object_keys",
						object_keys_args(bucket, page)?,
					)
					.await?;
				Ok(S3Response::Objects(finalized_object_page(
					hash,
					page,
					response.map_err(s3_list_error)?,
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
				Ok(S3Response::Object(storage_finalized_value(
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
				Ok(S3Response::ObjectHistory(storage_finalized_page(
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
				Ok(S3Response::ObjectId(storage_finalized_value(
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
				Ok(S3Response::IsBucketOwner(storage_finalized_value(
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
	view: names_api::ClientNameView<RuntimeAccountId, RuntimeBlockNumber, RuntimeHash>,
) -> DomainResult<DomainNameView> {
	let label = String::from_utf8(view.label).map_err(|_| {
		NativeError::new(NativeErrorCode::UnsupportedRuntime, "Orbis Names label is not UTF-8")
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
fn provider_view(
	provider: AccountId,
	info: storage_api::ProviderInfo<RuntimeHash, RuntimeBlockNumber>,
) -> DomainResult<ProviderView> {
	Ok(ProviderView {
		provider,
		endpoint: Endpoint::new(info.endpoint)?,
		organization: ProviderOrganizationView {
			entity_id: info.organization.entity_id,
			attestation: domain_hash(info.organization.attestation_id),
			schema: domain_hash(info.organization.schema_id),
			sla_commitment: domain_hash(info.organization.sla_commitment),
			sla_version: info.organization.sla_version,
			valid_from: info.organization.valid_from,
			valid_until: info.organization.valid_until,
			rotation_predecessor: info.organization.rotation_predecessor.map(domain_hash),
		},
		service_key: ProviderServiceKeyView {
			active: ServiceKey::new(info.service_key.active.to_vec())?,
			active_version: info.service_key.active_version,
			previous: info
				.service_key
				.previous
				.map(|key| ServiceKey::new(key.to_vec()))
				.transpose()?,
			pending: info
				.service_key
				.pending
				.map(|key| ServiceKey::new(key.to_vec()))
				.transpose()?,
			pending_version: info.service_key.pending_version,
			pending_effective_at: info.service_key.pending_effective_at,
		},
		capacity_bytes: info.capacity_bytes,
		allocated_bytes: info.allocated_bytes,
		pending_bytes: info.pending_bytes,
		status: match info.status {
			storage_api::ProviderStatus::Active => ProviderStatus::Active,
			storage_api::ProviderStatus::Suspended => ProviderStatus::Suspended,
		},
		last_heartbeat: info.last_heartbeat,
		overdue_challenges: info.overdue_challenges,
		authority_validated_at: info.authority_validated_at,
	})
}

fn agreement_view(
	info: storage_api::AgreementInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
) -> DomainResult<AgreementView> {
	Ok(AgreementView {
		agreement: agreement_id(info.agreement_id),
		owner: account_id(&info.owner)?,
		bucket: bucket_id(info.bucket_id),
		primary: account_id(&info.primary)?,
		replicas: info.replicas.iter().map(account_id).collect::<DomainResult<Vec<_>>>()?,
		bytes: info.bytes,
		created_at: info.created_at,
		expires_at: info.expires_at,
		release_at: info.release_at,
		state_version: info.state_version,
		status: match info.status {
			storage_api::AgreementStatus::Proposed => AgreementStatus::Proposed,
			storage_api::AgreementStatus::Active => AgreementStatus::Active,
			storage_api::AgreementStatus::Suspended => AgreementStatus::Suspended,
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
		bucket: bucket_id(info.bucket_id),
		provider: account_id(&info.provider)?,
		expected_commitment: commitment_view(info.expected_commitment),
		location: ChunkLocationView {
			leaf_index: info.location.leaf_index,
			chunk_index: info.location.chunk_index,
		},
		due_at: info.due_at,
		status: match info.status {
			storage_api::ChallengeStatus::Open => ChallengeStatus::Open,
			storage_api::ChallengeStatus::Proved => ChallengeStatus::Proved,
			storage_api::ChallengeStatus::TimedOut => ChallengeStatus::TimedOut,
		},
	})
}

fn checkpoint_view(
	info: storage_api::CheckpointInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
) -> DomainResult<CheckpointView> {
	Ok(CheckpointView {
		bucket: bucket_id(info.bucket_id),
		commitment: commitment_view(info.commitment),
		checkpoint_block: info.checkpoint_block,
		primary_signers: info.primary_signers,
		commitment_nonce: info.commitment_nonce,
		replica_confirmations: info
			.replica_confirmations
			.iter()
			.map(account_id)
			.collect::<DomainResult<Vec<_>>>()?,
	})
}

fn commitment_view(info: storage_api::CommitmentInfo<RuntimeHash>) -> CommitmentView {
	CommitmentView {
		mmr_root: ProofCommitment(domain_hash(info.mmr_root)),
		start_seq: info.start_seq,
		leaf_count: info.leaf_count,
	}
}

fn drive_view(
	info: storage_api::DriveInfo<RuntimeAccountId, RuntimeHash, RuntimeBlockNumber>,
) -> DomainResult<DriveView> {
	let status = match info.status {
		storage_api::ContainerStatus::Active => DriveStatus::Active,
		storage_api::ContainerStatus::Archived => DriveStatus::Archived,
		storage_api::ContainerStatus::Deleted => DriveStatus::Deleted,
	};
	Ok(DriveView {
		drive: drive_id(info.drive_id),
		owner: account_id(&info.owner)?,
		name: DriveName::new(info.name)?,
		root_manifest: info.root_manifest.map(content_id),
		root_provider_commitment: info.root_provider_commitment.map(content_id),
		version: info.version,
		status,
		created_at: info.created_at,
		updated_at: info.updated_at,
		controllers: info.controllers.iter().map(account_id).collect::<DomainResult<Vec<_>>>()?,
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
		provider_commitment: info
			.provider_commitment
			.map(|hash| ContentCommitment(Hash32::from_bytes(hash))),
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
		provider_commitment: info
			.provider_commitment
			.map(|hash| ContentCommitment(Hash32::from_bytes(hash))),
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

fn storage_finalized_value<T>(
	hash: &Hash32,
	version: u16,
	value: Option<T>,
) -> DomainResult<FinalizedValue<T>> {
	ensure_storage_response_version(version)?;
	hash.validate()?;
	Ok(FinalizedValue { version, finalized_block_hash: hash.clone(), value })
}

fn storage_finalized_page<T>(
	hash: &Hash32,
	version: u16,
	items: Vec<T>,
	next_cursor: Option<u32>,
) -> DomainResult<FinalizedPage<T>> {
	ensure_storage_response_version(version)?;
	hash.validate()?;
	if items.len() > storage_api::MAX_PAGE_SIZE as usize {
		return Err(NativeError::new(
			NativeErrorCode::UnsupportedRuntime,
			"storage runtime response exceeded its page contract",
		));
	}
	if items.is_empty() && next_cursor.is_some() {
		return Err(NativeError::new(
			NativeErrorCode::UnsupportedRuntime,
			"empty storage runtime response advanced its cursor",
		));
	}
	Ok(FinalizedPage { version, finalized_block_hash: hash.clone(), items, next_cursor })
}

fn storage_sparse_finalized_page<T>(
	hash: &Hash32,
	version: u16,
	items: Vec<T>,
	next_cursor: Option<u32>,
	request_cursor: Option<u32>,
) -> DomainResult<FinalizedPage<T>> {
	ensure_storage_response_version(version)?;
	hash.validate()?;
	if items.len() > storage_api::MAX_PAGE_SIZE as usize {
		return Err(NativeError::new(
			NativeErrorCode::UnsupportedRuntime,
			"storage runtime response exceeded its page contract",
		));
	}
	if next_cursor.is_some_and(|next| next <= request_cursor.unwrap_or(0)) {
		return Err(NativeError::new(
			NativeErrorCode::InconsistentSnapshot,
			"sparse storage runtime response did not advance its cursor",
		));
	}
	Ok(FinalizedPage { version, finalized_block_hash: hash.clone(), items, next_cursor })
}

fn finalized_object_page(
	hash: &Hash32,
	request: &ObjectListRequest,
	response: storage_api::SnapshotPage<Vec<u8>>,
) -> DomainResult<FinalizedObjectPage> {
	ensure_storage_response_version(response.version)?;
	hash.validate()?;
	if response.items.len() > storage_api::MAX_PAGE_SIZE as usize {
		return Err(NativeError::new(
			NativeErrorCode::UnsupportedRuntime,
			"S3 object response exceeded its page contract",
		));
	}
	if request
		.cursor
		.as_ref()
		.is_some_and(|cursor| cursor.snapshot_version != response.snapshot_version)
	{
		return Err(NativeError::new(
			NativeErrorCode::InconsistentSnapshot,
			"S3 object response changed snapshot version",
		));
	}

	let items = response
		.items
		.into_iter()
		.map(ObjectKey::new)
		.collect::<DomainResult<Vec<_>>>()?;
	let next_cursor = response
		.next_cursor
		.map(|cursor| {
			Ok(ObjectListCursor {
				snapshot_version: cursor.snapshot_version,
				last_key: ObjectKey::new(cursor.last_key)?,
			})
		})
		.transpose()?;
	if let Some(next) = next_cursor.as_ref() {
		if next.snapshot_version != response.snapshot_version {
			return Err(NativeError::new(
				NativeErrorCode::InconsistentSnapshot,
				"S3 object cursor changed snapshot version",
			));
		}
		if items.last() != Some(&next.last_key) {
			return Err(NativeError::new(
				NativeErrorCode::InconsistentSnapshot,
				"S3 object cursor does not identify the last returned key",
			));
		}
		if request
			.cursor
			.as_ref()
			.is_some_and(|current| next.last_key.as_bytes() <= current.last_key.as_bytes())
		{
			return Err(NativeError::new(
				NativeErrorCode::InconsistentSnapshot,
				"S3 object cursor did not advance",
			));
		}
	}

	Ok(FinalizedObjectPage {
		version: response.version,
		finalized_block_hash: hash.clone(),
		items,
		next_cursor,
		snapshot_version: response.snapshot_version,
	})
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

fn ensure_storage_response_version(version: u16) -> DomainResult<()> {
	if version != storage_api::RESPONSE_VERSION {
		return Err(NativeError::new(
			NativeErrorCode::UnsupportedRuntime,
			format!("unsupported Orbis storage runtime API response version {version}"),
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
fn page_args(first: Value, page: &super::domains::PageRequest) -> Vec<Value> {
	let mut args = vec![first];
	args.extend(page_tail(page));
	args
}

fn page_tail(page: &super::domains::PageRequest) -> [Value; 2] {
	[option_u32(page.cursor), Value::u128(page.limit as u128)]
}

fn object_keys_args(bucket: &BucketId, page: &ObjectListRequest) -> DomainResult<Vec<Value>> {
	page.validate()?;
	Ok(vec![
		hash_arg(bucket.as_hash())?,
		option_bytes(page.prefix.as_ref().map(ObjectKeyPrefix::as_bytes)),
		option_object_cursor(page.cursor.as_ref()),
		Value::u128(page.limit as u128),
	])
}

fn option_bytes(value: Option<&[u8]>) -> Value {
	match value {
		Some(value) => Value::variant("Some", Composite::unnamed(vec![Value::from_bytes(value)])),
		None => Value::variant("None", Composite::unnamed(Vec::new())),
	}
}

fn option_object_cursor(cursor: Option<&ObjectListCursor>) -> Value {
	match cursor {
		Some(cursor) => Value::variant(
			"Some",
			Composite::unnamed(vec![Value::named_composite(vec![
				("snapshot_version", Value::u128(cursor.snapshot_version as u128)),
				("last_key", Value::from_bytes(cursor.last_key.as_bytes())),
			])]),
		),
		None => Value::variant("None", Composite::unnamed(Vec::new())),
	}
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

fn s3_list_error(error: storage_api::S3ListError) -> NativeError {
	let (code, message) = match error {
		storage_api::S3ListError::BucketNotFound => {
			(NativeErrorCode::NotFound, "S3 bucket was not found")
		},
		storage_api::S3ListError::BucketDeleted => {
			(NativeErrorCode::Conflict, "S3 bucket is deleted")
		},
		storage_api::S3ListError::CursorStale => {
			(NativeErrorCode::InconsistentSnapshot, "S3 object cursor is stale")
		},
		storage_api::S3ListError::PageLimitInvalid => {
			(NativeErrorCode::InvalidInput, "S3 object page limit is invalid")
		},
		storage_api::S3ListError::CursorKeyInvalid => {
			(NativeErrorCode::InvalidInput, "S3 object cursor key is invalid")
		},
	};
	NativeError::new(code, message)
}
