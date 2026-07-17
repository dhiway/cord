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

//! Executable finalized native Commons storage event subscription.
use super::{
	domains::{
		storage_events::*,
		storage_provider::{AgreementStatus, ProviderStatus},
		AccountId, AgreementId, BucketId, ChallengeId, ContentCommitment, ContentHash,
		DomainResult, DriveId, Hash32, ObjectId,
	},
	NativeError, NativeErrorCode,
};
use crate::{
	config::OrbisConfig,
	types::account::{account_id_from_subxt, account_id_to_ss58},
};
use futures::{Stream, StreamExt};
use scale_decode::DecodeAsType;
use std::{collections::VecDeque, pin::Pin};
use subxt::{blocks::Block, events::StaticEvent, OnlineClient};
use unicode_normalization::UnicodeNormalization;
type A = subxt::utils::AccountId32;
type H = subxt::utils::H256;
type RuntimeBlock = Block<OrbisConfig, OnlineClient<OrbisConfig>>;
type FinalizedBlockStream =
	Pin<Box<dyn Stream<Item = Result<RuntimeBlock, subxt::Error>> + Send + 'static>>;
#[derive(DecodeAsType)]
enum ProviderStatusWire {
	Active,
	Suspended,
}
#[derive(DecodeAsType)]
enum AgreementStatusWire {
	Proposed,
	Active,
	Suspended,
	Cancelled,
	Expired,
}
#[derive(DecodeAsType)]
enum BucketRoleWire {
	Reader,
	Writer,
	Admin,
}
#[derive(DecodeAsType)]
enum DriveRoleWire {
	Reader,
	Writer,
	Admin,
}
#[derive(DecodeAsType)]
enum CommitmentStateWire {
	Publishable,
	Pending,
	Tombstoned,
	Missing,
}
#[derive(DecodeAsType)]
enum DriveNodeKindWire {
	Directory,
	File,
}
#[derive(DecodeAsType)]
struct CommitmentWire {
	mmr_root: H,
	start_seq: u64,
	leaf_count: u64,
}
macro_rules! wire{($name:ident,$pallet:literal,$event:literal,{$($field:ident:$ty:ty),*$(,)?})=>{#[derive(DecodeAsType)]struct $name{$($field:$ty,)*}impl StaticEvent for $name{const PALLET:&'static str=$pallet;const EVENT:&'static str=$event;}}}
wire!(ProviderRegisteredWire,"StorageProvider","ProviderRegistered",{provider:A,capacity_bytes:u64});
wire!(ProviderUpdatedWire,"StorageProvider","ProviderUpdated",{provider:A,capacity_bytes:u64});
wire!(ProviderStatusChangedWire,"StorageProvider","ProviderStatusChanged",{provider:A,status:ProviderStatusWire});
wire!(ProviderRemovedWire,"StorageProvider","ProviderRemoved",{provider:A});
wire!(HeartbeatWire,"StorageProvider","Heartbeat",{provider:A,at:u32});
wire!(StorageBucketCreatedWire,"StorageProvider","BucketCreated",{bucket_id:H,owner:A,primary:A,replicas:Vec<A>,version:u64});
wire!(StorageBucketGrantChangedWire,"StorageProvider","BucketGrantChanged",{bucket_id:H,account:A,role:Option<BucketRoleWire>,previous_version:u64,new_version:u64});
wire!(AgreementTransitionedWire,"StorageProvider","AgreementTransitioned",{agreement_id:H,previous:Option<AgreementStatusWire>,current:AgreementStatusWire,previous_version:u64,new_version:u64});
wire!(AgreementCapacityReleasedWire,"StorageProvider","AgreementCapacityReleased",{agreement_id:H});
wire!(AgreementProviderReboundWire,"StorageProvider","AgreementProviderRebound",{agreement_id:H,old_provider:A,new_provider:A,status:AgreementStatusWire,bytes:u64});
wire!(ChallengeIssuedWire,"StorageProvider","ChallengeIssued",{challenge_id:H,bucket_id:H,provider:A,due_at:u32});
wire!(ChallengeProvedWire,"StorageProvider","ChallengeProved",{challenge_id:H,provider:A});
wire!(ChallengeTimedOutWire,"StorageProvider","ChallengeTimedOut",{challenge_id:H,provider:A,checkpoint:u32});
wire!(CheckpointAcceptedWire,"StorageProvider","CheckpointAccepted",{bucket_id:H,commitment:CommitmentWire,checkpoint:u32,replica_confirmations:Vec<A>});
wire!(CheckpointEquivocationWire,"StorageProvider","CheckpointEquivocation",{code:u16,bucket_id:H,provider:A,accepted_root:H,conflicting_root:H,nonce:u32});
wire!(ReplicaSelectedWire,"StorageProvider","ReplicaSelected",{bucket_id:H,provider:A,checkpoint:u32});
wire!(PrimaryPromotedWire,"StorageProvider","PrimaryPromoted",{bucket_id:H,old_provider:A,new_provider:A,checkpoint:u32});
wire!(BucketReplicaReplacedWire,"StorageProvider","BucketReplicaReplaced",{bucket_id:H,old_provider:A,new_provider:A,previous_version:u64,new_version:u64});
wire!(ManifestCommitmentChangedWire,"StorageProvider","ManifestCommitmentChanged",{manifest:[u8;32],bucket_id:H,state:CommitmentStateWire,checkpoint:Option<u32>});
wire!(ManifestDeletionAcknowledgedWire,"StorageProvider","ManifestDeletionAcknowledged",{manifest:[u8;32],bucket_id:H,provider:A,evidence_hash:H,acknowledged_at:u32});
wire!(DriveCreatedWire,"Drive","DriveCreated",{drive_id:H,owner:A,version:u64});
wire!(DriveRootUpdatedWire,"Drive","DriveRootUpdated",{drive_id:H,previous_root:Option<[u8;32]>,new_root:[u8;32],previous_version:u64,version:u64});
wire!(DriveGrantChangedWire,"Drive","GrantChanged",{drive_id:H,subject:A,role:Option<DriveRoleWire>,previous_version:u64,version:u64});
wire!(DriveTransferredWire,"Drive","DriveTransferred",{drive_id:H,old_owner:A,new_owner:A,previous_version:u64,version:u64});
wire!(DriveArchivedWire,"Drive","DriveArchived",{drive_id:H,previous_version:u64,version:u64});
wire!(DriveNodeWrittenWire,"Drive","NodeWritten",{drive_id:H,path:Vec<u8>,kind:DriveNodeKindWire,previous_version:u64,version:u64});
wire!(DriveNodeRemovedWire,"Drive","NodeRemoved",{drive_id:H,path:Vec<u8>,previous_version:u64,version:u64});
wire!(S3BucketCreatedWire,"S3","BucketCreated",{bucket:H,name:Vec<u8>,owner:A});
wire!(S3ControllerChangedWire,"S3","ControllerChanged",{bucket:H,controller:A,enabled:bool,version:u64});
wire!(S3BucketTransferredWire,"S3","BucketTransferred",{bucket:H,from:A,to:A,version:u64});
wire!(S3BucketArchivedWire,"S3","BucketArchived",{bucket:H,archived:bool,version:u64});
wire!(S3BucketVersioningChangedWire,"S3","BucketVersioningChanged",{bucket:H,enabled:bool,version:u64});
wire!(S3ObjectPutWire,"S3","ObjectPut",{bucket:H,object:H,key:Vec<u8>,content_hash:[u8;32],version:u64});
wire!(S3ObjectDeletedWire,"S3","ObjectDeleted",{bucket:H,object:H,key:Vec<u8>,version:u64});
wire!(S3ObjectPurgedWire,"S3","ObjectPurged",{bucket:H,key:Vec<u8>});
wire!(S3BucketDeletedWire,"S3","BucketDeleted",{bucket:H,name:Vec<u8>,owner:A});
wire!(S3ObjectHistoryPrunedWire,"S3","ObjectHistoryPruned",{bucket:H,key:Vec<u8>,through_version:u64,removed:u32});
#[cfg(test)]
const CURATED_EVENT_VARIANTS: [(&str, &str); 37] = [
	(
		<ProviderRegisteredWire as StaticEvent>::PALLET,
		<ProviderRegisteredWire as StaticEvent>::EVENT,
	),
	(<ProviderUpdatedWire as StaticEvent>::PALLET, <ProviderUpdatedWire as StaticEvent>::EVENT),
	(
		<ProviderStatusChangedWire as StaticEvent>::PALLET,
		<ProviderStatusChangedWire as StaticEvent>::EVENT,
	),
	(<ProviderRemovedWire as StaticEvent>::PALLET, <ProviderRemovedWire as StaticEvent>::EVENT),
	(<HeartbeatWire as StaticEvent>::PALLET, <HeartbeatWire as StaticEvent>::EVENT),
	(
		<StorageBucketCreatedWire as StaticEvent>::PALLET,
		<StorageBucketCreatedWire as StaticEvent>::EVENT,
	),
	(
		<StorageBucketGrantChangedWire as StaticEvent>::PALLET,
		<StorageBucketGrantChangedWire as StaticEvent>::EVENT,
	),
	(
		<AgreementTransitionedWire as StaticEvent>::PALLET,
		<AgreementTransitionedWire as StaticEvent>::EVENT,
	),
	(
		<AgreementCapacityReleasedWire as StaticEvent>::PALLET,
		<AgreementCapacityReleasedWire as StaticEvent>::EVENT,
	),
	(
		<AgreementProviderReboundWire as StaticEvent>::PALLET,
		<AgreementProviderReboundWire as StaticEvent>::EVENT,
	),
	(<ChallengeIssuedWire as StaticEvent>::PALLET, <ChallengeIssuedWire as StaticEvent>::EVENT),
	(<ChallengeProvedWire as StaticEvent>::PALLET, <ChallengeProvedWire as StaticEvent>::EVENT),
	(<ChallengeTimedOutWire as StaticEvent>::PALLET, <ChallengeTimedOutWire as StaticEvent>::EVENT),
	(
		<CheckpointAcceptedWire as StaticEvent>::PALLET,
		<CheckpointAcceptedWire as StaticEvent>::EVENT,
	),
	(
		<CheckpointEquivocationWire as StaticEvent>::PALLET,
		<CheckpointEquivocationWire as StaticEvent>::EVENT,
	),
	(<ReplicaSelectedWire as StaticEvent>::PALLET, <ReplicaSelectedWire as StaticEvent>::EVENT),
	(<PrimaryPromotedWire as StaticEvent>::PALLET, <PrimaryPromotedWire as StaticEvent>::EVENT),
	(
		<BucketReplicaReplacedWire as StaticEvent>::PALLET,
		<BucketReplicaReplacedWire as StaticEvent>::EVENT,
	),
	(
		<ManifestCommitmentChangedWire as StaticEvent>::PALLET,
		<ManifestCommitmentChangedWire as StaticEvent>::EVENT,
	),
	(
		<ManifestDeletionAcknowledgedWire as StaticEvent>::PALLET,
		<ManifestDeletionAcknowledgedWire as StaticEvent>::EVENT,
	),
	(<DriveCreatedWire as StaticEvent>::PALLET, <DriveCreatedWire as StaticEvent>::EVENT),
	(<DriveRootUpdatedWire as StaticEvent>::PALLET, <DriveRootUpdatedWire as StaticEvent>::EVENT),
	(<DriveGrantChangedWire as StaticEvent>::PALLET, <DriveGrantChangedWire as StaticEvent>::EVENT),
	(<DriveTransferredWire as StaticEvent>::PALLET, <DriveTransferredWire as StaticEvent>::EVENT),
	(<DriveArchivedWire as StaticEvent>::PALLET, <DriveArchivedWire as StaticEvent>::EVENT),
	(<DriveNodeWrittenWire as StaticEvent>::PALLET, <DriveNodeWrittenWire as StaticEvent>::EVENT),
	(<DriveNodeRemovedWire as StaticEvent>::PALLET, <DriveNodeRemovedWire as StaticEvent>::EVENT),
	(<S3BucketCreatedWire as StaticEvent>::PALLET, <S3BucketCreatedWire as StaticEvent>::EVENT),
	(
		<S3ControllerChangedWire as StaticEvent>::PALLET,
		<S3ControllerChangedWire as StaticEvent>::EVENT,
	),
	(
		<S3BucketTransferredWire as StaticEvent>::PALLET,
		<S3BucketTransferredWire as StaticEvent>::EVENT,
	),
	(<S3BucketArchivedWire as StaticEvent>::PALLET, <S3BucketArchivedWire as StaticEvent>::EVENT),
	(
		<S3BucketVersioningChangedWire as StaticEvent>::PALLET,
		<S3BucketVersioningChangedWire as StaticEvent>::EVENT,
	),
	(<S3ObjectPutWire as StaticEvent>::PALLET, <S3ObjectPutWire as StaticEvent>::EVENT),
	(<S3ObjectDeletedWire as StaticEvent>::PALLET, <S3ObjectDeletedWire as StaticEvent>::EVENT),
	(<S3ObjectPurgedWire as StaticEvent>::PALLET, <S3ObjectPurgedWire as StaticEvent>::EVENT),
	(<S3BucketDeletedWire as StaticEvent>::PALLET, <S3BucketDeletedWire as StaticEvent>::EVENT),
	(
		<S3ObjectHistoryPrunedWire as StaticEvent>::PALLET,
		<S3ObjectHistoryPrunedWire as StaticEvent>::EVENT,
	),
];
pub struct OrbisStorageEventSubscription {
	blocks: FinalizedBlockStream,
	kinds: Vec<StorageNativeEventKind>,
	pending: VecDeque<FinalizedStorageNativeOutcome>,
}
impl OrbisStorageEventSubscription {
	pub async fn subscribe(
		client: OnlineClient<OrbisConfig>,
		subscription: StorageNativeEventSubscription,
	) -> DomainResult<Self> {
		let latest = client.blocks().at_latest().await.map_err(subscription_error)?;
		let requested = runtime_hash(&subscription.from_finalized_block)?;
		if latest.hash() != requested {
			return Err(NativeError::new(
				NativeErrorCode::InconsistentSnapshot,
				"storage subscription anchor must equal the current finalized block",
			));
		}
		let blocks = client.blocks().subscribe_finalized().await.map_err(subscription_error)?;
		Ok(Self { blocks: Box::pin(blocks), kinds: subscription.kinds, pending: VecDeque::new() })
	}
	pub async fn next(&mut self) -> DomainResult<Option<FinalizedStorageNativeOutcome>> {
		loop {
			if let Some(v) = self.pending.pop_front() {
				return Ok(Some(v));
			}
			let Some(block) = self.blocks.next().await else { return Ok(None) };
			let block = block.map_err(subscription_error)?;
			let finalized = domain_hash(block.hash());
			let events = block.events().await.map_err(subscription_error)?;
			for details in events.iter() {
				let details = details.map_err(subscription_error)?;
				if !matches!(details.pallet_name(), "StorageProvider" | "Drive" | "S3") {
					continue;
				}
				if is_known_unexposed(details.pallet_name(), details.variant_name()) {
					continue;
				}
				let event = decode_event(&details)?;
				if self.kinds.contains(&event.kind()) {
					let outcome = event.outcome();
					self.pending.push_back(FinalizedStorageNativeOutcome {
						event: FinalizedStorageNativeEvent {
							finalized_block_hash: finalized.clone(),
							event_index: details.index(),
							event,
						},
						outcome,
					})
				}
			}
		}
	}
}
fn is_known_unexposed(p: &str, e: &str) -> bool {
	p == "StorageProvider"
		&& matches!(
			e,
			"ServiceKeyRotationScheduled"
				| "ServiceKeyRotated"
				| "ProviderOrganizationRotated"
				| "ProviderAuthorityRefreshed"
				| "BucketAuthorityRefreshed"
				| "BucketReconciliationDeferred"
				| "FinalizedCheckpointAdvanced"
				| "HostDelegationCreated"
				| "HostDelegationKeyRotated"
				| "HostDelegationRevoked"
				| "ChallengeEvidenceOverflowed"
				| "EvidenceRecorded"
				| "ProviderIneligible"
				| "CheckpointFallbackPromotionPendingQuorum"
		)
}
fn decode_event(d: &subxt::events::EventDetails<OrbisConfig>) -> DomainResult<StorageNativeEvent> {
	macro_rules! dec {
		($t:ty) => {
			d.as_event::<$t>().map_err(subscription_error)?.ok_or_else(|| {
				NativeError::new(
					NativeErrorCode::UnsupportedRuntime,
					"storage native event metadata mismatch",
				)
			})?
		};
	}
	Ok(match (d.pallet_name(), d.variant_name()) {
		("StorageProvider", "ProviderRegistered") => {
			let x = dec!(ProviderRegisteredWire);
			StorageNativeEvent::ProviderRegistered {
				provider: account(&x.provider)?,
				capacity_bytes: x.capacity_bytes,
			}
		},
		("StorageProvider", "ProviderUpdated") => {
			let x = dec!(ProviderUpdatedWire);
			StorageNativeEvent::ProviderUpdated {
				provider: account(&x.provider)?,
				capacity_bytes: x.capacity_bytes,
			}
		},
		("StorageProvider", "ProviderStatusChanged") => {
			let x = dec!(ProviderStatusChangedWire);
			StorageNativeEvent::ProviderStatusChanged {
				provider: account(&x.provider)?,
				status: provider_status(x.status),
			}
		},
		("StorageProvider", "ProviderRemoved") => {
			let x = dec!(ProviderRemovedWire);
			StorageNativeEvent::ProviderRemoved { provider: account(&x.provider)? }
		},
		("StorageProvider", "Heartbeat") => {
			let x = dec!(HeartbeatWire);
			StorageNativeEvent::Heartbeat { provider: account(&x.provider)?, at: x.at }
		},
		("StorageProvider", "BucketCreated") => {
			let x = dec!(StorageBucketCreatedWire);
			let replicas = accounts(x.replicas, 2, 4, false, Some(&x.primary))?;
			StorageNativeEvent::StorageBucketCreated {
				bucket: BucketId(hash(x.bucket_id)),
				owner: account(&x.owner)?,
				primary: account(&x.primary)?,
				replicas,
				version: x.version,
			}
		},
		("StorageProvider", "BucketGrantChanged") => {
			let x = dec!(StorageBucketGrantChangedWire);
			StorageNativeEvent::StorageBucketGrantChanged {
				bucket: BucketId(hash(x.bucket_id)),
				account: account(&x.account)?,
				role: x.role.map(bucket_role),
				previous_version: x.previous_version,
				new_version: x.new_version,
			}
		},
		("StorageProvider", "AgreementTransitioned") => {
			let x = dec!(AgreementTransitionedWire);
			StorageNativeEvent::AgreementTransitioned {
				agreement: AgreementId(hash(x.agreement_id)),
				previous: x.previous.map(agreement_status),
				current: agreement_status(x.current),
				previous_version: x.previous_version,
				new_version: x.new_version,
			}
		},
		("StorageProvider", "AgreementCapacityReleased") => {
			let x = dec!(AgreementCapacityReleasedWire);
			StorageNativeEvent::AgreementCapacityReleased {
				agreement: AgreementId(hash(x.agreement_id)),
			}
		},
		("StorageProvider", "AgreementProviderRebound") => {
			let x = dec!(AgreementProviderReboundWire);
			StorageNativeEvent::AgreementProviderRebound {
				agreement: AgreementId(hash(x.agreement_id)),
				old_provider: account(&x.old_provider)?,
				new_provider: account(&x.new_provider)?,
				status: agreement_status(x.status),
				bytes: x.bytes,
			}
		},
		("StorageProvider", "ChallengeIssued") => {
			let x = dec!(ChallengeIssuedWire);
			StorageNativeEvent::ChallengeIssued {
				challenge: ChallengeId(hash(x.challenge_id)),
				bucket: BucketId(hash(x.bucket_id)),
				provider: account(&x.provider)?,
				due_at: x.due_at,
			}
		},
		("StorageProvider", "ChallengeProved") => {
			let x = dec!(ChallengeProvedWire);
			StorageNativeEvent::ChallengeProved {
				challenge: ChallengeId(hash(x.challenge_id)),
				provider: account(&x.provider)?,
			}
		},
		("StorageProvider", "ChallengeTimedOut") => {
			let x = dec!(ChallengeTimedOutWire);
			StorageNativeEvent::ChallengeTimedOut {
				challenge: ChallengeId(hash(x.challenge_id)),
				provider: account(&x.provider)?,
				checkpoint: x.checkpoint,
			}
		},
		("StorageProvider", "CheckpointAccepted") => {
			let x = dec!(CheckpointAcceptedWire);
			let commitment = checkpoint_commitment(x.commitment)?;
			StorageNativeEvent::CheckpointAccepted {
				bucket: BucketId(hash(x.bucket_id)),
				commitment,
				checkpoint: x.checkpoint,
				replica_confirmations: accounts(x.replica_confirmations, 2, 2, true, None)?,
			}
		},
		("StorageProvider", "CheckpointEquivocation") => {
			let x = dec!(CheckpointEquivocationWire);
			StorageNativeEvent::CheckpointEquivocation {
				code: x.code,
				bucket: BucketId(hash(x.bucket_id)),
				provider: account(&x.provider)?,
				accepted_root: ContentCommitment(hash(x.accepted_root)),
				conflicting_root: ContentCommitment(hash(x.conflicting_root)),
				nonce: x.nonce,
			}
		},
		("StorageProvider", "ReplicaSelected") => {
			let x = dec!(ReplicaSelectedWire);
			StorageNativeEvent::ReplicaSelected {
				bucket: BucketId(hash(x.bucket_id)),
				provider: account(&x.provider)?,
				checkpoint: x.checkpoint,
			}
		},
		("StorageProvider", "PrimaryPromoted") => {
			let x = dec!(PrimaryPromotedWire);
			StorageNativeEvent::PrimaryPromoted {
				bucket: BucketId(hash(x.bucket_id)),
				old_provider: account(&x.old_provider)?,
				new_provider: account(&x.new_provider)?,
				checkpoint: x.checkpoint,
			}
		},
		("StorageProvider", "BucketReplicaReplaced") => {
			let x = dec!(BucketReplicaReplacedWire);
			StorageNativeEvent::BucketReplicaReplaced {
				bucket: BucketId(hash(x.bucket_id)),
				old_provider: account(&x.old_provider)?,
				new_provider: account(&x.new_provider)?,
				previous_version: x.previous_version,
				new_version: x.new_version,
			}
		},
		("StorageProvider", "ManifestCommitmentChanged") => {
			let x = dec!(ManifestCommitmentChangedWire);
			StorageNativeEvent::ManifestCommitmentChanged {
				manifest: ContentCommitment(Hash32::from_bytes(x.manifest)),
				bucket: BucketId(hash(x.bucket_id)),
				state: commitment_state(x.state),
				checkpoint: x.checkpoint,
			}
		},
		("StorageProvider", "ManifestDeletionAcknowledged") => {
			let x = dec!(ManifestDeletionAcknowledgedWire);
			StorageNativeEvent::ManifestDeletionAcknowledged {
				manifest: ContentCommitment(Hash32::from_bytes(x.manifest)),
				bucket: BucketId(hash(x.bucket_id)),
				provider: account(&x.provider)?,
				evidence_hash: ContentCommitment(hash(x.evidence_hash)),
				acknowledged_at: x.acknowledged_at,
			}
		},
		("Drive", "DriveCreated") => {
			let x = dec!(DriveCreatedWire);
			StorageNativeEvent::DriveCreated {
				drive: DriveId(hash(x.drive_id)),
				owner: account(&x.owner)?,
				version: x.version,
			}
		},
		("Drive", "DriveRootUpdated") => {
			let x = dec!(DriveRootUpdatedWire);
			StorageNativeEvent::DriveRootUpdated {
				drive: DriveId(hash(x.drive_id)),
				previous_root: x.previous_root.map(|v| ContentCommitment(Hash32::from_bytes(v))),
				new_root: ContentCommitment(Hash32::from_bytes(x.new_root)),
				previous_version: x.previous_version,
				version: x.version,
			}
		},
		("Drive", "GrantChanged") => {
			let x = dec!(DriveGrantChangedWire);
			StorageNativeEvent::DriveGrantChanged {
				drive: DriveId(hash(x.drive_id)),
				subject: account(&x.subject)?,
				role: x.role.map(drive_role),
				previous_version: x.previous_version,
				version: x.version,
			}
		},
		("Drive", "DriveTransferred") => {
			let x = dec!(DriveTransferredWire);
			StorageNativeEvent::DriveTransferred {
				drive: DriveId(hash(x.drive_id)),
				old_owner: account(&x.old_owner)?,
				new_owner: account(&x.new_owner)?,
				previous_version: x.previous_version,
				version: x.version,
			}
		},
		("Drive", "DriveArchived") => {
			let x = dec!(DriveArchivedWire);
			StorageNativeEvent::DriveArchived {
				drive: DriveId(hash(x.drive_id)),
				previous_version: x.previous_version,
				version: x.version,
			}
		},
		("Drive", "NodeWritten") => {
			let x = dec!(DriveNodeWrittenWire);
			validate_drive_path(&x.path)?;
			StorageNativeEvent::DriveNodeWritten {
				drive: DriveId(hash(x.drive_id)),
				path: x.path,
				kind: drive_node_kind(x.kind),
				previous_version: x.previous_version,
				version: x.version,
			}
		},
		("Drive", "NodeRemoved") => {
			let x = dec!(DriveNodeRemovedWire);
			validate_drive_path(&x.path)?;
			StorageNativeEvent::DriveNodeRemoved {
				drive: DriveId(hash(x.drive_id)),
				path: x.path,
				previous_version: x.previous_version,
				version: x.version,
			}
		},
		("S3", "BucketCreated") => {
			let x = dec!(S3BucketCreatedWire);
			validate_s3_bucket_name(&x.name)?;
			StorageNativeEvent::S3BucketCreated {
				bucket: BucketId(hash(x.bucket)),
				name: x.name,
				owner: account(&x.owner)?,
			}
		},
		("S3", "ControllerChanged") => {
			let x = dec!(S3ControllerChangedWire);
			StorageNativeEvent::S3ControllerChanged {
				bucket: BucketId(hash(x.bucket)),
				controller: account(&x.controller)?,
				enabled: x.enabled,
				version: x.version,
			}
		},
		("S3", "BucketTransferred") => {
			let x = dec!(S3BucketTransferredWire);
			StorageNativeEvent::S3BucketTransferred {
				bucket: BucketId(hash(x.bucket)),
				from: account(&x.from)?,
				to: account(&x.to)?,
				version: x.version,
			}
		},
		("S3", "BucketArchived") => {
			let x = dec!(S3BucketArchivedWire);
			StorageNativeEvent::S3BucketArchived {
				bucket: BucketId(hash(x.bucket)),
				archived: x.archived,
				version: x.version,
			}
		},
		("S3", "BucketVersioningChanged") => {
			let x = dec!(S3BucketVersioningChangedWire);
			StorageNativeEvent::S3BucketVersioningChanged {
				bucket: BucketId(hash(x.bucket)),
				enabled: x.enabled,
				version: x.version,
			}
		},
		("S3", "ObjectPut") => {
			let x = dec!(S3ObjectPutWire);
			validate_s3_object_key(&x.key)?;
			StorageNativeEvent::S3ObjectPut {
				bucket: BucketId(hash(x.bucket)),
				object: ObjectId(hash(x.object)),
				key: x.key,
				content_hash: ContentHash(Hash32::from_bytes(x.content_hash)),
				version: x.version,
			}
		},
		("S3", "ObjectDeleted") => {
			let x = dec!(S3ObjectDeletedWire);
			validate_s3_object_key(&x.key)?;
			StorageNativeEvent::S3ObjectDeleted {
				bucket: BucketId(hash(x.bucket)),
				object: ObjectId(hash(x.object)),
				key: x.key,
				version: x.version,
			}
		},
		("S3", "ObjectPurged") => {
			let x = dec!(S3ObjectPurgedWire);
			validate_s3_object_key(&x.key)?;
			StorageNativeEvent::S3ObjectPurged { bucket: BucketId(hash(x.bucket)), key: x.key }
		},
		("S3", "BucketDeleted") => {
			let x = dec!(S3BucketDeletedWire);
			validate_s3_bucket_name(&x.name)?;
			StorageNativeEvent::S3BucketDeleted {
				bucket: BucketId(hash(x.bucket)),
				name: x.name,
				owner: account(&x.owner)?,
			}
		},
		("S3", "ObjectHistoryPruned") => {
			let x = dec!(S3ObjectHistoryPrunedWire);
			validate_s3_object_key(&x.key)?;
			StorageNativeEvent::S3ObjectHistoryPruned {
				bucket: BucketId(hash(x.bucket)),
				key: x.key,
				through_version: x.through_version,
				removed: x.removed,
			}
		},
		_ => {
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedRuntime,
				"unknown native storage/provider event",
			))
		},
	})
}
fn provider_status(v: ProviderStatusWire) -> ProviderStatus {
	match v {
		ProviderStatusWire::Active => ProviderStatus::Active,
		ProviderStatusWire::Suspended => ProviderStatus::Suspended,
	}
}
fn agreement_status(v: AgreementStatusWire) -> AgreementStatus {
	match v {
		AgreementStatusWire::Proposed => AgreementStatus::Proposed,
		AgreementStatusWire::Active => AgreementStatus::Active,
		AgreementStatusWire::Suspended => AgreementStatus::Suspended,
		AgreementStatusWire::Cancelled => AgreementStatus::Cancelled,
		AgreementStatusWire::Expired => AgreementStatus::Expired,
	}
}
fn bucket_role(v: BucketRoleWire) -> BucketRole {
	match v {
		BucketRoleWire::Reader => BucketRole::Reader,
		BucketRoleWire::Writer => BucketRole::Writer,
		BucketRoleWire::Admin => BucketRole::Admin,
	}
}
fn drive_role(v: DriveRoleWire) -> DriveRole {
	match v {
		DriveRoleWire::Reader => DriveRole::Reader,
		DriveRoleWire::Writer => DriveRole::Writer,
		DriveRoleWire::Admin => DriveRole::Admin,
	}
}
fn commitment_state(v: CommitmentStateWire) -> CommitmentState {
	match v {
		CommitmentStateWire::Publishable => CommitmentState::Publishable,
		CommitmentStateWire::Pending => CommitmentState::Pending,
		CommitmentStateWire::Tombstoned => CommitmentState::Tombstoned,
		CommitmentStateWire::Missing => CommitmentState::Missing,
	}
}
fn drive_node_kind(v: DriveNodeKindWire) -> DriveNodeKind {
	match v {
		DriveNodeKindWire::Directory => DriveNodeKind::Directory,
		DriveNodeKindWire::File => DriveNodeKind::File,
	}
}
fn accounts(
	v: Vec<A>,
	minimum: usize,
	maximum: usize,
	sorted: bool,
	excluded: Option<&A>,
) -> DomainResult<Vec<AccountId>> {
	if !(minimum..=maximum).contains(&v.len()) {
		return Err(invalid_event("storage event account count violates native bounds"));
	}
	if excluded.is_some_and(|account| v.iter().any(|candidate| candidate.0 == account.0)) {
		return Err(invalid_event("storage bucket replicas contain the primary provider"));
	}
	if sorted && v.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
		return Err(invalid_event("storage event accounts are not in canonical order"));
	}
	let decoded = v.iter().map(account).collect::<DomainResult<Vec<_>>>()?;
	let unique = decoded.iter().collect::<std::collections::HashSet<_>>();
	if unique.len() != decoded.len() {
		return Err(NativeError::new(
			NativeErrorCode::InvalidInput,
			"storage event contains duplicate accounts",
		));
	}
	Ok(decoded)
}
fn checkpoint_commitment(value: CommitmentWire) -> DomainResult<CheckpointCommitment> {
	if value.leaf_count == 0 || value.start_seq.checked_add(value.leaf_count).is_none() {
		return Err(invalid_event("storage checkpoint sequence range is invalid"));
	}
	Ok(CheckpointCommitment {
		mmr_root: ContentCommitment(hash(value.mmr_root)),
		start_seq: value.start_seq,
		leaf_count: value.leaf_count,
	})
}
fn validate_drive_path(path: &[u8]) -> DomainResult<()> {
	if path.is_empty() || path.len() > 4_096 || path[0] != b'/' {
		return Err(invalid_event("storage Drive path violates native bounds"));
	}
	if path == b"/" {
		return Ok(())
	}
	if path.last() == Some(&b'/') {
		return Err(invalid_event("storage Drive path has a trailing separator"));
	}
	let components = path[1..].split(|byte| *byte == b'/').collect::<Vec<_>>();
	if components.len() > 64 {
		return Err(invalid_event("storage Drive path exceeds native depth"));
	}
	for component in components {
		if component.is_empty()
			|| component.len() > 256
			|| component.contains(&0)
			|| component == b"."
			|| component == b".."
		{
			return Err(invalid_event("storage Drive path contains an invalid component"));
		}
		let text = core::str::from_utf8(component)
			.map_err(|_| invalid_event("storage Drive path is not UTF-8"))?;
		if !text.nfc().eq(text.chars()) {
			return Err(invalid_event("storage Drive path is not NFC"));
		}
	}
	Ok(())
}
fn validate_s3_bucket_name(name: &[u8]) -> DomainResult<()> {
	let alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
	if !(3..=63).contains(&name.len())
		|| !alphanumeric(name[0])
		|| !alphanumeric(name[name.len() - 1])
		|| name.iter().any(|byte| !alphanumeric(*byte) && *byte != b'-')
	{
		return Err(invalid_event("storage S3 bucket name violates native bounds"));
	}
	Ok(())
}
fn validate_s3_object_key(key: &[u8]) -> DomainResult<()> {
	if key.is_empty() || key.len() > 1_024 || key.contains(&0) {
		return Err(invalid_event("storage S3 object key violates native bounds"));
	}
	Ok(())
}
fn invalid_event(message: &'static str) -> NativeError {
	NativeError::new(NativeErrorCode::InvalidInput, message)
}
fn hash(v: H) -> Hash32 {
	Hash32::from_bytes(*v.as_fixed_bytes())
}
fn domain_hash(v: H) -> Hash32 {
	hash(v)
}
fn account(v: &A) -> DomainResult<AccountId> {
	AccountId::new(account_id_to_ss58(&account_id_from_subxt(v)))
}
fn runtime_hash(v: &Hash32) -> DomainResult<H> {
	let b = hex::decode(&v.as_str()[2..])
		.map_err(|_| NativeError::new(NativeErrorCode::InvalidInput, "invalid finalized hash"))?;
	Ok(H::from_slice(&b))
}
fn subscription_error(e: impl core::fmt::Display) -> NativeError {
	NativeError::new(NativeErrorCode::ContentUnavailable, e.to_string()).retryable()
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn curated_surface_has_37_exact_unique_variants_and_kinds() {
		assert_eq!(
			CURATED_EVENT_VARIANTS,
			[
				("StorageProvider", "ProviderRegistered"),
				("StorageProvider", "ProviderUpdated"),
				("StorageProvider", "ProviderStatusChanged"),
				("StorageProvider", "ProviderRemoved"),
				("StorageProvider", "Heartbeat"),
				("StorageProvider", "BucketCreated"),
				("StorageProvider", "BucketGrantChanged"),
				("StorageProvider", "AgreementTransitioned"),
				("StorageProvider", "AgreementCapacityReleased"),
				("StorageProvider", "AgreementProviderRebound"),
				("StorageProvider", "ChallengeIssued"),
				("StorageProvider", "ChallengeProved"),
				("StorageProvider", "ChallengeTimedOut"),
				("StorageProvider", "CheckpointAccepted"),
				("StorageProvider", "CheckpointEquivocation"),
				("StorageProvider", "ReplicaSelected"),
				("StorageProvider", "PrimaryPromoted"),
				("StorageProvider", "BucketReplicaReplaced"),
				("StorageProvider", "ManifestCommitmentChanged"),
				("StorageProvider", "ManifestDeletionAcknowledged"),
				("Drive", "DriveCreated"),
				("Drive", "DriveRootUpdated"),
				("Drive", "GrantChanged"),
				("Drive", "DriveTransferred"),
				("Drive", "DriveArchived"),
				("Drive", "NodeWritten"),
				("Drive", "NodeRemoved"),
				("S3", "BucketCreated"),
				("S3", "ControllerChanged"),
				("S3", "BucketTransferred"),
				("S3", "BucketArchived"),
				("S3", "BucketVersioningChanged"),
				("S3", "ObjectPut"),
				("S3", "ObjectDeleted"),
				("S3", "ObjectPurged"),
				("S3", "BucketDeleted"),
				("S3", "ObjectHistoryPruned")
			]
		);
		assert_eq!(
			CURATED_EVENT_VARIANTS.iter().collect::<std::collections::HashSet<_>>().len(),
			37
		);
		assert_eq!(
			ALL_STORAGE_NATIVE_EVENT_KINDS
				.iter()
				.collect::<std::collections::HashSet<_>>()
				.len(),
			37
		);
	}
	#[test]
	fn known_unexposed_are_ignored_but_future_events_are_not() {
		assert!(is_known_unexposed("StorageProvider", "ServiceKeyRotated"));
		assert!(!is_known_unexposed("StorageProvider", "FutureEvent"));
		assert!(!is_known_unexposed("Drive", "ServiceKeyRotated"));
	}
	#[test]
	fn duplicate_replica_accounts_fail_closed() {
		assert!(accounts(vec![A::from([1; 32]), A::from([1; 32])], 2, 4, false, None).is_err());
	}
	#[test]
	fn native_storage_bounds_fail_closed() {
		let primary = A::from([1; 32]);
		assert!(accounts(vec![A::from([2; 32])], 2, 4, false, Some(&primary)).is_err());
		assert!(accounts(
			vec![A::from([1; 32]), A::from([2; 32])],
			2,
			4,
			false,
			Some(&primary)
		)
		.is_err());
		assert!(accounts(
			vec![A::from([3; 32]), A::from([2; 32])],
			2,
			2,
			true,
			None
		)
		.is_err());
		assert!(checkpoint_commitment(CommitmentWire {
			mmr_root: H::from([1; 32]),
			start_seq: u64::MAX,
			leaf_count: 1,
		})
		.is_err());
		assert!(checkpoint_commitment(CommitmentWire {
			mmr_root: H::from([1; 32]),
			start_seq: 0,
			leaf_count: 0,
		})
		.is_err());
		assert!(validate_drive_path(b"/dir/file").is_ok());
		assert!(validate_drive_path(b"dir/file").is_err());
		assert!(validate_drive_path(b"/dir/..").is_err());
		assert!(validate_drive_path("/e\u{301}".as_bytes()).is_err());
		assert!(validate_s3_bucket_name(b"app-data").is_ok());
		assert!(validate_s3_bucket_name(b"App").is_err());
		assert!(validate_s3_object_key(&[0xff]).is_ok());
		assert!(validate_s3_object_key(&[0]).is_err());
	}
	#[test]
	fn s3_key_outcome_ids_do_not_collapse_within_a_bucket() {
		let bucket = BucketId(Hash32::from_bytes([9; 32]));
		let first = StorageNativeEvent::S3ObjectPurged { bucket: bucket.clone(), key: vec![0xff] };
		let second = StorageNativeEvent::S3ObjectPurged { bucket, key: vec![0xfe] };
		assert_ne!(first.outcome(), second.outcome());
	}
}
