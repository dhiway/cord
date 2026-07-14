//! Executable finalized native-attestation event subscription.

use std::{collections::VecDeque, pin::Pin};

use futures::{Stream, StreamExt};
use pallet_orbis_attestation_runtime_api as att_api;
use scale_decode::DecodeAsType;
use subxt::{blocks::Block, events::StaticEvent, OnlineClient};

use super::{
	domains::attestation::{
		AttestationEvent, AttestationEventKind, AttestationEventSubscription,
		FinalizedAttestationEvent, FinalizedAttestationOutcome, IndexPolicy, SchemaStatus,
	},
	domains::{
		AccountId, AttestationId, DomainResult, Hash32, SchemaId, StatusCommitment,
		SubjectCommitment,
	},
	NativeError, NativeErrorCode,
};
use crate::{
	config::OrbisConfig,
	types::account::{account_id_from_subxt, account_id_to_ss58},
};

type RuntimeAccountId = subxt::utils::AccountId32;
type RuntimeHash = subxt::utils::H256;
type RuntimeBlock = Block<OrbisConfig, OnlineClient<OrbisConfig>>;
type FinalizedBlockStream =
	Pin<Box<dyn Stream<Item = Result<RuntimeBlock, subxt::Error>> + Send + 'static>>;

macro_rules! static_event {
	($type:ty, $event:literal) => {
		impl StaticEvent for $type {
			const PALLET: &'static str = "Attestation";
			const EVENT: &'static str = $event;
		}
	};
}

#[derive(DecodeAsType)]
struct SchemaCreatedWire {
	schema: RuntimeHash,
	creator: RuntimeAccountId,
	definition_commitment: RuntimeHash,
	revocable: bool,
	unique: bool,
	index_policy: att_api::IndexPolicy,
}
static_event!(SchemaCreatedWire, "SchemaCreated");

#[derive(DecodeAsType)]
struct SchemaStatusChangedWire {
	schema: RuntimeHash,
	status: att_api::SchemaStatus,
	forced: bool,
}
static_event!(SchemaStatusChangedWire, "SchemaStatusChanged");

#[derive(DecodeAsType)]
struct AttestationIssuedWire {
	attestation: RuntimeHash,
	schema: RuntimeHash,
	issuer: RuntimeAccountId,
	subject_commitment: RuntimeHash,
}
static_event!(AttestationIssuedWire, "AttestationIssued");

#[derive(DecodeAsType)]
struct DelegatedIntentConsumedWire {
	issuer: RuntimeAccountId,
	delegate: RuntimeAccountId,
	nonce: u64,
	attestation: RuntimeHash,
}
static_event!(DelegatedIntentConsumedWire, "DelegatedIntentConsumed");

#[derive(DecodeAsType)]
struct DelegatedRevocationConsumedWire {
	revoker: RuntimeAccountId,
	delegate: RuntimeAccountId,
	nonce: u64,
	attestation: RuntimeHash,
}
static_event!(DelegatedRevocationConsumedWire, "DelegatedRevocationConsumed");

#[derive(DecodeAsType)]
struct AttestationRevokedWire {
	attestation: RuntimeHash,
	by: Option<RuntimeAccountId>,
	forced: bool,
}
static_event!(AttestationRevokedWire, "AttestationRevoked");

#[derive(DecodeAsType)]
struct ExternalStatusRevokedWire {
	key: RuntimeHash,
	issuer: RuntimeAccountId,
	status_commitment: RuntimeHash,
	revoked_at: u32,
}
static_event!(ExternalStatusRevokedWire, "ExternalStatusRevoked");

#[derive(DecodeAsType)]
struct EmergencyPauseChangedWire {
	paused: bool,
}
static_event!(EmergencyPauseChangedWire, "EmergencyPauseChanged");

enum AttestationEventWire {
	SchemaCreated(SchemaCreatedWire),
	SchemaStatusChanged(SchemaStatusChangedWire),
	AttestationIssued(AttestationIssuedWire),
	DelegatedIntentConsumed(DelegatedIntentConsumedWire),
	DelegatedRevocationConsumed(DelegatedRevocationConsumedWire),
	AttestationRevoked(AttestationRevokedWire),
	ExternalStatusRevoked(ExternalStatusRevokedWire),
	EmergencyPauseChanged(EmergencyPauseChangedWire),
}

/// A live finalized subscription that decodes only native `Attestation` pallet events.
pub struct OrbisAttestationEventSubscription {
	blocks: FinalizedBlockStream,
	kinds: Vec<AttestationEventKind>,
	pending: VecDeque<FinalizedAttestationOutcome>,
}

