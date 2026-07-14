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

//! Bounded immutable schemas and commitment-only native attestations for Orbis.
//!
//! Claim bodies remain off-chain. This pallet stores only bounded schema definitions and opaque
//! commitments needed to prove issuance, relationships, uniqueness, expiry, and revocation.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{
	pallet_prelude::*,
	traits::{EnsureOrigin, StorageVersion},
	transactional, BoundedVec, CloneNoBound, DebugNoBound, EqNoBound, PartialEqNoBound,
};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::traits::{Hash as HashT, IdentifyAccount, Verify, Zero};
pub use weights::WeightInfo;

const SCHEMA_ID_DOMAIN: &[u8] = b"cord:orbis:schema:v1";
const ATTESTATION_ID_DOMAIN: &[u8] = b"cord:orbis:attestation:v1";
const DELEGATED_INTENT_DOMAIN: &[u8] = b"cord:orbis:delegated-attestation:v1";
const DELEGATED_REVOKE_DOMAIN: &[u8] = b"cord:orbis:delegated-revocation:v1";
const EXTERNAL_STATUS_DOMAIN: &[u8] = b"cord:orbis:external-status:v1";
const UNIQUE_ATTESTATION_KEY: &[u8] = b"unique";
const NON_UNIQUE_ATTESTATION_KEY: &[u8] = b"nonce";

pub type SchemaDefinitionOf<T> = BoundedVec<u8, <T as Config>::MaxSchemaDefinitionLen>;
pub type AuthorizedIssuersOf<T> =
	BoundedVec<<T as frame_system::Config>::AccountId, <T as Config>::MaxAuthorizedIssuers>;
pub type SchemaIndexOf<T> =
	BoundedVec<<T as frame_system::Config>::Hash, <T as Config>::MaxSchemasPerCreator>;
pub type AttestationIndexOf<T> =
	BoundedVec<<T as frame_system::Config>::Hash, <T as Config>::MaxAttestationsPerIndex>;
pub type BatchOf<T> = BoundedVec<AttestationInputOf<T>, <T as Config>::MaxBatchSize>;
pub type AttestationIdBatchOf<T> =
	BoundedVec<<T as frame_system::Config>::Hash, <T as Config>::MaxBatchSize>;
pub type DelegatedIssueBatchOf<T> =
	BoundedVec<SignedDelegatedIntent<T>, <T as Config>::MaxBatchSize>;
pub type DelegatedRevokeBatchOf<T> =
	BoundedVec<SignedDelegatedRevokeIntent<T>, <T as Config>::MaxBatchSize>;

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Default,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum SchemaStatus {
	#[default]
	Active,
	Paused,
	Retired,
}

/// Bounded native indexes admitted for a schema. There are no external resolver callbacks.
#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Default,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum IndexPolicy {
	#[default]
	None,
	Issuer,
	SubjectAndSchema,
	IssuerAndSubjectSchema,
}

impl IndexPolicy {
	fn indexes_issuer(self) -> bool {
		matches!(self, Self::Issuer | Self::IssuerAndSubjectSchema)
	}

	fn indexes_subject_schema(self) -> bool {
		matches!(self, Self::SubjectAndSchema | Self::IssuerAndSubjectSchema)
	}
}

#[derive(
	Clone,
	Copy,
	Debug,
	Decode,
	DecodeWithMemTracking,
	Encode,
	Eq,
	MaxEncodedLen,
	PartialEq,
	TypeInfo,
)]
pub enum DelegatedAction {
	Issue,
	Revoke,
}

#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct SchemaRecord<T: Config> {
	pub creator: T::AccountId,
	pub definition: SchemaDefinitionOf<T>,
	pub definition_commitment: T::Hash,
	pub status: SchemaStatus,
	/// Whether attestations under this schema may ever be revoked.
	pub revocable: bool,
	/// Whether every issuance must provide and permanently consume a uniqueness commitment.
	pub unique: bool,
	pub index_policy: IndexPolicy,
	pub authorized_issuers: AuthorizedIssuersOf<T>,
	pub created_at: BlockNumberFor<T>,
}

#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct AttestationInput<T: Config> {
	pub schema: T::Hash,
	pub subject_commitment: T::Hash,
	pub payload_commitment: T::Hash,
	pub status_commitment: T::Hash,
	pub parent: Option<T::Hash>,
	pub expiry: Option<BlockNumberFor<T>>,
	pub uniqueness_commitment: Option<T::Hash>,
	/// An individual attestation may opt out of revocation, but cannot opt in when its schema is
	/// irrevocable.
	pub revocable: bool,
}

