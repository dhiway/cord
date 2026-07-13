//! Executable finalized native-DotNS event subscription.

use std::{collections::VecDeque, pin::Pin};

use futures::{Stream, StreamExt};
use scale_decode::DecodeAsType;
use subxt::{blocks::Block, events::StaticEvent, OnlineClient};

use super::{
	domains::{
		dotns::{
			DotnsEvent, DotnsEventKind, DotnsEventSubscription, FinalizedDotnsEvent,
			FinalizedDotnsOutcome, Label, TextKey,
		},
		AccountId, DomainResult, Hash32, NameId, RegistrationCommitment,
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

macro_rules! wire {
	($name:ident, $event:literal, {$($field:ident: $ty:ty),* $(,)?}) => {
		#[derive(DecodeAsType)]
		struct $name { $( $field: $ty, )* }
		impl StaticEvent for $name {
			const PALLET: &'static str = "Dotns";
			const EVENT: &'static str = $event;
		}
	};
}

wire!(CommitmentStoredWire, "CommitmentStored", { owner: RuntimeAccountId, commitment: RuntimeHash, at: u32 });
wire!(CommitmentRemovedWire, "CommitmentRemoved", { owner: RuntimeAccountId, commitment: RuntimeHash });
wire!(NameRegisteredWire, "NameRegistered", { name: RuntimeHash, parent: Option<RuntimeHash>, label: Vec<u8>, owner: RuntimeAccountId, expires_at: u32 });
wire!(NameRenewedWire, "NameRenewed", { name: RuntimeHash, expires_at: u32 });
wire!(NameTransferredWire, "NameTransferred", { name: RuntimeHash, from: RuntimeAccountId, to: RuntimeAccountId });
wire!(NameReleasedWire, "NameReleased", { name: RuntimeHash, owner: RuntimeAccountId });
wire!(ExpiredNameRemovedWire, "ExpiredNameRemoved", { name: RuntimeHash });
wire!(ControllerAddedWire, "ControllerAdded", { name: RuntimeHash, controller: RuntimeAccountId });
wire!(ControllerRemovedWire, "ControllerRemoved", { name: RuntimeHash, controller: RuntimeAccountId });
wire!(AddressSetWire, "AddressSet", { name: RuntimeHash, present: bool });
wire!(SubjectSetWire, "SubjectSet", { name: RuntimeHash, present: bool });
wire!(AttestationSetWire, "AttestationSet", { name: RuntimeHash, present: bool });
wire!(ContentSetWire, "ContentSet", { name: RuntimeHash, present: bool });
wire!(TextSetWire, "TextSet", { name: RuntimeHash, key: Vec<u8>, present: bool });
wire!(PrimaryNameSetWire, "PrimaryNameSet", { owner: RuntimeAccountId, name: Option<RuntimeHash> });
wire!(NameReservedWire, "NameReserved", { name: RuntimeHash, beneficiary: Option<RuntimeAccountId>, expires_at: Option<u32> });
wire!(ReservationClearedWire, "ReservationCleared", { name: RuntimeHash });
wire!(LabelProtectionSetWire, "LabelProtectionSet", { label: Vec<u8>, protected: bool });
wire!(PauseSetWire, "PauseSet", { paused: bool });
wire!(EmergencyNameRevokedWire, "EmergencyNameRevoked", { name: RuntimeHash });
wire!(RegistrarSetWire, "RegistrarSet", { registrar: RuntimeAccountId, enabled: bool });

enum DotnsEventWire {
	CommitmentStored(CommitmentStoredWire),
	CommitmentRemoved(CommitmentRemovedWire),
	NameRegistered(NameRegisteredWire),
	NameRenewed(NameRenewedWire),
	NameTransferred(NameTransferredWire),
	NameReleased(NameReleasedWire),
	ExpiredNameRemoved(ExpiredNameRemovedWire),
	ControllerAdded(ControllerAddedWire),
	ControllerRemoved(ControllerRemovedWire),
	AddressSet(AddressSetWire),
	SubjectSet(SubjectSetWire),
	AttestationSet(AttestationSetWire),
	ContentSet(ContentSetWire),
	TextSet(TextSetWire),
	PrimaryNameSet(PrimaryNameSetWire),
	NameReserved(NameReservedWire),
	ReservationCleared(ReservationClearedWire),
	LabelProtectionSet(LabelProtectionSetWire),
	PauseSet(PauseSetWire),
	EmergencyNameRevoked(EmergencyNameRevokedWire),
	RegistrarSet(RegistrarSetWire),
}

/// A live finalized subscription that decodes only native `Dotns` pallet events.
pub struct OrbisDotnsEventSubscription {
	blocks: FinalizedBlockStream,
	kinds: Vec<DotnsEventKind>,
	pending: VecDeque<FinalizedDotnsOutcome>,
}

impl OrbisDotnsEventSubscription {
	/// Anchor at the current finalized block and subscribe to subsequent finalized blocks.
	/// Historical replay/reconnect is intentionally owned by production-readiness work.
	pub async fn subscribe(
		client: OnlineClient<OrbisConfig>,
		subscription: DotnsEventSubscription,
	) -> DomainResult<Self> {
		let latest = client.blocks().at_latest().await.map_err(subscription_error)?;
		let requested = runtime_hash(&subscription.from_finalized_block)?;
		if latest.hash() != requested {
			return Err(NativeError::new(
				NativeErrorCode::InconsistentSnapshot,
				"DotNS subscription anchor must equal the current finalized block",
			));
		}
		let blocks = client.blocks().subscribe_finalized().await.map_err(subscription_error)?;
		Ok(Self { blocks: Box::pin(blocks), kinds: subscription.kinds, pending: VecDeque::new() })
	}

	pub async fn next(&mut self) -> DomainResult<Option<FinalizedDotnsOutcome>> {
		loop {
			if let Some(item) = self.pending.pop_front() {
				return Ok(Some(item));
			}
			let Some(block) = self.blocks.next().await else { return Ok(None) };
			let block = block.map_err(subscription_error)?;
			let finalized_block_hash = domain_hash(block.hash());
			let events = block.events().await.map_err(subscription_error)?;
			for details in events.iter() {
				let details = details.map_err(subscription_error)?;
				if details.pallet_name() != "Dotns" {
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
	pending: &mut VecDeque<FinalizedDotnsOutcome>,
	kinds: &[DotnsEventKind],
	finalized_block_hash: Hash32,
	event_index: u32,
	event: DotnsEvent,
) {
	if kinds.contains(&event.kind()) {
		let outcome = event.outcome();
		pending.push_back(FinalizedDotnsOutcome {
			event: FinalizedDotnsEvent { finalized_block_hash, event_index, event },
			outcome,
		});
	}
}

fn decode_event(details: &subxt::events::EventDetails<OrbisConfig>) -> DomainResult<DotnsEvent> {
	macro_rules! decode {
		($wire:ty) => {
			details.as_event::<$wire>().map_err(subscription_error)?.ok_or_else(|| {
				NativeError::new(
					NativeErrorCode::UnsupportedRuntime,
					"DotNS event metadata mismatch",
				)
			})?
		};
	}
	let wire = match details.variant_name() {
		"CommitmentStored" => DotnsEventWire::CommitmentStored(decode!(CommitmentStoredWire)),
		"CommitmentRemoved" => DotnsEventWire::CommitmentRemoved(decode!(CommitmentRemovedWire)),
		"NameRegistered" => DotnsEventWire::NameRegistered(decode!(NameRegisteredWire)),
		"NameRenewed" => DotnsEventWire::NameRenewed(decode!(NameRenewedWire)),
		"NameTransferred" => DotnsEventWire::NameTransferred(decode!(NameTransferredWire)),
		"NameReleased" => DotnsEventWire::NameReleased(decode!(NameReleasedWire)),
		"ExpiredNameRemoved" => DotnsEventWire::ExpiredNameRemoved(decode!(ExpiredNameRemovedWire)),
		"ControllerAdded" => DotnsEventWire::ControllerAdded(decode!(ControllerAddedWire)),
		"ControllerRemoved" => DotnsEventWire::ControllerRemoved(decode!(ControllerRemovedWire)),
		"AddressSet" => DotnsEventWire::AddressSet(decode!(AddressSetWire)),
		"SubjectSet" => DotnsEventWire::SubjectSet(decode!(SubjectSetWire)),
		"AttestationSet" => DotnsEventWire::AttestationSet(decode!(AttestationSetWire)),
		"ContentSet" => DotnsEventWire::ContentSet(decode!(ContentSetWire)),
		"TextSet" => DotnsEventWire::TextSet(decode!(TextSetWire)),
		"PrimaryNameSet" => DotnsEventWire::PrimaryNameSet(decode!(PrimaryNameSetWire)),
		"NameReserved" => DotnsEventWire::NameReserved(decode!(NameReservedWire)),
		"ReservationCleared" => DotnsEventWire::ReservationCleared(decode!(ReservationClearedWire)),
		"LabelProtectionSet" => DotnsEventWire::LabelProtectionSet(decode!(LabelProtectionSetWire)),
		"PauseSet" => DotnsEventWire::PauseSet(decode!(PauseSetWire)),
		"EmergencyNameRevoked" => {
			DotnsEventWire::EmergencyNameRevoked(decode!(EmergencyNameRevokedWire))
		},
		"RegistrarSet" => DotnsEventWire::RegistrarSet(decode!(RegistrarSetWire)),
		_ => {
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedRuntime,
				"unknown native DotNS event",
			))
		},
	};
	decode_wire_event(wire)
}

fn decode_wire_event(wire: DotnsEventWire) -> DomainResult<DotnsEvent> {
	Ok(match wire {
		DotnsEventWire::CommitmentStored(w) => DotnsEvent::CommitmentStored {
			owner: domain_account(&w.owner)?,
			commitment: commitment(w.commitment),
			at: w.at,
		},
		DotnsEventWire::CommitmentRemoved(w) => DotnsEvent::CommitmentRemoved {
			owner: domain_account(&w.owner)?,
			commitment: commitment(w.commitment),
		},
		DotnsEventWire::NameRegistered(w) => DotnsEvent::NameRegistered {
			name: name(w.name),
			parent: w.parent.map(name),
			label: label(w.label)?,
			owner: domain_account(&w.owner)?,
			expires_at: w.expires_at,
		},
		DotnsEventWire::NameRenewed(w) => {
			DotnsEvent::NameRenewed { name: name(w.name), expires_at: w.expires_at }
		},
		DotnsEventWire::NameTransferred(w) => DotnsEvent::NameTransferred {
			name: name(w.name),
			from: domain_account(&w.from)?,
			to: domain_account(&w.to)?,
		},
		DotnsEventWire::NameReleased(w) => {
			DotnsEvent::NameReleased { name: name(w.name), owner: domain_account(&w.owner)? }
		},
		DotnsEventWire::ExpiredNameRemoved(w) => {
			DotnsEvent::ExpiredNameRemoved { name: name(w.name) }
		},
		DotnsEventWire::ControllerAdded(w) => DotnsEvent::ControllerAdded {
			name: name(w.name),
			controller: domain_account(&w.controller)?,
		},
		DotnsEventWire::ControllerRemoved(w) => DotnsEvent::ControllerRemoved {
			name: name(w.name),
			controller: domain_account(&w.controller)?,
		},
		DotnsEventWire::AddressSet(w) => {
			DotnsEvent::AddressSet { name: name(w.name), present: w.present }
		},
		DotnsEventWire::SubjectSet(w) => {
			DotnsEvent::SubjectSet { name: name(w.name), present: w.present }
		},
		DotnsEventWire::AttestationSet(w) => {
			DotnsEvent::AttestationSet { name: name(w.name), present: w.present }
		},
		DotnsEventWire::ContentSet(w) => {
			DotnsEvent::ContentSet { name: name(w.name), present: w.present }
		},
		DotnsEventWire::TextSet(w) => DotnsEvent::TextSet {
			name: name(w.name),
			key: TextKey::new(w.key)?,
			present: w.present,
		},
		DotnsEventWire::PrimaryNameSet(w) => {
			DotnsEvent::PrimaryNameSet { owner: domain_account(&w.owner)?, name: w.name.map(name) }
		},
		DotnsEventWire::NameReserved(w) => DotnsEvent::NameReserved {
			name: name(w.name),
			beneficiary: w.beneficiary.as_ref().map(domain_account).transpose()?,
			expires_at: w.expires_at,
		},
		DotnsEventWire::ReservationCleared(w) => {
			DotnsEvent::ReservationCleared { name: name(w.name) }
		},
		DotnsEventWire::LabelProtectionSet(w) => {
			DotnsEvent::LabelProtectionSet { label: label(w.label)?, protected: w.protected }
		},
		DotnsEventWire::PauseSet(w) => DotnsEvent::PauseSet { paused: w.paused },
		DotnsEventWire::EmergencyNameRevoked(w) => {
			DotnsEvent::EmergencyNameRevoked { name: name(w.name) }
		},
		DotnsEventWire::RegistrarSet(w) => DotnsEvent::RegistrarSet {
			registrar: domain_account(&w.registrar)?,
			enabled: w.enabled,
		},
	})
}

fn label(bytes: Vec<u8>) -> DomainResult<Label> {
	Label::new(
		String::from_utf8(bytes).map_err(|_| {
			NativeError::new(NativeErrorCode::InvalidInput, "DotNS label is not UTF-8")
		})?,
	)
}
fn name(hash: RuntimeHash) -> NameId {
	NameId(domain_hash(hash))
}
fn commitment(hash: RuntimeHash) -> RegistrationCommitment {
	RegistrationCommitment(domain_hash(hash))
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
fn subscription_error(error: impl core::fmt::Display) -> NativeError {
	NativeError::new(NativeErrorCode::ContentUnavailable, error.to_string()).retryable()
}

#[cfg(test)]
mod tests {
	use super::*;
	fn hash(byte: u8) -> RuntimeHash {
		RuntimeHash::from([byte; 32])
	}
	fn account(byte: u8) -> RuntimeAccountId {
		RuntimeAccountId::from([byte; 32])
	}

	#[test]
	fn every_wire_variant_decodes_filters_and_produces_an_outcome() {
		let wires = vec![
			DotnsEventWire::CommitmentStored(CommitmentStoredWire {
				owner: account(1),
				commitment: hash(2),
				at: 3,
			}),
			DotnsEventWire::CommitmentRemoved(CommitmentRemovedWire {
				owner: account(1),
				commitment: hash(2),
			}),
			DotnsEventWire::NameRegistered(NameRegisteredWire {
				name: hash(4),
				parent: None,
				label: b"alice".to_vec(),
				owner: account(1),
				expires_at: 5,
			}),
			DotnsEventWire::NameRenewed(NameRenewedWire { name: hash(4), expires_at: 6 }),
			DotnsEventWire::NameTransferred(NameTransferredWire {
				name: hash(4),
				from: account(1),
				to: account(2),
			}),
			DotnsEventWire::NameReleased(NameReleasedWire { name: hash(4), owner: account(2) }),
			DotnsEventWire::ExpiredNameRemoved(ExpiredNameRemovedWire { name: hash(4) }),
			DotnsEventWire::ControllerAdded(ControllerAddedWire {
				name: hash(4),
				controller: account(3),
			}),
			DotnsEventWire::ControllerRemoved(ControllerRemovedWire {
				name: hash(4),
				controller: account(3),
			}),
			DotnsEventWire::AddressSet(AddressSetWire { name: hash(4), present: true }),
			DotnsEventWire::SubjectSet(SubjectSetWire { name: hash(4), present: true }),
			DotnsEventWire::AttestationSet(AttestationSetWire { name: hash(4), present: true }),
			DotnsEventWire::ContentSet(ContentSetWire { name: hash(4), present: true }),
			DotnsEventWire::TextSet(TextSetWire {
				name: hash(4),
				key: b"url".to_vec(),
				present: true,
			}),
			DotnsEventWire::PrimaryNameSet(PrimaryNameSetWire {
				owner: account(1),
				name: Some(hash(4)),
			}),
			DotnsEventWire::NameReserved(NameReservedWire {
				name: hash(4),
				beneficiary: Some(account(1)),
				expires_at: Some(7),
			}),
			DotnsEventWire::ReservationCleared(ReservationClearedWire { name: hash(4) }),
			DotnsEventWire::LabelProtectionSet(LabelProtectionSetWire {
				label: b"root".to_vec(),
				protected: true,
			}),
			DotnsEventWire::PauseSet(PauseSetWire { paused: true }),
			DotnsEventWire::EmergencyNameRevoked(EmergencyNameRevokedWire { name: hash(4) }),
			DotnsEventWire::RegistrarSet(RegistrarSetWire { registrar: account(5), enabled: true }),
		];
		let kinds = [
			DotnsEventKind::CommitmentStored,
			DotnsEventKind::CommitmentRemoved,
			DotnsEventKind::NameRegistered,
			DotnsEventKind::NameRenewed,
			DotnsEventKind::NameTransferred,
			DotnsEventKind::NameReleased,
			DotnsEventKind::ExpiredNameRemoved,
			DotnsEventKind::ControllerAdded,
			DotnsEventKind::ControllerRemoved,
			DotnsEventKind::AddressSet,
			DotnsEventKind::SubjectSet,
			DotnsEventKind::AttestationSet,
			DotnsEventKind::ContentSet,
			DotnsEventKind::TextSet,
			DotnsEventKind::PrimaryNameSet,
			DotnsEventKind::NameReserved,
			DotnsEventKind::ReservationCleared,
			DotnsEventKind::LabelProtectionSet,
			DotnsEventKind::PauseSet,
			DotnsEventKind::EmergencyNameRevoked,
			DotnsEventKind::RegistrarSet,
		];
		let finalized = Hash32::from_bytes([9; 32]);
		let mut pending = VecDeque::new();
		for (index, (wire, kind)) in wires.into_iter().zip(kinds).enumerate() {
			let event = decode_wire_event(wire).unwrap();
			assert_eq!(event.kind(), kind);
			enqueue_if_selected(&mut pending, &kinds, finalized.clone(), index as u32, event);
		}
		assert_eq!(pending.len(), 21);
		for (index, item) in pending.into_iter().enumerate() {
			assert_eq!(item.event.event_index, index as u32);
			assert_eq!(item.event.event.kind(), kinds[index]);
		}
	}
}