impl OrbisAttestationEventSubscription {
	/// Anchor at the current finalized block and subscribe to subsequent finalized blocks.
	/// Historical replay/reconnect is intentionally deferred to P6/P7.
	pub async fn subscribe(
		client: OnlineClient<OrbisConfig>,
		subscription: AttestationEventSubscription,
	) -> DomainResult<Self> {
		let latest = client.blocks().at_latest().await.map_err(subscription_error)?;
		let requested = runtime_hash(&subscription.from_finalized_block)?;
		if latest.hash() != requested {
			return Err(NativeError::new(
				NativeErrorCode::InconsistentSnapshot,
				"attestation subscription anchor must equal the current finalized block",
			));
		}
		let blocks = client.blocks().subscribe_finalized().await.map_err(subscription_error)?;
		Ok(Self { blocks: Box::pin(blocks), kinds: subscription.kinds, pending: VecDeque::new() })
	}

	/// Yield the next filtered finalized event together with its stable semantic outcome.
	pub async fn next(&mut self) -> DomainResult<Option<FinalizedAttestationOutcome>> {
		loop {
			if let Some(item) = self.pending.pop_front() {
				return Ok(Some(item));
			}
			let Some(block) = self.blocks.next().await else {
				return Ok(None);
			};
			let block = block.map_err(subscription_error)?;
			let finalized_block_hash = domain_hash(block.hash());
			let events = block.events().await.map_err(subscription_error)?;
			for details in events.iter() {
				let details = details.map_err(subscription_error)?;
				if details.pallet_name() != "Attestation" {
					continue;
				}
				let event = decode_event(&details)?;
				enqueue_if_selected(
					&mut self.pending,
					&self.kinds,
					finalized_block_hash.clone(),
					details.index(),
					event,
				);
			}
		}
	}
}

fn enqueue_if_selected(
	pending: &mut VecDeque<FinalizedAttestationOutcome>,
	kinds: &[AttestationEventKind],
	finalized_block_hash: Hash32,
	event_index: u32,
	event: AttestationEvent,
) {
	if kinds.contains(&event.kind()) {
		let outcome = event.outcome();
		pending.push_back(FinalizedAttestationOutcome {
			event: FinalizedAttestationEvent { finalized_block_hash, event_index, event },
			outcome,
		});
	}
}

fn decode_event(
	details: &subxt::events::EventDetails<OrbisConfig>,
) -> DomainResult<AttestationEvent> {
	macro_rules! decode {
		($wire:ty) => {
			details.as_event::<$wire>().map_err(subscription_error)?.ok_or_else(|| {
				NativeError::new(
					NativeErrorCode::UnsupportedRuntime,
					"attestation event metadata mismatch",
				)
			})?
		};
	}
	let wire = match details.variant_name() {
		"SchemaCreated" => AttestationEventWire::SchemaCreated(decode!(SchemaCreatedWire)),
		"SchemaStatusChanged" => {
			AttestationEventWire::SchemaStatusChanged(decode!(SchemaStatusChangedWire))
		},
		"AttestationIssued" => {
			AttestationEventWire::AttestationIssued(decode!(AttestationIssuedWire))
		},
		"DelegatedIntentConsumed" => {
			AttestationEventWire::DelegatedIntentConsumed(decode!(DelegatedIntentConsumedWire))
		},
		"DelegatedRevocationConsumed" => AttestationEventWire::DelegatedRevocationConsumed(
			decode!(DelegatedRevocationConsumedWire),
		),
		"AttestationRevoked" => {
			AttestationEventWire::AttestationRevoked(decode!(AttestationRevokedWire))
		},
		"ExternalStatusRevoked" => {
			AttestationEventWire::ExternalStatusRevoked(decode!(ExternalStatusRevokedWire))
		},
		"EmergencyPauseChanged" => {
			AttestationEventWire::EmergencyPauseChanged(decode!(EmergencyPauseChangedWire))
		},
		_ => {
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedRuntime,
				"unknown native attestation event",
			))
		},
	};
	decode_wire_event(wire)
}