pub type AttestationInputOf<T> = AttestationInput<T>;

#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct DelegatedIntent<T: Config> {
	pub genesis_hash: T::Hash,
	pub spec_version: u32,
	pub action: DelegatedAction,
	pub issuer: T::AccountId,
	pub delegate: T::AccountId,
	pub schema: T::Hash,
	pub subject_commitment: T::Hash,
	pub payload_commitment: T::Hash,
	pub status_commitment: T::Hash,
	pub parent: Option<T::Hash>,
	pub expiry: Option<BlockNumberFor<T>>,
	pub uniqueness_commitment: Option<T::Hash>,
	pub revocable: bool,
	pub nonce: u64,
	pub deadline: BlockNumberFor<T>,
}

pub type DelegatedIntentOf<T> = DelegatedIntent<T>;

#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct SignedDelegatedIntent<T: Config> {
	pub intent: DelegatedIntentOf<T>,
	pub signature: T::Signature,
}

#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct DelegatedRevokeIntent<T: Config> {
	pub genesis_hash: T::Hash,
	pub spec_version: u32,
	pub action: DelegatedAction,
	pub revoker: T::AccountId,
	pub delegate: T::AccountId,
	pub attestation: T::Hash,
	pub nonce: u64,
	pub deadline: BlockNumberFor<T>,
}

pub type DelegatedRevokeIntentOf<T> = DelegatedRevokeIntent<T>;

#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct SignedDelegatedRevokeIntent<T: Config> {
	pub intent: DelegatedRevokeIntentOf<T>,
	pub signature: T::Signature,
}

#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct AttestationRecord<T: Config> {
	pub issuer: T::AccountId,
	pub schema: T::Hash,
	pub subject_commitment: T::Hash,
	pub payload_commitment: T::Hash,
	pub status_commitment: T::Hash,
	pub parent: Option<T::Hash>,
	pub expiry: Option<BlockNumberFor<T>>,
	pub uniqueness_commitment: Option<T::Hash>,
	pub revocable: bool,
	pub issuance_nonce: u64,
	pub issued_at: BlockNumberFor<T>,
	pub revoked_at: Option<BlockNumberFor<T>>,
	pub revoked_by: Option<T::AccountId>,
}

#[derive(
	CloneNoBound,
	DebugNoBound,
	Decode,
	DecodeWithMemTracking,
	Encode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct ExternalStatusRecord<T: Config> {
	pub issuer: T::AccountId,
	pub status_commitment: T::Hash,
	pub revoked_at: BlockNumberFor<T>,
}

pub fn delegated_signing_payload<T: Config>(intent: &DelegatedIntentOf<T>) -> Vec<u8> {
	(DELEGATED_INTENT_DOMAIN, intent).encode()
}

