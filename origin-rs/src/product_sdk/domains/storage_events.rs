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

//! Typed finalized lifecycle events for the native storage/provider/Drive/S3 stack.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::{
	common::{invalid, AccountId, BlockNumber, DomainResult, Hash32, Validate},
	storage_provider::ProviderStatus,
	AgreementId, BucketId, ChallengeId, ContentCommitment, DriveId, ObjectId, ProofCommitment,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageNativeEventKind {
	ProviderRegistered,
	ProviderUpdated,
	ProviderStatusChanged,
	ProviderRemoved,
	Heartbeat,
	AgreementProposed,
	AgreementAccepted,
	AgreementCancelled,
	AgreementRenewalRequested,
	AgreementRenewed,
	AgreementExpired,
	AgreementPruned,
	ChallengeIssued,
	CheckpointSubmitted,
	ChallengeTimedOut,
	ProviderRootCommitted,
	DeletionAcknowledged,
	DriveCreated,
	DriveRootUpdated,
	DriveControllerChanged,
	DriveTransferred,
	DriveArchived,
	BucketCreated,
	BucketControllerChanged,
	BucketTransferred,
	BucketArchived,
	BucketVersioningChanged,
	ObjectPut,
	ObjectDeleted,
	BucketDeleted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum StorageNativeEvent {
	ProviderRegistered {
		provider: AccountId,
		capacity_bytes: u64,
	},
	ProviderUpdated {
		provider: AccountId,
		capacity_bytes: u64,
	},
	ProviderStatusChanged {
		provider: AccountId,
		status: ProviderStatus,
	},
	ProviderRemoved {
		provider: AccountId,
	},
	Heartbeat {
		provider: AccountId,
		at: BlockNumber,
	},
	AgreementProposed {
		agreement: AgreementId,
		owner: AccountId,
		provider: AccountId,
	},
	AgreementAccepted {
		agreement: AgreementId,
	},
	AgreementCancelled {
		agreement: AgreementId,
	},
	AgreementRenewalRequested {
		agreement: AgreementId,
		expires_at: BlockNumber,
	},
	AgreementRenewed {
		agreement: AgreementId,
		expires_at: BlockNumber,
	},
	AgreementExpired {
		agreement: AgreementId,
	},
	AgreementPruned {
		agreement: AgreementId,
	},
	ChallengeIssued {
		challenge: ChallengeId,
		provider: AccountId,
		due_at: BlockNumber,
	},
	CheckpointSubmitted {
		challenge: ChallengeId,
		proof_commitment: ProofCommitment,
	},
	ChallengeTimedOut {
		challenge: ChallengeId,
		provider: AccountId,
	},
	ProviderRootCommitted {
		provider: AccountId,
		sequence: u64,
		root: ProofCommitment,
		leaf_count: u64,
	},
	DeletionAcknowledged {
		agreement: AgreementId,
		provider: AccountId,
		content_commitment: ContentCommitment,
		tombstone_root: ProofCommitment,
		root_sequence: u64,
		leaf_index: u64,
		leaf_count: u64,
		proof_commitment: ProofCommitment,
	},
	DriveCreated {
		drive: DriveId,
		owner: AccountId,
	},
	DriveRootUpdated {
		drive: DriveId,
		version: u64,
	},
	DriveControllerChanged {
		drive: DriveId,
		controller: AccountId,
		enabled: bool,
	},
	DriveTransferred {
		drive: DriveId,
		old_owner: AccountId,
		new_owner: AccountId,
	},
	DriveArchived {
		drive: DriveId,
	},
	BucketCreated {
		bucket: BucketId,
		name: Vec<u8>,
		owner: AccountId,
	},
	BucketControllerChanged {
		bucket: BucketId,
		controller: AccountId,
		enabled: bool,
		version: u64,
	},
	BucketTransferred {
		bucket: BucketId,
		from: AccountId,
		to: AccountId,
		version: u64,
	},
	BucketArchived {
		bucket: BucketId,
		archived: bool,
		version: u64,
	},
	BucketVersioningChanged {
		bucket: BucketId,
		enabled: bool,
		version: u64,
	},
	ObjectPut {
		bucket: BucketId,
		object: ObjectId,
		key: Vec<u8>,
		content_commitment: ContentCommitment,
		version: u64,
	},
	ObjectDeleted {
		bucket: BucketId,
		object: ObjectId,
		key: Vec<u8>,
		version: u64,
	},
	BucketDeleted {
		bucket: BucketId,
		name: Vec<u8>,
		owner: AccountId,
	},
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageLifecycleAction {
	Registered,
	Updated,
	StatusChanged,
	Removed,
	Heartbeat,
	Proposed,
	Accepted,
	Cancelled,
	RenewalRequested,
	Renewed,
	Expired,
	Pruned,
	Issued,
	CheckpointSubmitted,
	TimedOut,
	RootCommitted,
	DeletionAcknowledged,
	Created,
	RootUpdated,
	ControllerChanged,
	Transferred,
	Archived,
	VersioningChanged,
	Put,
	Deleted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "outcome", content = "data", rename_all = "snake_case")]
pub enum StorageNativeOutcome {
	Provider { provider: AccountId, action: StorageLifecycleAction },
	Agreement { agreement: AgreementId, action: StorageLifecycleAction },
	Challenge { challenge: ChallengeId, action: StorageLifecycleAction },
	Drive { drive: DriveId, action: StorageLifecycleAction, version: Option<u64> },
	Bucket { bucket: BucketId, action: StorageLifecycleAction, version: Option<u64> },
	Object { bucket: BucketId, object: ObjectId, action: StorageLifecycleAction, version: u64 },
}

impl StorageNativeEvent {
	pub const fn kind(&self) -> StorageNativeEventKind {
		use StorageNativeEventKind as K;
		match self {
			Self::ProviderRegistered { .. } => K::ProviderRegistered,
			Self::ProviderUpdated { .. } => K::ProviderUpdated,
			Self::ProviderStatusChanged { .. } => K::ProviderStatusChanged,
			Self::ProviderRemoved { .. } => K::ProviderRemoved,
			Self::Heartbeat { .. } => K::Heartbeat,
			Self::AgreementProposed { .. } => K::AgreementProposed,
			Self::AgreementAccepted { .. } => K::AgreementAccepted,
			Self::AgreementCancelled { .. } => K::AgreementCancelled,
			Self::AgreementRenewalRequested { .. } => K::AgreementRenewalRequested,
			Self::AgreementRenewed { .. } => K::AgreementRenewed,
			Self::AgreementExpired { .. } => K::AgreementExpired,
			Self::AgreementPruned { .. } => K::AgreementPruned,
			Self::ChallengeIssued { .. } => K::ChallengeIssued,
			Self::CheckpointSubmitted { .. } => K::CheckpointSubmitted,
			Self::ChallengeTimedOut { .. } => K::ChallengeTimedOut,
			Self::ProviderRootCommitted { .. } => K::ProviderRootCommitted,
			Self::DeletionAcknowledged { .. } => K::DeletionAcknowledged,
			Self::DriveCreated { .. } => K::DriveCreated,
			Self::DriveRootUpdated { .. } => K::DriveRootUpdated,
			Self::DriveControllerChanged { .. } => K::DriveControllerChanged,
			Self::DriveTransferred { .. } => K::DriveTransferred,
			Self::DriveArchived { .. } => K::DriveArchived,
			Self::BucketCreated { .. } => K::BucketCreated,
			Self::BucketControllerChanged { .. } => K::BucketControllerChanged,
			Self::BucketTransferred { .. } => K::BucketTransferred,
			Self::BucketArchived { .. } => K::BucketArchived,
			Self::BucketVersioningChanged { .. } => K::BucketVersioningChanged,
			Self::ObjectPut { .. } => K::ObjectPut,
			Self::ObjectDeleted { .. } => K::ObjectDeleted,
			Self::BucketDeleted { .. } => K::BucketDeleted,
		}
	}

	pub fn outcome(&self) -> StorageNativeOutcome {
		use StorageLifecycleAction as A;
		match self {
			Self::ProviderRegistered { provider, .. } => provider_out(provider, A::Registered),
			Self::ProviderUpdated { provider, .. } => provider_out(provider, A::Updated),
			Self::ProviderStatusChanged { provider, .. } => {
				provider_out(provider, A::StatusChanged)
			},
			Self::ProviderRemoved { provider } => provider_out(provider, A::Removed),
			Self::Heartbeat { provider, .. } => provider_out(provider, A::Heartbeat),
			Self::ProviderRootCommitted { provider, .. } => {
				provider_out(provider, A::RootCommitted)
			},
			Self::AgreementProposed { agreement, .. } => agreement_out(agreement, A::Proposed),
			Self::AgreementAccepted { agreement } => agreement_out(agreement, A::Accepted),
			Self::AgreementCancelled { agreement } => agreement_out(agreement, A::Cancelled),
			Self::AgreementRenewalRequested { agreement, .. } => {
				agreement_out(agreement, A::RenewalRequested)
			},
			Self::AgreementRenewed { agreement, .. } => agreement_out(agreement, A::Renewed),
			Self::AgreementExpired { agreement } => agreement_out(agreement, A::Expired),
			Self::AgreementPruned { agreement } => agreement_out(agreement, A::Pruned),
			Self::DeletionAcknowledged { agreement, .. } => {
				agreement_out(agreement, A::DeletionAcknowledged)
			},
			Self::ChallengeIssued { challenge, .. } => challenge_out(challenge, A::Issued),
			Self::CheckpointSubmitted { challenge, .. } => {
				challenge_out(challenge, A::CheckpointSubmitted)
			},
			Self::ChallengeTimedOut { challenge, .. } => challenge_out(challenge, A::TimedOut),
			Self::DriveCreated { drive, .. } => drive_out(drive, A::Created, None),
			Self::DriveRootUpdated { drive, version } => {
				drive_out(drive, A::RootUpdated, Some(*version))
			},
			Self::DriveControllerChanged { drive, .. } => {
				drive_out(drive, A::ControllerChanged, None)
			},
			Self::DriveTransferred { drive, .. } => drive_out(drive, A::Transferred, None),
			Self::DriveArchived { drive } => drive_out(drive, A::Archived, None),
			Self::BucketCreated { bucket, .. } => bucket_out(bucket, A::Created, None),
			Self::BucketControllerChanged { bucket, version, .. } => {
				bucket_out(bucket, A::ControllerChanged, Some(*version))
			},
			Self::BucketTransferred { bucket, version, .. } => {
				bucket_out(bucket, A::Transferred, Some(*version))
			},
			Self::BucketArchived { bucket, version, .. } => {
				bucket_out(bucket, A::Archived, Some(*version))
			},
			Self::BucketVersioningChanged { bucket, version, .. } => {
				bucket_out(bucket, A::VersioningChanged, Some(*version))
			},
			Self::BucketDeleted { bucket, .. } => bucket_out(bucket, A::Deleted, None),
			Self::ObjectPut { bucket, object, version, .. } => StorageNativeOutcome::Object {
				bucket: bucket.clone(),
				object: object.clone(),
				action: A::Put,
				version: *version,
			},
			Self::ObjectDeleted { bucket, object, version, .. } => StorageNativeOutcome::Object {
				bucket: bucket.clone(),
				object: object.clone(),
				action: A::Deleted,
				version: *version,
			},
		}
	}
}
fn provider_out(id: &AccountId, action: StorageLifecycleAction) -> StorageNativeOutcome {
	StorageNativeOutcome::Provider { provider: id.clone(), action }
}
fn agreement_out(id: &AgreementId, action: StorageLifecycleAction) -> StorageNativeOutcome {
	StorageNativeOutcome::Agreement { agreement: id.clone(), action }
}
fn challenge_out(id: &ChallengeId, action: StorageLifecycleAction) -> StorageNativeOutcome {
	StorageNativeOutcome::Challenge { challenge: id.clone(), action }
}
fn drive_out(
	id: &DriveId,
	action: StorageLifecycleAction,
	version: Option<u64>,
) -> StorageNativeOutcome {
	StorageNativeOutcome::Drive { drive: id.clone(), action, version }
}
fn bucket_out(
	id: &BucketId,
	action: StorageLifecycleAction,
	version: Option<u64>,
) -> StorageNativeOutcome {
	StorageNativeOutcome::Bucket { bucket: id.clone(), action, version }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedStorageNativeEvent {
	pub finalized_block_hash: Hash32,
	pub event_index: u32,
	pub event: StorageNativeEvent,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedStorageNativeOutcome {
	pub event: FinalizedStorageNativeEvent,
	pub outcome: StorageNativeOutcome,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorageNativeEventSubscription {
	pub finality: StorageSubscriptionFinality,
	pub from_finalized_block: Hash32,
	pub kinds: Vec<StorageNativeEventKind>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageSubscriptionFinality {
	Finalized,
}
impl StorageNativeEventSubscription {
	pub fn new(
		from_finalized_block: Hash32,
		kinds: Vec<StorageNativeEventKind>,
	) -> DomainResult<Self> {
		from_finalized_block.validate()?;
		if kinds.is_empty()
			|| kinds.len() > 32
			|| kinds.iter().collect::<HashSet<_>>().len() != kinds.len()
		{
			return Err(invalid("storage event subscription requires 1-32 unique kinds"));
		}
		Ok(Self { finality: StorageSubscriptionFinality::Finalized, from_finalized_block, kinds })
	}
}