fn decode_wire_event(wire: AttestationEventWire) -> DomainResult<AttestationEvent> {
	Ok(match wire {
		AttestationEventWire::SchemaCreated(wire) => AttestationEvent::SchemaCreated {
			schema: SchemaId(domain_hash(wire.schema)),
			creator: domain_account(&wire.creator)?,
			definition_commitment: domain_hash(wire.definition_commitment),
			revocable: wire.revocable,
			unique: wire.unique,
			index_policy: index_policy(wire.index_policy),
		},
		AttestationEventWire::SchemaStatusChanged(wire) => AttestationEvent::SchemaStatusChanged {
			schema: SchemaId(domain_hash(wire.schema)),
			status: schema_status(wire.status),
			forced: wire.forced,
		},
		AttestationEventWire::AttestationIssued(wire) => AttestationEvent::AttestationIssued {
			attestation: AttestationId(domain_hash(wire.attestation)),
			schema: SchemaId(domain_hash(wire.schema)),
			issuer: domain_account(&wire.issuer)?,
			subject_commitment: SubjectCommitment(domain_hash(wire.subject_commitment)),
		},
		AttestationEventWire::DelegatedIntentConsumed(wire) => {
			AttestationEvent::DelegatedIntentConsumed {
				issuer: domain_account(&wire.issuer)?,
				delegate: domain_account(&wire.delegate)?,
				nonce: wire.nonce,
				attestation: AttestationId(domain_hash(wire.attestation)),
			}
		},
		AttestationEventWire::DelegatedRevocationConsumed(wire) => {
			AttestationEvent::DelegatedRevocationConsumed {
				revoker: domain_account(&wire.revoker)?,
				delegate: domain_account(&wire.delegate)?,
				nonce: wire.nonce,
				attestation: AttestationId(domain_hash(wire.attestation)),
			}
		},
		AttestationEventWire::AttestationRevoked(wire) => AttestationEvent::AttestationRevoked {
			attestation: AttestationId(domain_hash(wire.attestation)),
			by: wire.by.as_ref().map(domain_account).transpose()?,
			forced: wire.forced,
		},
		AttestationEventWire::ExternalStatusRevoked(wire) => {
			AttestationEvent::ExternalStatusRevoked {
				key: domain_hash(wire.key),
				issuer: domain_account(&wire.issuer)?,
				status_commitment: StatusCommitment(domain_hash(wire.status_commitment)),
				revoked_at: wire.revoked_at,
			}
		},
		AttestationEventWire::EmergencyPauseChanged(wire) => {
			AttestationEvent::EmergencyPauseChanged { paused: wire.paused }
		},
	})
}

fn domain_hash(hash: RuntimeHash) -> Hash32 {
	Hash32::from_bytes(*hash.as_fixed_bytes())
}

fn runtime_hash(hash: &Hash32) -> DomainResult<RuntimeHash> {
	let bytes = hex::decode(&hash.as_str()[2..]).map_err(|_| {
		NativeError::new(NativeErrorCode::InvalidInput, "invalid finalized block hash")
	})?;
	Ok(RuntimeHash::from_slice(&bytes))
}

fn domain_account(account: &RuntimeAccountId) -> DomainResult<AccountId> {
	AccountId::new(account_id_to_ss58(&account_id_from_subxt(account)))
}

fn index_policy(policy: att_api::IndexPolicy) -> IndexPolicy {
	match policy {
		att_api::IndexPolicy::None => IndexPolicy::None,
		att_api::IndexPolicy::Issuer => IndexPolicy::Issuer,
		att_api::IndexPolicy::SubjectAndSchema => IndexPolicy::SubjectAndSchema,
		att_api::IndexPolicy::IssuerAndSubjectSchema => IndexPolicy::IssuerAndSubjectSchema,
	}
}

fn schema_status(status: att_api::SchemaStatus) -> SchemaStatus {
	match status {
		att_api::SchemaStatus::Active => SchemaStatus::Active,
		att_api::SchemaStatus::Paused => SchemaStatus::Paused,
		att_api::SchemaStatus::Retired => SchemaStatus::Retired,
	}
}

fn subscription_error(error: impl core::fmt::Display) -> NativeError {
	NativeError::new(NativeErrorCode::ContentUnavailable, error.to_string()).retryable()
}

#[cfg(test)]
mod tests {
	use super::super::domains::attestation::AttestationOutcome;
	use super::*;

	fn hash(byte: u8) -> RuntimeHash {
		RuntimeHash::from([byte; 32])
	}

	fn account(byte: u8) -> RuntimeAccountId {
		RuntimeAccountId::from([byte; 32])
	}

