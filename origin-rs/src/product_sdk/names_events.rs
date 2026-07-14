//! Executable finalized native-Orbis Names event subscription.

use std::{collections::VecDeque, pin::Pin};

use futures::{Stream, StreamExt};
use scale_decode::DecodeAsType;
use subxt::{blocks::Block, events::StaticEvent, OnlineClient};

use super::{
	domains::{
		names::{
			NamesEvent, NamesEventKind, NamesEventSubscription, FinalizedNamesEvent,
			FinalizedNamesOutcome, Label, TextKey,
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
			const PALLET: &'static str = "Names";
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

enum NamesEventWire {
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

/// A live finalized subscription that decodes only native `Names` pallet events.
pub struct OrbisNamesEventSubscription {
	blocks: FinalizedBlockStream,
	kinds: Vec<NamesEventKind>,
	pending: VecDeque<FinalizedNamesOutcome>,
}

impl OrbisNamesEventSubscription {
	/// Anchor at the current finalized block and subscribe to subsequent finalized blocks.
	/// Historical replay/reconnect is intentionally owned by production-readiness work.
	pub async fn subscribe(
		client: OnlineClient<OrbisConfig>,
		subscription: NamesEventSubscription,
	) -> DomainResult<Self> {
		let latest = client.blocks().at_latest().await.map_err(subscription_error)?;
		let requested = runtime_hash(&subscription.from_finalized_block)?;
		if latest.hash() != requested {
			return Err(NativeError::new(
				NativeErrorCode::InconsistentSnapshot,
				"Orbis Names subscription anchor must equal the current finalized block",
			));
		}
		let blocks = client.blocks().subscribe_finalized().await.map_err(subscription_error)?;
		Ok(Self { blocks: Box::pin(blocks), kinds: subscription.kinds, pending: VecDeque::new() })
	}

	pub async fn next(&mut self) -> DomainResult<Option<FinalizedNamesOutcome>> {
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
				if details.pallet_name() != "Names" {
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
	pending: &mut VecDeque<FinalizedNamesOutcome>,
	kinds: &[NamesEventKind],
	finalized_block_hash: Hash32,
	event_index: u32,
	event: NamesEvent,
) {
	if kinds.contains(&event.kind()) {
		let outcome = event.outcome();
		pending.push_back(FinalizedNamesOutcome {
			event: FinalizedNamesEvent { finalized_block_hash, event_index, event },
			outcome,
		});
	}
}

fn decode_event(details: &subxt::events::EventDetails<OrbisConfig>) -> DomainResult<NamesEvent> {
	macro_rules! decode {
		($wire:ty) => {
			details.as_event::<$wire>().map_err(subscription_error)?.ok_or_else(|| {
				NativeError::new(
					NativeErrorCode::UnsupportedRuntime,
					"Orbis Names event metadata mismatch",
				)
			})?
		};
	}
	let wire = match details.variant_name() {
		"CommitmentStored" => NamesEventWire::CommitmentStored(decode!(CommitmentStoredWire)),
		"CommitmentRemoved" => NamesEventWire::CommitmentRemoved(decode!(CommitmentRemovedWire)),
		"NameRegistered" => NamesEventWire::NameRegistered(decode!(NameRegisteredWire)),
		"NameRenewed" => NamesEventWire::NameRenewed(decode!(NameRenewedWire)),
		"NameTransferred" => NamesEventWire::NameTransferred(decode!(NameTransferredWire)),
		"NameReleased" => NamesEventWire::NameReleased(decode!(NameReleasedWire)),
		"ExpiredNameRemoved" => NamesEventWire::ExpiredNameRemoved(decode!(ExpiredNameRemovedWire)),
		"ControllerAdded" => NamesEventWire::ControllerAdded(decode!(ControllerAddedWire)),
		"ControllerRemoved" => NamesEventWire::ControllerRemoved(decode!(ControllerRemovedWire)),
		"AddressSet" => NamesEventWire::AddressSet(decode!(AddressSetWire)),
		"SubjectSet" => NamesEventWire::SubjectSet(decode!(SubjectSetWire)),
		"AttestationSet" => NamesEventWire::AttestationSet(decode!(AttestationSetWire)),
		"ContentSet" => NamesEventWire::ContentSet(decode!(ContentSetWire)),
		"TextSet" => NamesEventWire::TextSet(decode!(TextSetWire)),
		"PrimaryNameSet" => NamesEventWire::PrimaryNameSet(decode!(PrimaryNameSetWire)),
		"NameReserved" => NamesEventWire::NameReserved(decode!(NameReservedWire)),
		"ReservationCleared" => NamesEventWire::ReservationCleared(decode!(ReservationClearedWire)),
		"LabelProtectionSet" => NamesEventWire::LabelProtectionSet(decode!(LabelProtectionSetWire)),
		"PauseSet" => NamesEventWire::PauseSet(decode!(PauseSetWire)),
		"EmergencyNameRevoked" => {
			NamesEventWire::EmergencyNameRevoked(decode!(EmergencyNameRevokedWire))
		},
		"RegistrarSet" => NamesEventWire::RegistrarSet(decode!(RegistrarSetWire)),
		_ => {
			return Err(NativeError::new(
				NativeErrorCode::UnsupportedRuntime,
				"unknown native Orbis Names event",
			))
		},
	};
	decode_wire_event(wire)
}

fn decode_wire_event(wire: NamesEventWire) -> DomainResult<NamesEvent> {
	Ok(match wire {
		NamesEventWire::CommitmentStored(w) => NamesEvent::CommitmentStored {
			owner: domain_account(&w.owner)?,
			commitment: commitment(w.commitment),
			at: w.at,
		},
		NamesEventWire::CommitmentRemoved(w) => NamesEvent::CommitmentRemoved {
			owner: domain_account(&w.owner)?,
			commitment: commitment(w.commitment),
		},
		NamesEventWire::NameRegistered(w) => NamesEvent::NameRegistered {
			name: name(w.name),
			parent: w.parent.map(name),
			label: label(w.label)?,
			owner: domain_account(&w.owner)?,
			expires_at: w.expires_at,
		},
		NamesEventWire::NameRenewed(w) => {
			NamesEvent::NameRenewed { name: name(w.name), expires_at: w.expires_at }
		},
		NamesEventWire::NameTransferred(w) => NamesEvent::NameTransferred {
			name: name(w.name),
			from: domain_account(&w.from)?,
			to: domain_account(&w.to)?,
		},
		NamesEventWire::NameReleased(w) => {
			NamesEvent::NameReleased { name: name(w.name), owner: domain_account(&w.owner)? }
		},
		NamesEventWire::ExpiredNameRemoved(w) => {
			NamesEvent::ExpiredNameRemoved { name: name(w.name) }
		},
		NamesEventWire::ControllerAdded(w) => NamesEvent::ControllerAdded {
			name: name(w.name),
			controller: domain_account(&w.controller)?,
		},
		NamesEventWire::ControllerRemoved(w) => NamesEvent::ControllerRemoved {
			name: name(w.name),
			controller: domain_account(&w.controller)?,
		},
		NamesEventWire::AddressSet(w) => {
			NamesEvent::AddressSet { name: name(w.name), present: w.present }
		},
		NamesEventWire::SubjectSet(w) => {
			NamesEvent::SubjectSet { name: name(w.name), present: w.present }
		},
		NamesEventWire::AttestationSet(w) => {
			NamesEvent::AttestationSet { name: name(w.name), present: w.present }
		},
		NamesEventWire::ContentSet(w) => {
			NamesEvent::ContentSet { name: name(w.name), present: w.present }
		},
		NamesEventWire::TextSet(w) => NamesEvent::TextSet {
			name: name(w.name),
			key: TextKey::new(w.key)?,
			present: w.present,
		},
		NamesEventWire::PrimaryNameSet(w) => {
			NamesEvent::PrimaryNameSet { owner: domain_account(&w.owner)?, name: w.name.map(name) }
		},
		NamesEventWire::NameReserved(w) => NamesEvent::NameReserved {
			name: name(w.name),
			beneficiary: w.beneficiary.as_ref().map(domain_account).transpose()?,
			expires_at: w.expires_at,
		},
		NamesEventWire::ReservationCleared(w) => {
			NamesEvent::ReservationCleared { name: name(w.name) }
		},
		NamesEventWire::LabelProtectionSet(w) => {
			NamesEvent::LabelProtectionSet { label: label(w.label)?, protected: w.protected }
		},
		NamesEventWire::PauseSet(w) => NamesEvent::PauseSet { paused: w.paused },
		NamesEventWire::EmergencyNameRevoked(w) => {
			NamesEvent::EmergencyNameRevoked { name: name(w.name) }
		},
		NamesEventWire::RegistrarSet(w) => NamesEvent::RegistrarSet {
			registrar: domain_account(&w.registrar)?,
			enabled: w.enabled,
		},
	})
}

fn label(bytes: Vec<u8>) -> DomainResult<Label> {
	Label::new(
		String::from_utf8(bytes).map_err(|_| {
			NativeError::new(NativeErrorCode::InvalidInput, "Orbis Names label is not UTF-8")
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
			NamesEventWire::CommitmentStored(CommitmentStoredWire {
				owner: account(1),
				commitment: hash(2),
				at: 3,
			}),
			NamesEventWire::CommitmentRemoved(CommitmentRemovedWire {
				owner: account(1),
				commitment: hash(2),
			}),
			NamesEventWire::NameRegistered(NameRegisteredWire {
				name: hash(4),
				parent: None,
				label: b"alice".to_vec(),
				owner: account(1),
				expires_at: 5,
			}),
			NamesEventWire::NameRenewed(NameRenewedWire { name: hash(4), expires_at: 6 }),
			NamesEventWire::NameTransferred(NameTransferredWire {
				name: hash(4),
				from: account(1),
				to: account(2),
			}),
			NamesEventWire::NameReleased(NameReleasedWire { name: hash(4), owner: account(2) }),
			NamesEventWire::ExpiredNameRemoved(ExpiredNameRemovedWire { name: hash(4) }),
			NamesEventWire::ControllerAdded(ControllerAddedWire {
				name: hash(4),
				controller: account(3),
			}),
			NamesEventWire::ControllerRemoved(ControllerRemovedWire {
				name: hash(4),
				controller: account(3),
			}),
			NamesEventWire::AddressSet(AddressSetWire { name: hash(4), present: true }),
			NamesEventWire::SubjectSet(SubjectSetWire { name: hash(4), present: true }),
			NamesEventWire::AttestationSet(AttestationSetWire { name: hash(4), present: true }),
			NamesEventWire::ContentSet(ContentSetWire { name: hash(4), present: true }),
			NamesEventWire::TextSet(TextSetWire {
				name: hash(4),
				key: b"url".to_vec(),
				present: true,
			}),
			NamesEventWire::PrimaryNameSet(PrimaryNameSetWire {
				owner: account(1),
				name: Some(hash(4)),
			}),
			NamesEventWire::NameReserved(NameReservedWire {
				name: hash(4),
				beneficiary: Some(account(1)),
				expires_at: Some(7),
			}),
			NamesEventWire::ReservationCleared(ReservationClearedWire { name: hash(4) }),
			NamesEventWire::LabelProtectionSet(LabelProtectionSetWire {
				label: b"root".to_vec(),
				protected: true,
			}),
			NamesEventWire::PauseSet(PauseSetWire { paused: true }),
			NamesEventWire::EmergencyNameRevoked(EmergencyNameRevokedWire { name: hash(4) }),
			NamesEventWire::RegistrarSet(RegistrarSetWire { registrar: account(5), enabled: true }),
		];
		let kinds = [
			NamesEventKind::CommitmentStored,
			NamesEventKind::CommitmentRemoved,
			NamesEventKind::NameRegistered,
			NamesEventKind::NameRenewed,
			NamesEventKind::NameTransferred,
			NamesEventKind::NameReleased,
			NamesEventKind::ExpiredNameRemoved,
			NamesEventKind::ControllerAdded,
			NamesEventKind::ControllerRemoved,
			NamesEventKind::AddressSet,
			NamesEventKind::SubjectSet,
			NamesEventKind::AttestationSet,
			NamesEventKind::ContentSet,
			NamesEventKind::TextSet,
			NamesEventKind::PrimaryNameSet,
			NamesEventKind::NameReserved,
			NamesEventKind::ReservationCleared,
			NamesEventKind::LabelProtectionSet,
			NamesEventKind::PauseSet,
			NamesEventKind::EmergencyNameRevoked,
			NamesEventKind::RegistrarSet,
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