pub fn delegated_revoke_signing_payload<T: Config>(intent: &DelegatedRevokeIntentOf<T>) -> Vec<u8> {
	(DELEGATED_REVOKE_DOMAIN, intent).encode()
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Public signer used to prove which issuer authorized a delegated intent.
		type Signer: IdentifyAccount<AccountId = Self::AccountId>;

		/// Signature used by an issuer to authorize a delegated issuance intent.
		type Signature: Parameter + MaxEncodedLen + Verify<Signer = Self::Signer>;

		/// Emergency administration. Runtime integration should use a governed origin.
		type AdminOrigin: EnsureOrigin<OriginFor<Self>>;

		#[pallet::constant]
		type MaxSchemaDefinitionLen: Get<u32>;
		#[pallet::constant]
		type MaxAuthorizedIssuers: Get<u32>;
		#[pallet::constant]
		type MaxSchemasPerCreator: Get<u32>;
		#[pallet::constant]
		type MaxAttestationsPerIndex: Get<u32>;
		#[pallet::constant]
		type MaxBatchSize: Get<u32>;
		/// Maximum SCALE-encoded bytes accepted by any one batch call.
		#[pallet::constant]
		type MaxBatchEncodedLen: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type Schemas<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, SchemaRecord<T>, OptionQuery>;

	#[pallet::storage]
	pub type Attestations<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, AttestationRecord<T>, OptionQuery>;

	/// Total schemas registered since genesis. Schemas are never deleted.
	#[pallet::storage]
	pub type SchemaCount<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Total attestations issued since genesis. Attestations are never deleted.
	#[pallet::storage]
	pub type AttestationCount<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Next issuer-scoped nonce used to derive repeatable, non-unique attestation identifiers.
	#[pallet::storage]
	pub type NextIssuerAttestationNonce<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

	/// Issuer-authenticated status revocations for commitments whose status is maintained
	/// off-chain.
	#[pallet::storage]
	pub type ExternalStatuses<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, ExternalStatusRecord<T>, OptionQuery>;

	/// Creator -> bounded schema identifiers.
	#[pallet::storage]
	pub type CreatorSchemas<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, SchemaIndexOf<T>, ValueQuery>;

	/// Issuer -> bounded attestation identifiers.
	#[pallet::storage]
	pub type IssuerAttestations<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, AttestationIndexOf<T>, ValueQuery>;

	/// Schema + subject commitment -> bounded attestation identifiers.
	#[pallet::storage]
	pub type SubjectSchemaAttestations<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Blake2_128Concat,
		T::Hash,
		AttestationIndexOf<T>,
		ValueQuery,
	>;

	/// O(1) membership predicate used by runtime integrations such as Orbis Names. Attestations are
	/// append-only, so a boolean is sufficient and does not duplicate any mutable record.
	#[pallet::storage]
	pub type KnownSubjects<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, (), OptionQuery>;

	/// A uniqueness commitment is permanently consumed within its schema, including after expiry
	/// or revocation, preventing semantic replay under a new attestation identifier.
	#[pallet::storage]
	pub type UniquenessIndex<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Hash,
		Blake2_128Concat,
		T::Hash,
		T::Hash,
		OptionQuery,
	>;

	/// Next exact nonce accepted from each delegated issuer.
	#[pallet::storage]
	pub type NextDelegatedNonce<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

	#[pallet::storage]
	pub type EmergencyPaused<T: Config> = StorageValue<_, bool, ValueQuery>;

	/// Empty-by-design genesis. The only written key is the explicit storage version marker.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		#[serde(skip)]
		pub _phantom: core::marker::PhantomData<T>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { _phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			STORAGE_VERSION.put::<Pallet<T>>();
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		SchemaCreated {
			schema: T::Hash,
			creator: T::AccountId,
			definition_commitment: T::Hash,
			revocable: bool,
			unique: bool,
			index_policy: IndexPolicy,
		},
		SchemaStatusChanged {
			schema: T::Hash,
			status: SchemaStatus,
			forced: bool,
		},
		AttestationIssued {
			attestation: T::Hash,
			schema: T::Hash,
			issuer: T::AccountId,
			subject_commitment: T::Hash,
		},
		DelegatedIntentConsumed {
			issuer: T::AccountId,
			delegate: T::AccountId,
			nonce: u64,
			attestation: T::Hash,
		},
		DelegatedRevocationConsumed {
			revoker: T::AccountId,
			delegate: T::AccountId,
			nonce: u64,
			attestation: T::Hash,
		},
		AttestationRevoked {
			attestation: T::Hash,
			by: Option<T::AccountId>,
			forced: bool,
		},
		ExternalStatusRevoked {
			key: T::Hash,
			issuer: T::AccountId,
			status_commitment: T::Hash,
			revoked_at: BlockNumberFor<T>,
		},
		EmergencyPauseChanged {
			paused: bool,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		EmergencyPaused,
		EmptySchemaDefinition,
		DuplicateAuthorizedIssuer,
		EmptyBatch,
		BatchPayloadTooLarge,
		SchemaAlreadyExists,
		SchemaCountOverflow,
		SchemaNotFound,
		SchemaNotActive,
		SchemaRetired,
		NotSchemaCreator,
		UnauthorizedIssuer,
		CreatorSchemaIndexFull,
		IssuerAttestationIndexFull,
		SubjectSchemaIndexFull,
		AttestationAlreadyExists,
		AttestationCountOverflow,
		InvalidAttestationId,
		AttestationNotFound,
		AttestationExpired,
		AttestationRevoked,
		ParentSchemaMismatch,
		UniquenessAlreadyUsed,
		UniqueCommitmentRequired,
		UnexpectedUniquenessCommitment,
		RevocableMismatch,
		Irrevocable,
		InvalidGenesisHash,
		InvalidSpecVersion,
		InvalidDelegatedAction,
		InvalidDelegate,
		InvalidSignature,
		InvalidNonce,
		IntentExpired,
		ExpiryNotInFuture,
		NonceOverflow,
		ExternalStatusAlreadyRevoked,
		NotAuthorizedToRevoke,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::create_schema(definition.len() as u32, authorized_issuers.len() as u32))]
		#[transactional]
		pub fn create_schema(
			origin: OriginFor<T>,
			definition: SchemaDefinitionOf<T>,
			authorized_issuers: AuthorizedIssuersOf<T>,
			revocable: bool,
			unique: bool,
			index_policy: IndexPolicy,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			Self::ensure_operational()?;
			ensure!(!definition.is_empty(), Error::<T>::EmptySchemaDefinition);
			for (index, issuer) in authorized_issuers.iter().enumerate() {
				ensure!(
					!authorized_issuers[..index].contains(issuer),
					Error::<T>::DuplicateAuthorizedIssuer
				);
			}
			let definition_commitment = T::Hashing::hash(definition.as_slice());
			let schema =
				Self::schema_id(&creator, &definition_commitment, revocable, unique, index_policy);
			ensure!(!Schemas::<T>::contains_key(schema), Error::<T>::SchemaAlreadyExists);
			let schema_count =
				SchemaCount::<T>::get().checked_add(1).ok_or(Error::<T>::SchemaCountOverflow)?;
			CreatorSchemas::<T>::try_mutate(&creator, |schemas| {
				schemas.try_push(schema).map_err(|_| Error::<T>::CreatorSchemaIndexFull)
			})?;
			Schemas::<T>::insert(
				schema,
				SchemaRecord::<T> {
					creator: creator.clone(),
					definition,
					definition_commitment,
					status: SchemaStatus::Active,
					revocable,
					unique,
					index_policy,
					authorized_issuers,
					created_at: frame_system::Pallet::<T>::block_number(),
				},
			);
			SchemaCount::<T>::put(schema_count);
			Self::deposit_event(Event::SchemaCreated {
				schema,
				creator,
				definition_commitment,
				revocable,
				unique,
				index_policy,
			});
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::set_schema_status())]
		pub fn set_schema_status(
			origin: OriginFor<T>,
			schema: T::Hash,
			status: SchemaStatus,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			Schemas::<T>::try_mutate(schema, |entry| -> DispatchResult {
				let record = entry.as_mut().ok_or(Error::<T>::SchemaNotFound)?;
				ensure!(record.creator == creator, Error::<T>::NotSchemaCreator);
				ensure!(record.status != SchemaStatus::Retired, Error::<T>::SchemaRetired);
				record.status = status;
				Ok(())
			})?;
			Self::deposit_event(Event::SchemaStatusChanged { schema, status, forced: false });
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::issue())]
		#[transactional]
		pub fn issue(origin: OriginFor<T>, input: AttestationInputOf<T>) -> DispatchResult {
			let issuer = ensure_signed(origin)?;
			Self::ensure_operational()?;
			Self::issue_inner(&issuer, &input)?;
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::issue_delegated())]
		#[transactional]
		pub fn issue_delegated(
			origin: OriginFor<T>,
			intent: DelegatedIntentOf<T>,
			signature: T::Signature,
		) -> DispatchResult {
			let delegate = ensure_signed(origin)?;
			Self::ensure_operational()?;
			Self::validate_delegated_issue(
				&intent,
				&signature,
				&delegate,
				NextDelegatedNonce::<T>::get(&intent.issuer),
			)?;
			let input = Self::input_from_intent(&intent);
			let attestation = Self::issue_inner(&intent.issuer, &input)?;
			Self::consume_delegated_issue(intent, delegate, attestation)
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::issue_batch(items.len() as u32))]
		#[transactional]
		pub fn issue_batch(origin: OriginFor<T>, items: BatchOf<T>) -> DispatchResult {
			let issuer = ensure_signed(origin)?;
			Self::ensure_operational()?;
			Self::ensure_batch(&items, items.len())?;
			for input in items.iter() {
				Self::issue_inner(&issuer, input)?;
			}
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::revoke())]
		pub fn revoke(origin: OriginFor<T>, attestation: T::Hash) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::revoke_inner(attestation, Some(who), false)
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::set_emergency_pause())]
		pub fn set_emergency_pause(origin: OriginFor<T>, paused: bool) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			EmergencyPaused::<T>::put(paused);
			Self::deposit_event(Event::EmergencyPauseChanged { paused });
			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::force_schema_status())]
		pub fn force_schema_status(
			origin: OriginFor<T>,
			schema: T::Hash,
			status: SchemaStatus,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			Schemas::<T>::try_mutate(schema, |entry| -> DispatchResult {
				entry.as_mut().ok_or(Error::<T>::SchemaNotFound)?.status = status;
				Ok(())
			})?;
			Self::deposit_event(Event::SchemaStatusChanged { schema, status, forced: true });
			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::force_revoke())]
		pub fn force_revoke(origin: OriginFor<T>, attestation: T::Hash) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			Self::revoke_inner(attestation, None, true)
		}

		/// Revoke on behalf of an issuer/schema creator using an exact chain-bound signed intent.
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::revoke_delegated())]
		#[transactional]
		pub fn revoke_delegated(
			origin: OriginFor<T>,
			intent: DelegatedRevokeIntentOf<T>,
			signature: T::Signature,
		) -> DispatchResult {
			let delegate = ensure_signed(origin)?;
			Self::ensure_operational()?;
			Self::validate_delegated_revoke(
				&intent,
				&signature,
				&delegate,
				NextDelegatedNonce::<T>::get(&intent.revoker),
			)?;
			Self::revoke_inner(intent.attestation, Some(intent.revoker.clone()), false)?;
			Self::consume_delegated_revoke(intent, delegate)
		}

		/// Atomically issue a batch of delegated intents. Every signature/nonce/deadline is checked
		/// before the first state mutation.
		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::issue_delegated_batch(items.len() as u32))]
		#[transactional]
		pub fn issue_delegated_batch(
			origin: OriginFor<T>,
			items: DelegatedIssueBatchOf<T>,
		) -> DispatchResult {
			let delegate = ensure_signed(origin)?;
			Self::ensure_operational()?;
			Self::ensure_batch(&items, items.len())?;
			for (index, item) in items.iter().enumerate() {
				let preceding = items[..index]
					.iter()
					.filter(|prior| prior.intent.issuer == item.intent.issuer)
					.count() as u64;
				let expected = NextDelegatedNonce::<T>::get(&item.intent.issuer)
					.checked_add(preceding)
					.ok_or(Error::<T>::NonceOverflow)?;
				Self::validate_delegated_issue(&item.intent, &item.signature, &delegate, expected)?;
			}
			for item in items {
				let input = Self::input_from_intent(&item.intent);
				let attestation = Self::issue_inner(&item.intent.issuer, &input)?;
				Self::consume_delegated_issue(item.intent, delegate.clone(), attestation)?;
			}
			Ok(())
		}

		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::revoke_batch(attestations.len() as u32))]
		#[transactional]
		pub fn revoke_batch(
			origin: OriginFor<T>,
			attestations: AttestationIdBatchOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_operational()?;
			Self::ensure_batch(&attestations, attestations.len())?;
			for attestation in attestations {
				Self::revoke_inner(attestation, Some(who.clone()), false)?;
			}
			Ok(())
		}

		/// Atomically revoke a delegated batch after first checking every signature and sequential
		/// per-revoker nonce.
		#[pallet::call_index(12)]
		#[pallet::weight(T::WeightInfo::revoke_delegated_batch(items.len() as u32))]
		#[transactional]
		pub fn revoke_delegated_batch(
			origin: OriginFor<T>,
			items: DelegatedRevokeBatchOf<T>,
		) -> DispatchResult {
			let delegate = ensure_signed(origin)?;
			Self::ensure_operational()?;
			Self::ensure_batch(&items, items.len())?;
			for (index, item) in items.iter().enumerate() {
				let preceding = items[..index]
					.iter()
					.filter(|prior| prior.intent.revoker == item.intent.revoker)
					.count() as u64;
				let expected = NextDelegatedNonce::<T>::get(&item.intent.revoker)
					.checked_add(preceding)
					.ok_or(Error::<T>::NonceOverflow)?;
				Self::validate_delegated_revoke(
					&item.intent,
					&item.signature,
					&delegate,
					expected,
				)?;
			}
			for item in items {
				Self::revoke_inner(
					item.intent.attestation,
					Some(item.intent.revoker.clone()),
					false,
				)?;
				Self::consume_delegated_revoke(item.intent, delegate.clone())?;
			}
			Ok(())
		}

		/// Record the signed issuer's revocation of an externally maintained status commitment.
		#[pallet::call_index(13)]
		#[pallet::weight(T::WeightInfo::revoke_external_status())]
		pub fn revoke_external_status(
			origin: OriginFor<T>,
			status_commitment: T::Hash,
		) -> DispatchResult {
			let issuer = ensure_signed(origin)?;
			Self::ensure_operational()?;
			Self::revoke_external_status_inner(&issuer, status_commitment)
		}

		/// Atomically revoke a bounded batch of externally maintained status commitments.
		#[pallet::call_index(14)]
		#[pallet::weight(T::WeightInfo::revoke_external_status_batch(status_commitments.len() as u32))]
		#[transactional]
		pub fn revoke_external_status_batch(
			origin: OriginFor<T>,
			status_commitments: AttestationIdBatchOf<T>,
		) -> DispatchResult {
			let issuer = ensure_signed(origin)?;
			Self::ensure_operational()?;
			Self::ensure_batch(&status_commitments, status_commitments.len())?;
			for status_commitment in status_commitments {
				Self::revoke_external_status_inner(&issuer, status_commitment)?;
			}
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn schema_id(
			creator: &T::AccountId,
			definition_commitment: &T::Hash,
			revocable: bool,
			unique: bool,
			index_policy: IndexPolicy,
		) -> T::Hash {
			T::Hashing::hash_of(&(
				SCHEMA_ID_DOMAIN,
				creator,
				definition_commitment,
				revocable,
				unique,
				index_policy,
			))
		}

		pub fn attestation_id(
			issuer: &T::AccountId,
			input: &AttestationInputOf<T>,
			issuance_nonce: u64,
		) -> T::Hash {
			if let Some(unique) = input.uniqueness_commitment {
				T::Hashing::hash_of(&(
					ATTESTATION_ID_DOMAIN,
					UNIQUE_ATTESTATION_KEY,
					issuer,
					input.schema,
					input.subject_commitment,
					unique,
				))
			} else {
				T::Hashing::hash_of(&(
					ATTESTATION_ID_DOMAIN,
					NON_UNIQUE_ATTESTATION_KEY,
					issuer,
					input.schema,
					input.subject_commitment,
					issuance_nonce,
				))
			}
		}

		pub fn external_status_key(issuer: &T::AccountId, status_commitment: T::Hash) -> T::Hash {
			T::Hashing::hash_of(&(EXTERNAL_STATUS_DOMAIN, issuer, status_commitment))
		}

		pub fn contains_attestation(attestation: T::Hash) -> bool {
			Attestations::<T>::contains_key(attestation)
		}

		pub fn is_live(attestation: T::Hash) -> bool {
			let now = frame_system::Pallet::<T>::block_number();
			Attestations::<T>::get(attestation).is_some_and(|record| {
				record.revoked_at.is_none()
					&& record.expiry.is_none_or(|expiry| now < expiry)
					&& Schemas::<T>::get(record.schema)
						.is_some_and(|schema| schema.status == SchemaStatus::Active)
			})
		}

		pub fn is_subject_known(subject_commitment: T::Hash) -> bool {
			KnownSubjects::<T>::contains_key(subject_commitment)
		}

		fn ensure_batch<I: Encode>(items: &I, len: usize) -> DispatchResult {
			ensure!(len > 0, Error::<T>::EmptyBatch);
			ensure!(
				items.encoded_size() <= T::MaxBatchEncodedLen::get() as usize,
				Error::<T>::BatchPayloadTooLarge
			);
			Ok(())
		}

		fn ensure_delegated_context(
			genesis_hash: T::Hash,
			spec_version: u32,
			delegate: &T::AccountId,
			expected_delegate: &T::AccountId,
			deadline: BlockNumberFor<T>,
		) -> DispatchResult {
			ensure!(delegate == expected_delegate, Error::<T>::InvalidDelegate);
			ensure!(
				genesis_hash == frame_system::BlockHash::<T>::get(BlockNumberFor::<T>::zero()),
				Error::<T>::InvalidGenesisHash
			);
			ensure!(spec_version == T::Version::get().spec_version, Error::<T>::InvalidSpecVersion);
			ensure!(
				frame_system::Pallet::<T>::block_number() <= deadline,
				Error::<T>::IntentExpired
			);
			Ok(())
		}

		fn validate_delegated_issue(
			intent: &DelegatedIntentOf<T>,
			signature: &T::Signature,
			delegate: &T::AccountId,
			expected_nonce: u64,
		) -> DispatchResult {
			ensure!(intent.action == DelegatedAction::Issue, Error::<T>::InvalidDelegatedAction);
			Self::ensure_delegated_context(
				intent.genesis_hash,
				intent.spec_version,
				delegate,
				&intent.delegate,
				intent.deadline,
			)?;
			ensure!(intent.nonce == expected_nonce, Error::<T>::InvalidNonce);
			ensure!(
				signature.verify(delegated_signing_payload::<T>(intent).as_slice(), &intent.issuer),
				Error::<T>::InvalidSignature
			);
			Ok(())
		}

		fn validate_delegated_revoke(
			intent: &DelegatedRevokeIntentOf<T>,
			signature: &T::Signature,
			delegate: &T::AccountId,
			expected_nonce: u64,
		) -> DispatchResult {
			ensure!(intent.action == DelegatedAction::Revoke, Error::<T>::InvalidDelegatedAction);
			Self::ensure_delegated_context(
				intent.genesis_hash,
				intent.spec_version,
				delegate,
				&intent.delegate,
				intent.deadline,
			)?;
			ensure!(intent.nonce == expected_nonce, Error::<T>::InvalidNonce);
			ensure!(
				signature.verify(
					delegated_revoke_signing_payload::<T>(intent).as_slice(),
					&intent.revoker,
				),
				Error::<T>::InvalidSignature
			);
			Ok(())
		}

		fn input_from_intent(intent: &DelegatedIntentOf<T>) -> AttestationInputOf<T> {
			AttestationInput::<T> {
				schema: intent.schema,
				subject_commitment: intent.subject_commitment,
				payload_commitment: intent.payload_commitment,
				status_commitment: intent.status_commitment,
				parent: intent.parent,
				expiry: intent.expiry,
				uniqueness_commitment: intent.uniqueness_commitment,
				revocable: intent.revocable,
			}
		}

		fn consume_delegated_issue(
			intent: DelegatedIntentOf<T>,
			delegate: T::AccountId,
			attestation: T::Hash,
		) -> DispatchResult {
			let next = intent.nonce.checked_add(1).ok_or(Error::<T>::NonceOverflow)?;
			NextDelegatedNonce::<T>::insert(&intent.issuer, next);
			Self::deposit_event(Event::DelegatedIntentConsumed {
				issuer: intent.issuer,
				delegate,
				nonce: intent.nonce,
				attestation,
			});
			Ok(())
		}

		fn consume_delegated_revoke(
			intent: DelegatedRevokeIntentOf<T>,
			delegate: T::AccountId,
		) -> DispatchResult {
			let next = intent.nonce.checked_add(1).ok_or(Error::<T>::NonceOverflow)?;
			NextDelegatedNonce::<T>::insert(&intent.revoker, next);
			Self::deposit_event(Event::DelegatedRevocationConsumed {
				revoker: intent.revoker,
				delegate,
				nonce: intent.nonce,
				attestation: intent.attestation,
			});
			Ok(())
		}

		fn ensure_operational() -> DispatchResult {
			ensure!(!EmergencyPaused::<T>::get(), Error::<T>::EmergencyPaused);
			Ok(())
		}

		fn issue_inner(
			issuer: &T::AccountId,
			input: &AttestationInputOf<T>,
		) -> Result<T::Hash, DispatchError> {
			let now = frame_system::Pallet::<T>::block_number();
			let schema = Schemas::<T>::get(input.schema).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(schema.status == SchemaStatus::Active, Error::<T>::SchemaNotActive);
			ensure!(
				schema.creator == *issuer || schema.authorized_issuers.contains(issuer),
				Error::<T>::UnauthorizedIssuer
			);
			if let Some(expiry) = input.expiry {
				ensure!(expiry > now, Error::<T>::ExpiryNotInFuture);
			}
			ensure!(!input.revocable || schema.revocable, Error::<T>::RevocableMismatch);
			if schema.unique {
				ensure!(
					input.uniqueness_commitment.is_some(),
					Error::<T>::UniqueCommitmentRequired
				);
			} else {
				ensure!(
					input.uniqueness_commitment.is_none(),
					Error::<T>::UnexpectedUniquenessCommitment
				);
			}
			if let Some(parent) = input.parent {
				let parent_record =
					Attestations::<T>::get(parent).ok_or(Error::<T>::AttestationNotFound)?;
				ensure!(parent_record.schema == input.schema, Error::<T>::ParentSchemaMismatch);
				ensure!(parent_record.revoked_at.is_none(), Error::<T>::AttestationRevoked);
				ensure!(
					parent_record.expiry.is_none_or(|expiry| now < expiry),
					Error::<T>::AttestationExpired
				);
			}
			if let Some(unique) = input.uniqueness_commitment {
				ensure!(
					!UniquenessIndex::<T>::contains_key(input.schema, unique),
					Error::<T>::UniquenessAlreadyUsed
				);
			}
			let issuance_nonce = NextIssuerAttestationNonce::<T>::get(issuer);
			let next_issuance_nonce =
				issuance_nonce.checked_add(1).ok_or(Error::<T>::NonceOverflow)?;
			let attestation_count = AttestationCount::<T>::get()
				.checked_add(1)
				.ok_or(Error::<T>::AttestationCountOverflow)?;
			let attestation = Self::attestation_id(issuer, input, issuance_nonce);
			ensure!(attestation != T::Hash::default(), Error::<T>::InvalidAttestationId);
			ensure!(
				!Attestations::<T>::contains_key(attestation),
				Error::<T>::AttestationAlreadyExists
			);
			if schema.index_policy.indexes_issuer() {
				IssuerAttestations::<T>::try_mutate(issuer, |items| {
					items.try_push(attestation).map_err(|_| Error::<T>::IssuerAttestationIndexFull)
				})?;
			}
			if schema.index_policy.indexes_subject_schema() {
				SubjectSchemaAttestations::<T>::try_mutate(
					input.schema,
					input.subject_commitment,
					|items| {
						items.try_push(attestation).map_err(|_| Error::<T>::SubjectSchemaIndexFull)
					},
				)?;
			}
			if let Some(unique) = input.uniqueness_commitment {
				UniquenessIndex::<T>::insert(input.schema, unique, attestation);
			}
			Attestations::<T>::insert(
				attestation,
				AttestationRecord::<T> {
					issuer: issuer.clone(),
					schema: input.schema,
					subject_commitment: input.subject_commitment,
					payload_commitment: input.payload_commitment,
					status_commitment: input.status_commitment,
					parent: input.parent,
					expiry: input.expiry,
					uniqueness_commitment: input.uniqueness_commitment,
					revocable: input.revocable,
					issuance_nonce,
					issued_at: now,
					revoked_at: None,
					revoked_by: None,
				},
			);
			NextIssuerAttestationNonce::<T>::insert(issuer, next_issuance_nonce);
			AttestationCount::<T>::put(attestation_count);
			KnownSubjects::<T>::insert(input.subject_commitment, ());
			Self::deposit_event(Event::AttestationIssued {
				attestation,
				schema: input.schema,
				issuer: issuer.clone(),
				subject_commitment: input.subject_commitment,
			});
			Ok(attestation)
		}

		fn revoke_external_status_inner(
			issuer: &T::AccountId,
			status_commitment: T::Hash,
		) -> DispatchResult {
			let key = Self::external_status_key(issuer, status_commitment);
			ensure!(
				!ExternalStatuses::<T>::contains_key(key),
				Error::<T>::ExternalStatusAlreadyRevoked
			);
			let revoked_at = frame_system::Pallet::<T>::block_number();
			ExternalStatuses::<T>::insert(
				key,
				ExternalStatusRecord::<T> { issuer: issuer.clone(), status_commitment, revoked_at },
			);
			Self::deposit_event(Event::ExternalStatusRevoked {
				key,
				issuer: issuer.clone(),
				status_commitment,
				revoked_at,
			});
			Ok(())
		}

		fn revoke_inner(
			attestation: T::Hash,
			by: Option<T::AccountId>,
			forced: bool,
		) -> DispatchResult {
			let now = frame_system::Pallet::<T>::block_number();
			Attestations::<T>::try_mutate(attestation, |entry| -> DispatchResult {
				let record = entry.as_mut().ok_or(Error::<T>::AttestationNotFound)?;
				ensure!(record.revoked_at.is_none(), Error::<T>::AttestationRevoked);
				if !forced {
					ensure!(record.revocable, Error::<T>::Irrevocable);
					let who = by.as_ref().ok_or(Error::<T>::NotAuthorizedToRevoke)?;
					let schema =
						Schemas::<T>::get(record.schema).ok_or(Error::<T>::SchemaNotFound)?;
					ensure!(
						*who == record.issuer || *who == schema.creator,
						Error::<T>::NotAuthorizedToRevoke
					);
				}
				record.revoked_at = Some(now);
				record.revoked_by = by.clone();
				Ok(())
			})?;
			Self::deposit_event(Event::AttestationRevoked { attestation, by, forced });
			Ok(())
		}
	}
}
