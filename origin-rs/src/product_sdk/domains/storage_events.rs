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

//! Typed finalized lifecycle events for the exact native Commons storage surface.

use super::{
	common::{invalid, AccountId, BlockNumber, DomainResult, Hash32, Validate},
	storage_provider::{AgreementStatus, ProviderStatus},
	AgreementId, BucketId, ChallengeId, ContentCommitment, ContentHash, DriveId, ObjectId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageNativeEventKind {
	ProviderRegistered,
	ProviderUpdated,
	ProviderStatusChanged,
	ProviderRemoved,
	Heartbeat,
	StorageBucketCreated,
	StorageBucketGrantChanged,
	AgreementTransitioned,
	AgreementCapacityReleased,
	AgreementProviderRebound,
	ChallengeIssued,
	ChallengeProved,
	ChallengeTimedOut,
	CheckpointAccepted,
	CheckpointEquivocation,
	ReplicaSelected,
	PrimaryPromoted,
	BucketReplicaReplaced,
	ManifestCommitmentChanged,
	ManifestDeletionAcknowledged,
	DriveCreated,
	DriveRootUpdated,
	DriveGrantChanged,
	DriveTransferred,
	DriveArchived,
	DriveNodeWritten,
	DriveNodeRemoved,
	S3BucketCreated,
	S3ControllerChanged,
	S3BucketTransferred,
	S3BucketArchived,
	S3BucketVersioningChanged,
	S3ObjectPut,
	S3ObjectDeleted,
	S3ObjectPurged,
	S3BucketDeleted,
	S3ObjectHistoryPruned,
}
pub const ALL_STORAGE_NATIVE_EVENT_KINDS: [StorageNativeEventKind; 37] = [
	StorageNativeEventKind::ProviderRegistered,
	StorageNativeEventKind::ProviderUpdated,
	StorageNativeEventKind::ProviderStatusChanged,
	StorageNativeEventKind::ProviderRemoved,
	StorageNativeEventKind::Heartbeat,
	StorageNativeEventKind::StorageBucketCreated,
	StorageNativeEventKind::StorageBucketGrantChanged,
	StorageNativeEventKind::AgreementTransitioned,
	StorageNativeEventKind::AgreementCapacityReleased,
	StorageNativeEventKind::AgreementProviderRebound,
	StorageNativeEventKind::ChallengeIssued,
	StorageNativeEventKind::ChallengeProved,
	StorageNativeEventKind::ChallengeTimedOut,
	StorageNativeEventKind::CheckpointAccepted,
	StorageNativeEventKind::CheckpointEquivocation,
	StorageNativeEventKind::ReplicaSelected,
	StorageNativeEventKind::PrimaryPromoted,
	StorageNativeEventKind::BucketReplicaReplaced,
	StorageNativeEventKind::ManifestCommitmentChanged,
	StorageNativeEventKind::ManifestDeletionAcknowledged,
	StorageNativeEventKind::DriveCreated,
	StorageNativeEventKind::DriveRootUpdated,
	StorageNativeEventKind::DriveGrantChanged,
	StorageNativeEventKind::DriveTransferred,
	StorageNativeEventKind::DriveArchived,
	StorageNativeEventKind::DriveNodeWritten,
	StorageNativeEventKind::DriveNodeRemoved,
	StorageNativeEventKind::S3BucketCreated,
	StorageNativeEventKind::S3ControllerChanged,
	StorageNativeEventKind::S3BucketTransferred,
	StorageNativeEventKind::S3BucketArchived,
	StorageNativeEventKind::S3BucketVersioningChanged,
	StorageNativeEventKind::S3ObjectPut,
	StorageNativeEventKind::S3ObjectDeleted,
	StorageNativeEventKind::S3ObjectPurged,
	StorageNativeEventKind::S3BucketDeleted,
	StorageNativeEventKind::S3ObjectHistoryPruned,
];
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BucketRole {
	Reader,
	Writer,
	Admin,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveRole {
	Reader,
	Writer,
	Admin,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentState {
	Publishable,
	Pending,
	Tombstoned,
	Missing,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveNodeKind {
	Directory,
	File,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CheckpointCommitment {
	pub mmr_root: ContentCommitment,
	pub start_seq: u64,
	pub leaf_count: u64,
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
	StorageBucketCreated {
		bucket: BucketId,
		owner: AccountId,
		primary: AccountId,
		replicas: Vec<AccountId>,
		version: u64,
	},
	StorageBucketGrantChanged {
		bucket: BucketId,
		account: AccountId,
		role: Option<BucketRole>,
		previous_version: u64,
		new_version: u64,
	},
	AgreementTransitioned {
		agreement: AgreementId,
		previous: Option<AgreementStatus>,
		current: AgreementStatus,
		previous_version: u64,
		new_version: u64,
	},
	AgreementCapacityReleased {
		agreement: AgreementId,
	},
	AgreementProviderRebound {
		agreement: AgreementId,
		old_provider: AccountId,
		new_provider: AccountId,
		status: AgreementStatus,
		bytes: u64,
	},
	ChallengeIssued {
		challenge: ChallengeId,
		bucket: BucketId,
		provider: AccountId,
		due_at: BlockNumber,
	},
	ChallengeProved {
		challenge: ChallengeId,
		provider: AccountId,
	},
	ChallengeTimedOut {
		challenge: ChallengeId,
		provider: AccountId,
		checkpoint: BlockNumber,
	},
	CheckpointAccepted {
		bucket: BucketId,
		commitment: CheckpointCommitment,
		checkpoint: BlockNumber,
		replica_confirmations: Vec<AccountId>,
	},
	CheckpointEquivocation {
		code: u16,
		bucket: BucketId,
		provider: AccountId,
		accepted_root: ContentCommitment,
		conflicting_root: ContentCommitment,
		nonce: BlockNumber,
	},
	ReplicaSelected {
		bucket: BucketId,
		provider: AccountId,
		checkpoint: BlockNumber,
	},
	PrimaryPromoted {
		bucket: BucketId,
		old_provider: AccountId,
		new_provider: AccountId,
		checkpoint: BlockNumber,
	},
	BucketReplicaReplaced {
		bucket: BucketId,
		old_provider: AccountId,
		new_provider: AccountId,
		previous_version: u64,
		new_version: u64,
	},
	ManifestCommitmentChanged {
		manifest: ContentCommitment,
		bucket: BucketId,
		state: CommitmentState,
		checkpoint: Option<BlockNumber>,
	},
	ManifestDeletionAcknowledged {
		manifest: ContentCommitment,
		bucket: BucketId,
		provider: AccountId,
		evidence_hash: ContentCommitment,
		acknowledged_at: BlockNumber,
	},
	DriveCreated {
		drive: DriveId,
		owner: AccountId,
		version: u64,
	},
	DriveRootUpdated {
		drive: DriveId,
		previous_root: Option<ContentCommitment>,
		new_root: ContentCommitment,
		previous_version: u64,
		version: u64,
	},
	DriveGrantChanged {
		drive: DriveId,
		subject: AccountId,
		role: Option<DriveRole>,
		previous_version: u64,
		version: u64,
	},
	DriveTransferred {
		drive: DriveId,
		old_owner: AccountId,
		new_owner: AccountId,
		previous_version: u64,
		version: u64,
	},
	DriveArchived {
		drive: DriveId,
		previous_version: u64,
		version: u64,
	},
	DriveNodeWritten {
		drive: DriveId,
		path: Vec<u8>,
		kind: DriveNodeKind,
		previous_version: u64,
		version: u64,
	},
	DriveNodeRemoved {
		drive: DriveId,
		path: Vec<u8>,
		previous_version: u64,
		version: u64,
	},
	S3BucketCreated {
		bucket: BucketId,
		name: Vec<u8>,
		owner: AccountId,
	},
	S3ControllerChanged {
		bucket: BucketId,
		controller: AccountId,
		enabled: bool,
		version: u64,
	},
	S3BucketTransferred {
		bucket: BucketId,
		from: AccountId,
		to: AccountId,
		version: u64,
	},
	S3BucketArchived {
		bucket: BucketId,
		archived: bool,
		version: u64,
	},
	S3BucketVersioningChanged {
		bucket: BucketId,
		enabled: bool,
		version: u64,
	},
	S3ObjectPut {
		bucket: BucketId,
		object: ObjectId,
		key: Vec<u8>,
		content_hash: ContentHash,
		version: u64,
	},
	S3ObjectDeleted {
		bucket: BucketId,
		object: ObjectId,
		key: Vec<u8>,
		version: u64,
	},
	S3ObjectPurged {
		bucket: BucketId,
		key: Vec<u8>,
	},
	S3BucketDeleted {
		bucket: BucketId,
		name: Vec<u8>,
		owner: AccountId,
	},
	S3ObjectHistoryPruned {
		bucket: BucketId,
		key: Vec<u8>,
		through_version: u64,
		removed: u32,
	},
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "outcome", content = "data", rename_all = "snake_case")]
pub enum StorageNativeOutcome {
	Provider { action: StorageNativeEventKind, id: Option<String>, version: Option<u64> },
	StorageBucket { action: StorageNativeEventKind, id: Option<String>, version: Option<u64> },
	Agreement { action: StorageNativeEventKind, id: Option<String>, version: Option<u64> },
	Challenge { action: StorageNativeEventKind, id: Option<String>, version: Option<u64> },
	Checkpoint { action: StorageNativeEventKind, id: Option<String>, version: Option<u64> },
	Manifest { action: StorageNativeEventKind, id: Option<String>, version: Option<u64> },
	Drive { action: StorageNativeEventKind, id: Option<String>, version: Option<u64> },
	S3Bucket { action: StorageNativeEventKind, id: Option<String>, version: Option<u64> },
	S3Object { action: StorageNativeEventKind, id: Option<String>, version: Option<u64> },
}
#[derive(Clone, Copy)]
enum OutcomeCategory {
	Provider,
	StorageBucket,
	Agreement,
	Challenge,
	Checkpoint,
	Manifest,
	Drive,
	S3Bucket,
	S3Object,
}
fn storage_out(
	category: OutcomeCategory,
	action: StorageNativeEventKind,
	id: Option<String>,
	version: Option<u64>,
) -> StorageNativeOutcome {
	macro_rules! make {
		($variant:ident) => {
			StorageNativeOutcome::$variant { action, id, version }
		};
	}
	match category {
		OutcomeCategory::Provider => make!(Provider),
		OutcomeCategory::StorageBucket => make!(StorageBucket),
		OutcomeCategory::Agreement => make!(Agreement),
		OutcomeCategory::Challenge => make!(Challenge),
		OutcomeCategory::Checkpoint => make!(Checkpoint),
		OutcomeCategory::Manifest => make!(Manifest),
		OutcomeCategory::Drive => make!(Drive),
		OutcomeCategory::S3Bucket => make!(S3Bucket),
		OutcomeCategory::S3Object => make!(S3Object),
	}
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
			Self::StorageBucketCreated { .. } => K::StorageBucketCreated,
			Self::StorageBucketGrantChanged { .. } => K::StorageBucketGrantChanged,
			Self::AgreementTransitioned { .. } => K::AgreementTransitioned,
			Self::AgreementCapacityReleased { .. } => K::AgreementCapacityReleased,
			Self::AgreementProviderRebound { .. } => K::AgreementProviderRebound,
			Self::ChallengeIssued { .. } => K::ChallengeIssued,
			Self::ChallengeProved { .. } => K::ChallengeProved,
			Self::ChallengeTimedOut { .. } => K::ChallengeTimedOut,
			Self::CheckpointAccepted { .. } => K::CheckpointAccepted,
			Self::CheckpointEquivocation { .. } => K::CheckpointEquivocation,
			Self::ReplicaSelected { .. } => K::ReplicaSelected,
			Self::PrimaryPromoted { .. } => K::PrimaryPromoted,
			Self::BucketReplicaReplaced { .. } => K::BucketReplicaReplaced,
			Self::ManifestCommitmentChanged { .. } => K::ManifestCommitmentChanged,
			Self::ManifestDeletionAcknowledged { .. } => K::ManifestDeletionAcknowledged,
			Self::DriveCreated { .. } => K::DriveCreated,
			Self::DriveRootUpdated { .. } => K::DriveRootUpdated,
			Self::DriveGrantChanged { .. } => K::DriveGrantChanged,
			Self::DriveTransferred { .. } => K::DriveTransferred,
			Self::DriveArchived { .. } => K::DriveArchived,
			Self::DriveNodeWritten { .. } => K::DriveNodeWritten,
			Self::DriveNodeRemoved { .. } => K::DriveNodeRemoved,
			Self::S3BucketCreated { .. } => K::S3BucketCreated,
			Self::S3ControllerChanged { .. } => K::S3ControllerChanged,
			Self::S3BucketTransferred { .. } => K::S3BucketTransferred,
			Self::S3BucketArchived { .. } => K::S3BucketArchived,
			Self::S3BucketVersioningChanged { .. } => K::S3BucketVersioningChanged,
			Self::S3ObjectPut { .. } => K::S3ObjectPut,
			Self::S3ObjectDeleted { .. } => K::S3ObjectDeleted,
			Self::S3ObjectPurged { .. } => K::S3ObjectPurged,
			Self::S3BucketDeleted { .. } => K::S3BucketDeleted,
			Self::S3ObjectHistoryPruned { .. } => K::S3ObjectHistoryPruned,
		}
	}
	pub fn outcome(&self) -> StorageNativeOutcome {
		use StorageNativeEventKind as K;
		match self {
			Self::ProviderRegistered { provider, .. } => storage_out(
				OutcomeCategory::Provider,
				K::ProviderRegistered,
				Some(provider.as_str().to_owned()),
				None,
			),
			Self::ProviderUpdated { provider, .. } => storage_out(
				OutcomeCategory::Provider,
				K::ProviderUpdated,
				Some(provider.as_str().to_owned()),
				None,
			),
			Self::ProviderStatusChanged { provider, .. } => storage_out(
				OutcomeCategory::Provider,
				K::ProviderStatusChanged,
				Some(provider.as_str().to_owned()),
				None,
			),
			Self::ProviderRemoved { provider, .. } => storage_out(
				OutcomeCategory::Provider,
				K::ProviderRemoved,
				Some(provider.as_str().to_owned()),
				None,
			),
			Self::Heartbeat { provider, .. } => storage_out(
				OutcomeCategory::Provider,
				K::Heartbeat,
				Some(provider.as_str().to_owned()),
				None,
			),
			Self::StorageBucketCreated { bucket, version, .. } => storage_out(
				OutcomeCategory::StorageBucket,
				K::StorageBucketCreated,
				Some(bucket.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::StorageBucketGrantChanged { bucket, new_version, .. } => storage_out(
				OutcomeCategory::StorageBucket,
				K::StorageBucketGrantChanged,
				Some(bucket.as_hash().as_str().to_owned()),
				Some(*new_version),
			),
			Self::AgreementTransitioned { agreement, new_version, .. } => storage_out(
				OutcomeCategory::Agreement,
				K::AgreementTransitioned,
				Some(agreement.as_hash().as_str().to_owned()),
				Some(*new_version),
			),
			Self::AgreementCapacityReleased { agreement, .. } => storage_out(
				OutcomeCategory::Agreement,
				K::AgreementCapacityReleased,
				Some(agreement.as_hash().as_str().to_owned()),
				None,
			),
			Self::AgreementProviderRebound { agreement, .. } => storage_out(
				OutcomeCategory::Agreement,
				K::AgreementProviderRebound,
				Some(agreement.as_hash().as_str().to_owned()),
				None,
			),
			Self::ChallengeIssued { challenge, .. } => storage_out(
				OutcomeCategory::Challenge,
				K::ChallengeIssued,
				Some(challenge.as_hash().as_str().to_owned()),
				None,
			),
			Self::ChallengeProved { challenge, .. } => storage_out(
				OutcomeCategory::Challenge,
				K::ChallengeProved,
				Some(challenge.as_hash().as_str().to_owned()),
				None,
			),
			Self::ChallengeTimedOut { challenge, .. } => storage_out(
				OutcomeCategory::Challenge,
				K::ChallengeTimedOut,
				Some(challenge.as_hash().as_str().to_owned()),
				None,
			),
			Self::CheckpointAccepted { bucket, .. } => storage_out(
				OutcomeCategory::Checkpoint,
				K::CheckpointAccepted,
				Some(bucket.as_hash().as_str().to_owned()),
				None,
			),
			Self::CheckpointEquivocation { bucket, .. } => storage_out(
				OutcomeCategory::Checkpoint,
				K::CheckpointEquivocation,
				Some(bucket.as_hash().as_str().to_owned()),
				None,
			),
			Self::ReplicaSelected { bucket, .. } => storage_out(
				OutcomeCategory::Checkpoint,
				K::ReplicaSelected,
				Some(bucket.as_hash().as_str().to_owned()),
				None,
			),
			Self::PrimaryPromoted { bucket, .. } => storage_out(
				OutcomeCategory::Checkpoint,
				K::PrimaryPromoted,
				Some(bucket.as_hash().as_str().to_owned()),
				None,
			),
			Self::BucketReplicaReplaced { bucket, new_version, .. } => storage_out(
				OutcomeCategory::Checkpoint,
				K::BucketReplicaReplaced,
				Some(bucket.as_hash().as_str().to_owned()),
				Some(*new_version),
			),
			Self::ManifestCommitmentChanged { manifest, .. } => storage_out(
				OutcomeCategory::Manifest,
				K::ManifestCommitmentChanged,
				Some(manifest.as_hash().as_str().to_owned()),
				None,
			),
			Self::ManifestDeletionAcknowledged { manifest, .. } => storage_out(
				OutcomeCategory::Manifest,
				K::ManifestDeletionAcknowledged,
				Some(manifest.as_hash().as_str().to_owned()),
				None,
			),
			Self::DriveCreated { drive, version, .. } => storage_out(
				OutcomeCategory::Drive,
				K::DriveCreated,
				Some(drive.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::DriveRootUpdated { drive, version, .. } => storage_out(
				OutcomeCategory::Drive,
				K::DriveRootUpdated,
				Some(drive.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::DriveGrantChanged { drive, version, .. } => storage_out(
				OutcomeCategory::Drive,
				K::DriveGrantChanged,
				Some(drive.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::DriveTransferred { drive, version, .. } => storage_out(
				OutcomeCategory::Drive,
				K::DriveTransferred,
				Some(drive.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::DriveArchived { drive, version, .. } => storage_out(
				OutcomeCategory::Drive,
				K::DriveArchived,
				Some(drive.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::DriveNodeWritten { drive, version, .. } => storage_out(
				OutcomeCategory::Drive,
				K::DriveNodeWritten,
				Some(drive.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::DriveNodeRemoved { drive, version, .. } => storage_out(
				OutcomeCategory::Drive,
				K::DriveNodeRemoved,
				Some(drive.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::S3BucketCreated { bucket, .. } => storage_out(
				OutcomeCategory::S3Bucket,
				K::S3BucketCreated,
				Some(bucket.as_hash().as_str().to_owned()),
				None,
			),
			Self::S3ControllerChanged { bucket, version, .. } => storage_out(
				OutcomeCategory::S3Bucket,
				K::S3ControllerChanged,
				Some(bucket.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::S3BucketTransferred { bucket, version, .. } => storage_out(
				OutcomeCategory::S3Bucket,
				K::S3BucketTransferred,
				Some(bucket.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::S3BucketArchived { bucket, version, .. } => storage_out(
				OutcomeCategory::S3Bucket,
				K::S3BucketArchived,
				Some(bucket.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::S3BucketVersioningChanged { bucket, version, .. } => storage_out(
				OutcomeCategory::S3Bucket,
				K::S3BucketVersioningChanged,
				Some(bucket.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::S3ObjectPut { object, version, .. } => storage_out(
				OutcomeCategory::S3Object,
				K::S3ObjectPut,
				Some(object.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::S3ObjectDeleted { object, version, .. } => storage_out(
				OutcomeCategory::S3Object,
				K::S3ObjectDeleted,
				Some(object.as_hash().as_str().to_owned()),
				Some(*version),
			),
			Self::S3ObjectPurged { bucket, key } => storage_out(
				OutcomeCategory::S3Object,
				K::S3ObjectPurged,
				Some(format!("s3-key:{}:{}", bucket.as_hash().as_str(), hex::encode(key))),
				None,
			),
			Self::S3BucketDeleted { bucket, .. } => storage_out(
				OutcomeCategory::S3Bucket,
				K::S3BucketDeleted,
				Some(bucket.as_hash().as_str().to_owned()),
				None,
			),
			Self::S3ObjectHistoryPruned { bucket, key, through_version, .. } => storage_out(
				OutcomeCategory::S3Object,
				K::S3ObjectHistoryPruned,
				Some(format!("s3-key:{}:{}", bucket.as_hash().as_str(), hex::encode(key))),
				Some(*through_version),
			),
		}
	}
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
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageSubscriptionFinality {
	Finalized,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorageNativeEventSubscription {
	pub finality: StorageSubscriptionFinality,
	pub from_finalized_block: Hash32,
	pub kinds: Vec<StorageNativeEventKind>,
}
impl StorageNativeEventSubscription {
	pub fn new(
		from_finalized_block: Hash32,
		kinds: Vec<StorageNativeEventKind>,
	) -> DomainResult<Self> {
		from_finalized_block.validate()?;
		if kinds.is_empty()
			|| kinds.len() > 37
			|| kinds.iter().collect::<HashSet<_>>().len() != kinds.len()
		{
			return Err(invalid("storage event subscription requires 1-37 unique kinds"));
		}
		Ok(Self { finality: StorageSubscriptionFinality::Finalized, from_finalized_block, kinds })
	}
}