	fn exercise_every_wire_variant_decoder_filter_and_outcome_path() {
		let wires = vec![
			AttestationEventWire::SchemaCreated(SchemaCreatedWire {
				schema: hash(1),
				creator: account(2),
				definition_commitment: hash(3),
				revocable: true,
				unique: false,
				index_policy: att_api::IndexPolicy::IssuerAndSubjectSchema,
			}),
			AttestationEventWire::SchemaStatusChanged(SchemaStatusChangedWire {
				schema: hash(1),
				status: att_api::SchemaStatus::Paused,
				forced: false,
			}),
			AttestationEventWire::AttestationIssued(AttestationIssuedWire {
				attestation: hash(4),
				schema: hash(1),
				issuer: account(2),
				subject_commitment: hash(5),
			}),
			AttestationEventWire::DelegatedIntentConsumed(DelegatedIntentConsumedWire {
				issuer: account(2),
				delegate: account(6),
				nonce: 7,
				attestation: hash(4),
			}),
			AttestationEventWire::DelegatedRevocationConsumed(DelegatedRevocationConsumedWire {
				revoker: account(2),
				delegate: account(6),
				nonce: 8,
				attestation: hash(4),
			}),
			AttestationEventWire::AttestationRevoked(AttestationRevokedWire {
				attestation: hash(4),
				by: Some(account(2)),
				forced: false,
			}),
			AttestationEventWire::ExternalStatusRevoked(ExternalStatusRevokedWire {
				key: hash(9),
				issuer: account(2),
				status_commitment: hash(10),
				revoked_at: 11,
			}),
			AttestationEventWire::EmergencyPauseChanged(EmergencyPauseChangedWire { paused: true }),
		];
		let kinds = [
			AttestationEventKind::SchemaCreated,
			AttestationEventKind::SchemaStatusChanged,
			AttestationEventKind::AttestationIssued,
			AttestationEventKind::DelegatedIntentConsumed,
			AttestationEventKind::DelegatedRevocationConsumed,
			AttestationEventKind::AttestationRevoked,
			AttestationEventKind::ExternalStatusRevoked,
			AttestationEventKind::EmergencyPauseChanged,
		];
		let finalized = Hash32::from_bytes([12; 32]);
		let mut pending = VecDeque::new();
		for (index, (wire, expected_kind)) in wires.into_iter().zip(kinds).enumerate() {
			let event = decode_wire_event(wire).unwrap();
			assert_eq!(event.kind(), expected_kind);
			enqueue_if_selected(&mut pending, &kinds, finalized.clone(), index as u32, event);
		}

		assert_eq!(pending.len(), kinds.len());
		for (index, item) in pending.into_iter().enumerate() {
			assert_eq!(item.event.finalized_block_hash, finalized);
			assert_eq!(item.event.event_index, index as u32);
			assert!(match (index, item.outcome) {
				(0, AttestationOutcome::SchemaAvailable { .. })
				| (1, AttestationOutcome::SchemaStatusChanged { .. })
				| (2, AttestationOutcome::AttestationAvailable { .. })
				| (3 | 4, AttestationOutcome::DelegationConsumed { .. })
				| (5, AttestationOutcome::AttestationRevoked { .. })
				| (6, AttestationOutcome::ExternalStatusRevoked { .. })
				| (7, AttestationOutcome::EmergencyPauseChanged { .. }) => true,
				_ => false,
			});
		}
	}

	#[test]
	fn every_wire_variant_decodes_filters_and_produces_its_semantic_outcome() {
		exercise_every_wire_variant_decoder_filter_and_outcome_path();
	}

	#[test]
	fn deterministic_decoded_transport_filters_and_envelopes_events() {
		let finalized = Hash32::from_bytes([1; 32]);
		let attestation = AttestationId::new(format!("0x{}", "02".repeat(32))).unwrap();
		let mut pending = VecDeque::new();
		enqueue_if_selected(
			&mut pending,
			&[AttestationEventKind::AttestationRevoked],
			finalized.clone(),
			3,
			AttestationEvent::AttestationIssued {
				attestation: attestation.clone(),
				schema: SchemaId::new(format!("0x{}", "03".repeat(32))).unwrap(),
				issuer: AccountId::new("issuer:alice").unwrap(),
				subject_commitment: SubjectCommitment::new(format!("0x{}", "04".repeat(32)))
					.unwrap(),
			},
		);
		enqueue_if_selected(
			&mut pending,
			&[AttestationEventKind::AttestationRevoked],
			finalized.clone(),
			4,
			AttestationEvent::AttestationRevoked {
				attestation: attestation.clone(),
				by: Some(AccountId::new("issuer:alice").unwrap()),
				forced: false,
			},
		);

		assert_eq!(pending.len(), 1);
		let item = pending.pop_front().unwrap();
		assert_eq!(item.event.finalized_block_hash, finalized);
		assert_eq!(item.event.event_index, 4);
		assert_eq!(
			item.outcome,
			super::super::domains::attestation::AttestationOutcome::AttestationRevoked {
				attestation,
			}
		);
	}
}
