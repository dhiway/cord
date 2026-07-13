//! Executable finalized native storage-provider, Drive and S3 event subscription.

use super::{
	domains::{
		storage_events::{
			FinalizedStorageNativeEvent, FinalizedStorageNativeOutcome, StorageNativeEvent,
			StorageNativeEventKind, StorageNativeEventSubscription,
		},
		storage_provider::ProviderStatus,
		AccountId, AgreementId, BucketId, ChallengeId, ContentCommitment, DomainResult, DriveId,
		Hash32, ObjectId, ProofCommitment,
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
macro_rules! wire {($name:ident,$pallet:literal,$event:literal,{$($field:ident:$ty:ty),*$(,)?})=>{
	#[derive(DecodeAsType)] struct $name{$($field:$ty,)*}
	impl StaticEvent for $name{const PALLET:&'static str=$pallet;const EVENT:&'static str=$event;}
}}
wire!(ProviderRegisteredWire,"StorageProvider","ProviderRegistered",{provider:A,capacity_bytes:u64});
wire!(ProviderUpdatedWire,"StorageProvider","ProviderUpdated",{provider:A,capacity_bytes:u64});
wire!(ProviderStatusChangedWire,"StorageProvider","ProviderStatusChanged",{provider:A,status:ProviderStatusWire});
wire!(ProviderRemovedWire,"StorageProvider","ProviderRemoved",{provider:A});
wire!(HeartbeatWire,"StorageProvider","Heartbeat",{provider:A,at:u32});
wire!(AgreementProposedWire,"StorageProvider","AgreementProposed",{agreement_id:H,owner:A,provider:A});
wire!(AgreementAcceptedWire,"StorageProvider","AgreementAccepted",{agreement_id:H});
wire!(AgreementCancelledWire,"StorageProvider","AgreementCancelled",{agreement_id:H});
wire!(AgreementRenewalRequestedWire,"StorageProvider","AgreementRenewalRequested",{agreement_id:H,expires_at:u32});
wire!(AgreementRenewedWire,"StorageProvider","AgreementRenewed",{agreement_id:H,expires_at:u32});
wire!(AgreementExpiredWire,"StorageProvider","AgreementExpired",{agreement_id:H});
wire!(AgreementPrunedWire,"StorageProvider","AgreementPruned",{agreement_id:H});
wire!(ChallengeIssuedWire,"StorageProvider","ChallengeIssued",{challenge_id:H,provider:A,due_at:u32});
wire!(CheckpointSubmittedWire,"StorageProvider","CheckpointSubmitted",{challenge_id:H,proof_commitment:H});
wire!(ChallengeTimedOutWire,"StorageProvider","ChallengeTimedOut",{challenge_id:H,provider:A});
wire!(ProviderRootCommittedWire,"StorageProvider","ProviderRootCommitted",{provider:A,sequence:u64,root:H,leaf_count:u64});
wire!(DeletionAcknowledgedWire,"StorageProvider","DeletionAcknowledged",{agreement_id:H,provider:A,content_commitment:H,tombstone_root:H,root_sequence:u64,leaf_index:u64,leaf_count:u64,proof_commitment:H});
wire!(DriveCreatedWire,"Drive","DriveCreated",{drive_id:H,owner:A});
wire!(DriveRootUpdatedWire,"Drive","DriveRootUpdated",{drive_id:H,version:u64});
wire!(DriveControllerChangedWire,"Drive","ControllerChanged",{drive_id:H,controller:A,enabled:bool});
wire!(DriveTransferredWire,"Drive","DriveTransferred",{drive_id:H,old_owner:A,new_owner:A});
wire!(DriveArchivedWire,"Drive","DriveArchived",{drive_id:H});
wire!(BucketCreatedWire,"S3","BucketCreated",{bucket:H,name:Vec<u8>,owner:A});
wire!(BucketControllerChangedWire,"S3","ControllerChanged",{bucket:H,controller:A,enabled:bool,version:u64});
wire!(BucketTransferredWire,"S3","BucketTransferred",{bucket:H,from:A,to:A,version:u64});
wire!(BucketArchivedWire,"S3","BucketArchived",{bucket:H,archived:bool,version:u64});
wire!(BucketVersioningChangedWire,"S3","BucketVersioningChanged",{bucket:H,enabled:bool,version:u64});
wire!(ObjectPutWire,"S3","ObjectPut",{bucket:H,object:H,key:Vec<u8>,content_hash:[u8;32],version:u64});
wire!(ObjectDeletedWire,"S3","ObjectDeleted",{bucket:H,object:H,key:Vec<u8>,version:u64});
wire!(BucketDeletedWire,"S3","BucketDeleted",{bucket:H,name:Vec<u8>,owner:A});

enum Wire {
	ProviderRegistered(ProviderRegisteredWire),
	ProviderUpdated(ProviderUpdatedWire),
	ProviderStatusChanged(ProviderStatusChangedWire),
	ProviderRemoved(ProviderRemovedWire),
	Heartbeat(HeartbeatWire),
	AgreementProposed(AgreementProposedWire),
	AgreementAccepted(AgreementAcceptedWire),
	AgreementCancelled(AgreementCancelledWire),
	AgreementRenewalRequested(AgreementRenewalRequestedWire),
	AgreementRenewed(AgreementRenewedWire),
	AgreementExpired(AgreementExpiredWire),
	AgreementPruned(AgreementPrunedWire),
	ChallengeIssued(ChallengeIssuedWire),
	CheckpointSubmitted(CheckpointSubmittedWire),
	ChallengeTimedOut(ChallengeTimedOutWire),
	ProviderRootCommitted(ProviderRootCommittedWire),
	DeletionAcknowledged(DeletionAcknowledgedWire),
	DriveCreated(DriveCreatedWire),
	DriveRootUpdated(DriveRootUpdatedWire),
	DriveControllerChanged(DriveControllerChangedWire),
	DriveTransferred(DriveTransferredWire),
	DriveArchived(DriveArchivedWire),
	BucketCreated(BucketCreatedWire),
	BucketControllerChanged(BucketControllerChangedWire),
	BucketTransferred(BucketTransferredWire),
	BucketArchived(BucketArchivedWire),
	BucketVersioningChanged(BucketVersioningChangedWire),
	ObjectPut(ObjectPutWire),
	ObjectDeleted(ObjectDeletedWire),
	BucketDeleted(BucketDeletedWire),
}

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
					});
				}
			}
		}
	}
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
	let w = match (d.pallet_name(), d.variant_name()) {
		("StorageProvider", "ProviderRegistered") => {
			Wire::ProviderRegistered(dec!(ProviderRegisteredWire))
		},
		("StorageProvider", "ProviderUpdated") => Wire::ProviderUpdated(dec!(ProviderUpdatedWire)),
		("StorageProvider", "ProviderStatusChanged") => {
			Wire::ProviderStatusChanged(dec!(ProviderStatusChangedWire))
		},
		("StorageProvider", "ProviderRemoved") => Wire::ProviderRemoved(dec!(ProviderRemovedWire)),
		("StorageProvider", "Heartbeat") => Wire::Heartbeat(dec!(HeartbeatWire)),
		("StorageProvider", "AgreementProposed") => {
			Wire::AgreementProposed(dec!(AgreementProposedWire))
		},
		("StorageProvider", "AgreementAccepted") => {
			Wire::AgreementAccepted(dec!(AgreementAcceptedWire))
		},
		("StorageProvider", "AgreementCancelled") => {
			Wire::AgreementCancelled(dec!(AgreementCancelledWire))
		},
		("StorageProvider", "AgreementRenewalRequested") => {
			Wire::AgreementRenewalRequested(dec!(AgreementRenewalRequestedWire))
		},
		("StorageProvider", "AgreementRenewed") => {
			Wire::AgreementRenewed(dec!(AgreementRenewedWire))
		},
		("StorageProvider", "AgreementExpired") => {
			Wire::AgreementExpired(dec!(AgreementExpiredWire))
		},
		("StorageProvider", "AgreementPruned") => Wire::AgreementPruned(dec!(AgreementPrunedWire)),
		("StorageProvider", "ChallengeIssued") => Wire::ChallengeIssued(dec!(ChallengeIssuedWire)),
		("StorageProvider", "CheckpointSubmitted") => {
			Wire::CheckpointSubmitted(dec!(CheckpointSubmittedWire))
		},
		("StorageProvider", "ChallengeTimedOut") => {
			Wire::ChallengeTimedOut(dec!(ChallengeTimedOutWire))
		},
		("StorageProvider", "ProviderRootCommitted") => {
			Wire::ProviderRootCommitted(dec!(ProviderRootCommittedWire))
		},
		("StorageProvider", "DeletionAcknowledged") => {
			Wire::DeletionAcknowledged(dec!(DeletionAcknowledgedWire))
		},
		("Drive", "DriveCreated") => Wire::DriveCreated(dec!(DriveCreatedWire)),
		("Drive", "DriveRootUpdated") => Wire::DriveRootUpdated(dec!(DriveRootUpdatedWire)),
		("Drive", "ControllerChanged") => {
			Wire::DriveControllerChanged(dec!(DriveControllerChangedWire))
		},
		("Drive", "DriveTransferred") => Wire::DriveTransferred(dec!(DriveTransferredWire)),
		("Drive", "DriveArchived") => Wire::DriveArchived(dec!(DriveArchivedWire)),
		("S3", "BucketCreated") => Wire::BucketCreated(dec!(BucketCreatedWire)),
		("S3", "ControllerChanged") => {
			Wire::BucketControllerChanged(dec!(BucketControllerChangedWire))
		},
		("S3", "BucketTransferred") => Wire::BucketTransferred(dec!(BucketTransferredWire)),
		("S3", "BucketArchived") => Wire::BucketArchived(dec!(BucketArchivedWire)),
		("S3", "BucketVersioningChanged") => {
			Wire::BucketVersioningChanged(dec!(BucketVersioningChangedWire))
		},
		("S3", "ObjectPut") => Wire::ObjectPut(dec!(ObjectPutWire)),
		("S3", "ObjectDeleted") => Wire::ObjectDeleted(dec!(ObjectDeletedWire)),
		("S3", "BucketDeleted") => Wire::BucketDeleted(dec!(BucketDeletedWire)),
		_ => {
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedRuntime,
				"unknown native storage/provider event",
			))
		},
	};
	decode_wire(w)
}
fn decode_wire(w: Wire) -> DomainResult<StorageNativeEvent> {
	Ok(match w {
		Wire::ProviderRegistered(x) => StorageNativeEvent::ProviderRegistered {
			provider: account(&x.provider)?,
			capacity_bytes: x.capacity_bytes,
		},
		Wire::ProviderUpdated(x) => StorageNativeEvent::ProviderUpdated {
			provider: account(&x.provider)?,
			capacity_bytes: x.capacity_bytes,
		},
		Wire::ProviderStatusChanged(x) => StorageNativeEvent::ProviderStatusChanged {
			provider: account(&x.provider)?,
			status: match x.status {
				ProviderStatusWire::Active => ProviderStatus::Active,
				ProviderStatusWire::Suspended => ProviderStatus::Suspended,
			},
		},
		Wire::ProviderRemoved(x) => {
			StorageNativeEvent::ProviderRemoved { provider: account(&x.provider)? }
		},
		Wire::Heartbeat(x) => {
			StorageNativeEvent::Heartbeat { provider: account(&x.provider)?, at: x.at }
		},
		Wire::AgreementProposed(x) => StorageNativeEvent::AgreementProposed {
			agreement: AgreementId(hash(x.agreement_id)),
			owner: account(&x.owner)?,
			provider: account(&x.provider)?,
		},
		Wire::AgreementAccepted(x) => {
			StorageNativeEvent::AgreementAccepted { agreement: AgreementId(hash(x.agreement_id)) }
		},
		Wire::AgreementCancelled(x) => {
			StorageNativeEvent::AgreementCancelled { agreement: AgreementId(hash(x.agreement_id)) }
		},
		Wire::AgreementRenewalRequested(x) => StorageNativeEvent::AgreementRenewalRequested {
			agreement: AgreementId(hash(x.agreement_id)),
			expires_at: x.expires_at,
		},
		Wire::AgreementRenewed(x) => StorageNativeEvent::AgreementRenewed {
			agreement: AgreementId(hash(x.agreement_id)),
			expires_at: x.expires_at,
		},
		Wire::AgreementExpired(x) => {
			StorageNativeEvent::AgreementExpired { agreement: AgreementId(hash(x.agreement_id)) }
		},
		Wire::AgreementPruned(x) => {
			StorageNativeEvent::AgreementPruned { agreement: AgreementId(hash(x.agreement_id)) }
		},
		Wire::ChallengeIssued(x) => StorageNativeEvent::ChallengeIssued {
			challenge: ChallengeId(hash(x.challenge_id)),
			provider: account(&x.provider)?,
			due_at: x.due_at,
		},
		Wire::CheckpointSubmitted(x) => StorageNativeEvent::CheckpointSubmitted {
			challenge: ChallengeId(hash(x.challenge_id)),
			proof_commitment: ProofCommitment(hash(x.proof_commitment)),
		},
		Wire::ChallengeTimedOut(x) => StorageNativeEvent::ChallengeTimedOut {
			challenge: ChallengeId(hash(x.challenge_id)),
			provider: account(&x.provider)?,
		},
		Wire::ProviderRootCommitted(x) => StorageNativeEvent::ProviderRootCommitted {
			provider: account(&x.provider)?,
			sequence: x.sequence,
			root: ProofCommitment(hash(x.root)),
			leaf_count: x.leaf_count,
		},
		Wire::DeletionAcknowledged(x) => StorageNativeEvent::DeletionAcknowledged {
			agreement: AgreementId(hash(x.agreement_id)),
			provider: account(&x.provider)?,
			content_commitment: ContentCommitment(hash(x.content_commitment)),
			tombstone_root: ProofCommitment(hash(x.tombstone_root)),
			root_sequence: x.root_sequence,
			leaf_index: x.leaf_index,
			leaf_count: x.leaf_count,
			proof_commitment: ProofCommitment(hash(x.proof_commitment)),
		},
		Wire::DriveCreated(x) => StorageNativeEvent::DriveCreated {
			drive: DriveId(hash(x.drive_id)),
			owner: account(&x.owner)?,
		},
		Wire::DriveRootUpdated(x) => StorageNativeEvent::DriveRootUpdated {
			drive: DriveId(hash(x.drive_id)),
			version: x.version,
		},
		Wire::DriveControllerChanged(x) => StorageNativeEvent::DriveControllerChanged {
			drive: DriveId(hash(x.drive_id)),
			controller: account(&x.controller)?,
			enabled: x.enabled,
		},
		Wire::DriveTransferred(x) => StorageNativeEvent::DriveTransferred {
			drive: DriveId(hash(x.drive_id)),
			old_owner: account(&x.old_owner)?,
			new_owner: account(&x.new_owner)?,
		},
		Wire::DriveArchived(x) => {
			StorageNativeEvent::DriveArchived { drive: DriveId(hash(x.drive_id)) }
		},
		Wire::BucketCreated(x) => StorageNativeEvent::BucketCreated {
			bucket: BucketId(hash(x.bucket)),
			name: x.name,
			owner: account(&x.owner)?,
		},
		Wire::BucketControllerChanged(x) => StorageNativeEvent::BucketControllerChanged {
			bucket: BucketId(hash(x.bucket)),
			controller: account(&x.controller)?,
			enabled: x.enabled,
			version: x.version,
		},
		Wire::BucketTransferred(x) => StorageNativeEvent::BucketTransferred {
			bucket: BucketId(hash(x.bucket)),
			from: account(&x.from)?,
			to: account(&x.to)?,
			version: x.version,
		},
		Wire::BucketArchived(x) => StorageNativeEvent::BucketArchived {
			bucket: BucketId(hash(x.bucket)),
			archived: x.archived,
			version: x.version,
		},
		Wire::BucketVersioningChanged(x) => StorageNativeEvent::BucketVersioningChanged {
			bucket: BucketId(hash(x.bucket)),
			enabled: x.enabled,
			version: x.version,
		},
		Wire::ObjectPut(x) => StorageNativeEvent::ObjectPut {
			bucket: BucketId(hash(x.bucket)),
			object: ObjectId(hash(x.object)),
			key: x.key,
			content_commitment: ContentCommitment(Hash32::from_bytes(x.content_hash)),
			version: x.version,
		},
		Wire::ObjectDeleted(x) => StorageNativeEvent::ObjectDeleted {
			bucket: BucketId(hash(x.bucket)),
			object: ObjectId(hash(x.object)),
			key: x.key,
			version: x.version,
		},
		Wire::BucketDeleted(x) => StorageNativeEvent::BucketDeleted {
			bucket: BucketId(hash(x.bucket)),
			name: x.name,
			owner: account(&x.owner)?,
		},
	})
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
	fn h(n: u8) -> H {
		H::from([n; 32])
	}
	fn a(n: u8) -> A {
		A::from([n; 32])
	}
	#[test]
	fn every_native_variant_decodes_to_a_distinct_kind_and_outcome() {
		let wires = vec![
			Wire::ProviderRegistered(ProviderRegisteredWire { provider: a(1), capacity_bytes: 2 }),
			Wire::ProviderUpdated(ProviderUpdatedWire { provider: a(1), capacity_bytes: 3 }),
			Wire::ProviderStatusChanged(ProviderStatusChangedWire {
				provider: a(1),
				status: ProviderStatusWire::Active,
			}),
			Wire::ProviderRemoved(ProviderRemovedWire { provider: a(1) }),
			Wire::Heartbeat(HeartbeatWire { provider: a(1), at: 2 }),
			Wire::AgreementProposed(AgreementProposedWire {
				agreement_id: h(2),
				owner: a(2),
				provider: a(1),
			}),
			Wire::AgreementAccepted(AgreementAcceptedWire { agreement_id: h(2) }),
			Wire::AgreementCancelled(AgreementCancelledWire { agreement_id: h(2) }),
			Wire::AgreementRenewalRequested(AgreementRenewalRequestedWire {
				agreement_id: h(2),
				expires_at: 3,
			}),
			Wire::AgreementRenewed(AgreementRenewedWire { agreement_id: h(2), expires_at: 4 }),
			Wire::AgreementExpired(AgreementExpiredWire { agreement_id: h(2) }),
			Wire::AgreementPruned(AgreementPrunedWire { agreement_id: h(2) }),
			Wire::ChallengeIssued(ChallengeIssuedWire {
				challenge_id: h(3),
				provider: a(1),
				due_at: 4,
			}),
			Wire::CheckpointSubmitted(CheckpointSubmittedWire {
				challenge_id: h(3),
				proof_commitment: h(4),
			}),
			Wire::ChallengeTimedOut(ChallengeTimedOutWire { challenge_id: h(3), provider: a(1) }),
			Wire::ProviderRootCommitted(ProviderRootCommittedWire {
				provider: a(1),
				sequence: 2,
				root: h(4),
				leaf_count: 2,
			}),
			Wire::DeletionAcknowledged(DeletionAcknowledgedWire {
				agreement_id: h(2),
				provider: a(1),
				content_commitment: h(5),
				tombstone_root: h(6),
				root_sequence: 2,
				leaf_index: 1,
				leaf_count: 2,
				proof_commitment: h(7),
			}),
			Wire::DriveCreated(DriveCreatedWire { drive_id: h(8), owner: a(2) }),
			Wire::DriveRootUpdated(DriveRootUpdatedWire { drive_id: h(8), version: 2 }),
			Wire::DriveControllerChanged(DriveControllerChangedWire {
				drive_id: h(8),
				controller: a(3),
				enabled: true,
			}),
			Wire::DriveTransferred(DriveTransferredWire {
				drive_id: h(8),
				old_owner: a(2),
				new_owner: a(3),
			}),
			Wire::DriveArchived(DriveArchivedWire { drive_id: h(8) }),
			Wire::BucketCreated(BucketCreatedWire {
				bucket: h(9),
				name: b"b".to_vec(),
				owner: a(2),
			}),
			Wire::BucketControllerChanged(BucketControllerChangedWire {
				bucket: h(9),
				controller: a(3),
				enabled: true,
				version: 2,
			}),
			Wire::BucketTransferred(BucketTransferredWire {
				bucket: h(9),
				from: a(2),
				to: a(3),
				version: 3,
			}),
			Wire::BucketArchived(BucketArchivedWire { bucket: h(9), archived: true, version: 4 }),
			Wire::BucketVersioningChanged(BucketVersioningChangedWire {
				bucket: h(9),
				enabled: true,
				version: 5,
			}),
			Wire::ObjectPut(ObjectPutWire {
				bucket: h(9),
				object: h(10),
				key: b"k".to_vec(),
				content_hash: [11; 32],
				version: 1,
			}),
			Wire::ObjectDeleted(ObjectDeletedWire {
				bucket: h(9),
				object: h(10),
				key: b"k".to_vec(),
				version: 2,
			}),
			Wire::BucketDeleted(BucketDeletedWire {
				bucket: h(9),
				name: b"b".to_vec(),
				owner: a(3),
			}),
		];
		let mut kinds = std::collections::HashSet::new();
		for w in wires {
			let event = decode_wire(w).unwrap();
			assert!(kinds.insert(event.kind()));
			let _ = event.outcome();
		}
		assert_eq!(kinds.len(), 30);
	}
}
