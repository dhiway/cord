// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Resources-origin transaction extension.

use crate::{
	types::{FriendRequestReference, MembershipCollection, ProofOf},
	weights::WeightInfo as _,
	Config, Origin, Pallet,
};
use codec::{Decode, DecodeWithMemTracking, Encode};
use core::fmt;
use frame_support::{
	ensure,
	pallet_prelude::TransactionSource,
	traits::{Get, IsSubType, OriginTrait, UnixTime},
	weights::Weight,
	CloneNoBound, DefaultNoBound, EqNoBound, PartialEqNoBound,
};
use indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER;
use indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER;
use indiv_support::traits::{Alias, MembershipProver, RevisionIndex, RingIndex};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, TransactionExtension, ValidateResult},
	transaction_validity::{InvalidTransaction, TransactionValidityError, ValidTransaction},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Custom invalidity reasons for the `AsResources` transaction extension.
#[repr(u8)]
pub enum CustomValidity {
	/// The friend request period is outside the accepted current/grace window.
	InvalidFriendRequestPeriod = 220,
	/// The friend request sequence is above the collection-specific slot limit.
	InvalidFriendRequestSequence = 221,
	/// The requested demotion is not currently valid.
	InvalidPersonDemotion = 223,
	/// The requested friend request cleanup is not currently valid.
	InvalidExpiredFriendRequestCleanup = 224,
	/// The alias or account is already registered for a friend request slot.
	FriendRequestRegistrationConflict = 225,
	/// The statement store period is outside the accepted current/grace window.
	InvalidStmtStorePeriod = 226,
	/// The statement store sequence is above the collection-specific slot limit.
	InvalidStmtStoreSequence = 227,
	/// The replacement cooldown has not elapsed since the entry was last set.
	StmtStoreReplacementTooEarly = 228,
	/// The requested expired statement store cleanup is not currently valid.
	InvalidExpiredStmtStoreCleanup = 229,
	/// The long-term storage period is outside the accepted current/grace window.
	InvalidLongTermStoragePeriod = 230,
	/// The long-term storage alias has already been used in this period.
	LongTermStorageAliasAlreadySpent = 231,
	/// The long-term storage counter is out of range.
	InvalidLongTermStorageCounter = 232,
	/// The long-term storage period has not expired yet (for cleanup).
	LongTermStoragePeriodNotExpired = 233,
	/// There are no long-term storage aliases to clear for the period.
	NothingToClearForLongTermStoragePeriod = 234,
}

impl From<CustomValidity> for TransactionValidityError {
	fn from(e: CustomValidity) -> Self {
		InvalidTransaction::Custom(e as u8).into()
	}
}

/// Domain separator for signed, direct long-term storage claim proofs.
pub const DIRECT_LONG_TERM_STORAGE_DOMAIN: &[u8] =
	b"orbis/direct/resources/v6/long-term-storage/account-bound";

/// Validation state passed from `validate` to `prepare`.
pub enum AsResourcesVal<AccountId> {
	Other,
	Claim {
		payer: AccountId,
		alias: Alias,
		collection: MembershipCollection,
		revision: RevisionIndex,
		call_hash: [u8; 32],
	},
}

/// Preparation state retained until post-dispatch.
pub enum AsResourcesPre<AccountId> {
	Other,
	Claim { payer: AccountId, alias: Alias },
}

/// Information required to dispatch the friend request registration call as an anonymous alias.
#[derive(
	Encode, Decode, TypeInfo, EqNoBound, CloneNoBound, PartialEqNoBound, DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T))]
pub enum AsResourcesInfo<T: Config> {
	/// Validate the friend request registration proof for the anonymous alias.
	///
	/// This variant can only be used with
	/// `Call::set_friend_request_statement_account_for_sequence`.
	///
	/// Proof construction:
	/// - the proof must be valid for the people member collection (`PEOPLE_MEMBER_IDENTIFIER`),
	/// - `ring_index` must point to the ring used to create the proof,
	/// - the proof context is derived on-chain from the call payload using
	///   `Pallet::<T>::friend_request_context(reference)`,
	/// - the proof message must be `blake2_256(scale_encode(inherited_implication))`.
	///
	/// See [`AsResourcesInfo::RegisterFriendRequestForCollection`] for the collection-aware form.
	RegisterFriendRequestWithProof(ProofOf<T>, RingIndex),
	/// Validate a friend request registration proof for a member collection.
	///
	/// This variant can only be used with
	/// `Call::set_friend_request_statement_account_for_sequence`.
	///
	/// Proof construction:
	/// - `collection` selects which member collection the proof targets:
	///   [`MembershipCollection::People`] uses `PEOPLE_MEMBER_IDENTIFIER`,
	///   [`MembershipCollection::LitePeople`] uses `LITE_PEOPLE_MEMBER_IDENTIFIER`,
	/// - `ring_index` must point to the ring used to create the proof,
	/// - the proof context is derived on-chain from the call payload using
	///   `Pallet::<T>::friend_request_context(reference)`,
	/// - the proof message must be `blake2_256(scale_encode(inherited_implication))`.
	RegisterFriendRequestForCollection(ProofOf<T>, RingIndex, MembershipCollection),
	/// Validate a statement store allowance claim proof for a member collection.
	///
	/// This variant can only be used with `Call::set_statement_store_account`.
	///
	/// Proof construction:
	/// - `collection` selects which member collection the proof targets:
	///   [`MembershipCollection::People`] uses `PEOPLE_MEMBER_IDENTIFIER`,
	///   [`MembershipCollection::LitePeople`] uses `LITE_PEOPLE_MEMBER_IDENTIFIER`,
	/// - `ring_index` must point to the ring used to create the proof,
	/// - the proof context is derived on-chain from the call payload using
	///   `Pallet::<T>::stmt_store_slot_context(period, seq)`,
	/// - the proof message must be `blake2_256(scale_encode(inherited_implication))`.
	RegisterStatementStoreAllowance(ProofOf<T>, RingIndex, MembershipCollection),
	/// Validate a long-term storage claim proof for a member collection.
	///
	/// This variant can only be used with `Call::claim_long_term_storage`.
	///
	/// Proof construction:
	/// - `collection` selects which member collection the proof targets:
	///   [`MembershipCollection::People`] uses `PEOPLE_MEMBER_IDENTIFIER`,
	///   [`MembershipCollection::LitePeople`] uses `LITE_PEOPLE_MEMBER_IDENTIFIER`,
	/// - `ring_index` must point to the ring used to create the proof,
	/// - `revision` must be the ring revision the proof was built against; any revision still
	///   retained within the grace window is accepted, so a proof remains valid across ring
	///   revisions as long as the corresponding old root has not yet expired,
	/// - the proof context is derived on-chain from the call payload using
	///   `Pallet::<T>::long_term_storage_context(period, counter)`,
	/// - the proof message must be `blake2_256(scale_encode(inherited_implication))`.
	ClaimLongTermStorage(ProofOf<T>, RingIndex, RevisionIndex, MembershipCollection),
}

/// Extension to validate friend request registration proofs and transmute the origin into
/// `resources::Origin::FriendRequestAlias` generated from
/// `Pallet::<T>::friend_request_context(reference)`.
#[derive(
	Encode,
	Decode,
	TypeInfo,
	EqNoBound,
	CloneNoBound,
	PartialEqNoBound,
	DefaultNoBound,
	DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T))]
pub struct AsResources<T: Config>(Option<AsResourcesInfo<T>>);

impl<T: Config> fmt::Debug for AsResources<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "AsResources")
	}
}

impl<T: Config> AsResources<T> {
	pub fn new(explicit: Option<AsResourcesInfo<T>>) -> Self {
		Self(explicit)
	}
}

impl<T> AsResources<T>
where
	T: Config + indiv_pallet_people::Config + indiv_pallet_people_lite::Config,
{
	fn validate_friend_request(
		origin: <T as frame_system::Config>::RuntimeOrigin,
		call: &<T as frame_system::Config>::RuntimeCall,
		inherited_implication: &impl Encode,
		proof: &ProofOf<T>,
		ring_index: RingIndex,
		collection: &MembershipCollection,
	) -> ValidateResult<AsResourcesVal<T::AccountId>, <T as frame_system::Config>::RuntimeCall> {
		ensure!(
			matches!(origin.as_system_ref(), Some(frame_system::RawOrigin::None)),
			InvalidTransaction::BadSigner
		);

		let Some(crate::Call::<T>::set_friend_request_statement_account_for_sequence {
			reference,
			account_id,
		}) = call.is_sub_type()
		else {
			return Err(InvalidTransaction::Call.into());
		};

		Pallet::<T>::validate_friend_request_period(reference.period)
			.map_err(|_| CustomValidity::InvalidFriendRequestPeriod)?;

		match collection {
			MembershipCollection::People => Pallet::<T>::validate_friend_request_seq(reference.seq),
			MembershipCollection::LitePeople => {
				Pallet::<T>::validate_lite_friend_request_seq(reference.seq)
			},
		}
		.map_err(|_| CustomValidity::InvalidFriendRequestSequence)?;

		let context = Pallet::<T>::friend_request_context(FriendRequestReference {
			period: reference.period,
			seq: reference.seq,
		});
		let msg = inherited_implication.using_encoded(sp_io::hashing::blake2_256);

		let identifier = match collection {
			MembershipCollection::People => *PEOPLE_MEMBER_IDENTIFIER,
			MembershipCollection::LitePeople => *LITE_PEOPLE_MEMBER_IDENTIFIER,
		};

		let validated_rev_ca = <T as crate::Config>::MemberService::verify_membership(
			&identifier,
			proof,
			ring_index,
			context,
			&msg[..],
		)
		.map_err(|_| InvalidTransaction::BadProof)?;

		Pallet::<T>::validate_friend_request_registration(validated_rev_ca.ca.alias, account_id)
			.map_err(|_| CustomValidity::FriendRequestRegistrationConflict)?;

		// Deduplicate by the friend request slot itself so a user cannot queue many
		// unsigned variants for the same `(alias, period, seq)` by only changing the
		// statement account.
		let provides = sp_io::hashing::twox_64(
			&("friend request-slot", validated_rev_ca.ca.alias, context).encode(),
		);
		let validity =
			ValidTransaction::with_tag_prefix("Res:FriendRequest").and_provides(provides);

		let local_origin = Origin::FriendRequestAlias(validated_rev_ca.ca.alias);
		let mut origin = origin;
		origin.set_caller_from(local_origin);

		Ok((validity.into(), AsResourcesVal::Other, origin))
	}

	fn validate_stmt_store_slot(
		origin: <T as frame_system::Config>::RuntimeOrigin,
		call: &<T as frame_system::Config>::RuntimeCall,
		inherited_implication: &impl Encode,
		proof: &ProofOf<T>,
		ring_index: RingIndex,
		collection: &MembershipCollection,
	) -> ValidateResult<AsResourcesVal<T::AccountId>, <T as frame_system::Config>::RuntimeCall> {
		ensure!(
			matches!(origin.as_system_ref(), Some(frame_system::RawOrigin::None)),
			InvalidTransaction::BadSigner
		);

		let Some(crate::Call::<T>::set_statement_store_account { period, seq, target_account: _ }) =
			call.is_sub_type()
		else {
			return Err(InvalidTransaction::Call.into());
		};

		let current_period =
			Pallet::<T>::stmt_store_period_from_timestamp(T::Clock::now().as_secs());
		ensure!(*period == current_period, CustomValidity::InvalidStmtStorePeriod);

		let (identifier, seq_limit) = match collection {
			MembershipCollection::People => {
				(*PEOPLE_MEMBER_IDENTIFIER, T::StmtStoreSlotsPerPeriod::get())
			},
			MembershipCollection::LitePeople => {
				(*LITE_PEOPLE_MEMBER_IDENTIFIER, T::LiteStmtStoreSlotsPerPeriod::get())
			},
		};
		ensure!(*seq < seq_limit, CustomValidity::InvalidStmtStoreSequence);

		let context = Pallet::<T>::stmt_store_slot_context(*period, *seq);
		let msg = inherited_implication.using_encoded(sp_io::hashing::blake2_256);

		let validated_rev_ca = <T as crate::Config>::MemberService::verify_membership(
			&identifier,
			proof,
			ring_index,
			context,
			&msg[..],
		)
		.map_err(|_| InvalidTransaction::BadProof)?;

		let alias = validated_rev_ca.ca.alias;
		let period_key = indiv_support::utils::BigEndianU32::from(*period);

		// If an entry already exists, ensure the replacement cooldown has elapsed.
		// This mirrors the dispatch-level check and prevents pool spam of proofs that
		// would fail on dispatch.
		if let Some(existing) = crate::StatementStoreAllowances::<T>::get(period_key, alias) {
			let now = T::Clock::now().as_secs();
			let cooldown = T::StmtStoreReplacementCooldown::get() as u64;
			if now <= existing.since.saturating_add(cooldown) {
				return Err(CustomValidity::StmtStoreReplacementTooEarly.into());
			}
		}

		// Deduplicate by (alias, period, seq) so a user cannot queue many unsigned variants
		// for the same slot by only changing the target account.
		let provides = sp_io::hashing::twox_64(&("stmt-store-slot", alias, context).encode());
		let validity =
			ValidTransaction::with_tag_prefix("Res:StmtStoreSlot").and_provides(provides);

		let local_origin = Origin::StmtStoreAlias(alias);
		let mut origin = origin;
		origin.set_caller_from(local_origin);

		Ok((validity.into(), AsResourcesVal::Other, origin))
	}

	fn validate_long_term_storage_claim(
		origin: <T as frame_system::Config>::RuntimeOrigin,
		call: &<T as frame_system::Config>::RuntimeCall,
		inherited_implication: &impl Encode,
		proof: &ProofOf<T>,
		ring_index: RingIndex,
		revision: RevisionIndex,
		collection: &MembershipCollection,
	) -> ValidateResult<AsResourcesVal<T::AccountId>, <T as frame_system::Config>::RuntimeCall> {
		let signer = match origin.as_system_ref() {
			Some(frame_system::RawOrigin::Signed(signer)) => signer.clone(),
			_ => return Err(InvalidTransaction::BadSigner.into()),
		};

		let Some(crate::Call::<T>::claim_long_term_storage { period, counter, account_id }) =
			call.is_sub_type()
		else {
			return Err(InvalidTransaction::Call.into());
		};
		ensure!(account_id == &signer, InvalidTransaction::BadSigner);

		ensure!(
			Pallet::<T>::is_accepted_long_term_storage_period(*period),
			CustomValidity::InvalidLongTermStoragePeriod
		);
		ensure!(
			*counter < <T as crate::Config>::LongTermStorageClaimsPerPeriod::get(),
			CustomValidity::InvalidLongTermStorageCounter
		);
		let context = Pallet::<T>::long_term_storage_context(*period, *counter);
		let identifier = match collection {
			MembershipCollection::People => *PEOPLE_MEMBER_IDENTIFIER,
			MembershipCollection::LitePeople => *LITE_PEOPLE_MEMBER_IDENTIFIER,
		};

		let bound = match collection {
			MembershipCollection::People => indiv_pallet_people::AccountToAlias::<T>::get(&signer),
			MembershipCollection::LitePeople => {
				indiv_pallet_people_lite::AccountToAlias::<T>::get(&signer)
			},
		}
		.ok_or(InvalidTransaction::BadSigner)?;
		ensure!(bound.ring == ring_index, InvalidTransaction::BadSigner);
		ensure!(bound.revision == revision, InvalidTransaction::BadSigner);
		ensure!(bound.ca.context == context, InvalidTransaction::BadSigner);

		let msg = (
			DIRECT_LONG_TERM_STORAGE_DOMAIN,
			&signer,
			account_id,
			bound.ca.alias,
			collection,
			ring_index,
			revision,
			context,
			call,
			inherited_implication,
		)
			.using_encoded(sp_io::hashing::blake2_256);

		let validated_ca = <T as crate::Config>::MemberService::verify_membership_at_rev(
			&identifier,
			proof,
			ring_index,
			revision,
			context,
			&msg[..],
		)
		.map_err(|_| InvalidTransaction::BadProof)?;
		ensure!(validated_ca == bound.ca, InvalidTransaction::BadSigner);

		let reciprocal = match collection {
			MembershipCollection::People => {
				indiv_pallet_people::AliasToAccount::<T>::get(&bound.ca) == Some(signer.clone())
			},
			MembershipCollection::LitePeople => {
				indiv_pallet_people_lite::AliasToAccount::<T>::get(&bound.ca)
					== Some(signer.clone())
			},
		};
		ensure!(reciprocal, InvalidTransaction::BadSigner);

		ensure!(
			!crate::SpentLongTermStorageAliases::<T>::contains_key(
				indiv_support::utils::BigEndianU32::from(*period),
				bound.ca.alias
			),
			CustomValidity::LongTermStorageAliasAlreadySpent
		);

		let provides = sp_io::hashing::twox_64(&("lts-claim", bound.ca.alias, context).encode());
		let validity =
			ValidTransaction::with_tag_prefix("Res:LongTermStorage").and_provides(provides);

		let local_origin = Origin::LongTermStorageClaim {
			alias: bound.ca.alias,
			collection: *collection,
			payer: signer.clone(),
		};
		let mut origin = origin;
		origin.set_caller_from(local_origin);

		Ok((
			validity.into(),
			AsResourcesVal::Claim {
				payer: signer,
				alias: bound.ca.alias,
				collection: *collection,
				revision,
				call_hash: call.using_encoded(sp_io::hashing::blake2_256),
			},
			origin,
		))
	}
}

impl<T> TransactionExtension<<T as frame_system::Config>::RuntimeCall> for AsResources<T>
where
	T: Config + indiv_pallet_people::Config + indiv_pallet_people_lite::Config,
{
	const IDENTIFIER: &'static str = "AsResources";
	type Implicit = ();
	type Val = AsResourcesVal<T::AccountId>;
	type Pre = AsResourcesPre<T::AccountId>;

	fn weight(&self, _call: &<T as frame_system::Config>::RuntimeCall) -> Weight {
		match self.0 {
			Some(AsResourcesInfo::RegisterFriendRequestWithProof(..)) => {
				<T as Config>::WeightInfo::as_register_with_proof_tx_ext()
			},
			Some(AsResourcesInfo::RegisterFriendRequestForCollection(..)) => {
				<T as Config>::WeightInfo::as_register_for_collection_tx_ext()
			},
			Some(AsResourcesInfo::RegisterStatementStoreAllowance(..)) => {
				<T as Config>::WeightInfo::as_stmt_store_allowance_tx_ext()
			},
			Some(AsResourcesInfo::ClaimLongTermStorage(..)) => {
				<T as Config>::WeightInfo::claim_long_term_storage_tx_ext()
			},
			None => Weight::zero(),
		}
	}

	fn validate(
		&self,
		origin: <T as frame_system::Config>::RuntimeOrigin,
		call: &<T as frame_system::Config>::RuntimeCall,
		_info: &DispatchInfoOf<<T as frame_system::Config>::RuntimeCall>,
		_len: usize,
		_self_implicit: Self::Implicit,
		inherited_implication: &impl Encode,
		_source: TransactionSource,
	) -> ValidateResult<Self::Val, <T as frame_system::Config>::RuntimeCall> {
		match &self.0 {
			Some(AsResourcesInfo::RegisterFriendRequestWithProof(proof, ring_index)) => {
				Self::validate_friend_request(
					origin,
					call,
					inherited_implication,
					proof,
					*ring_index,
					&MembershipCollection::People,
				)
			},
			Some(AsResourcesInfo::RegisterFriendRequestForCollection(
				proof,
				ring_index,
				collection,
			)) => Self::validate_friend_request(
				origin,
				call,
				inherited_implication,
				proof,
				*ring_index,
				collection,
			),
			Some(AsResourcesInfo::RegisterStatementStoreAllowance(
				proof,
				ring_index,
				collection,
			)) => Self::validate_stmt_store_slot(
				origin,
				call,
				inherited_implication,
				proof,
				*ring_index,
				collection,
			),
			Some(AsResourcesInfo::ClaimLongTermStorage(
				proof,
				ring_index,
				revision,
				collection,
			)) => Self::validate_long_term_storage_claim(
				origin,
				call,
				inherited_implication,
				proof,
				*ring_index,
				*revision,
				collection,
			),
			None => Ok((ValidTransaction::default(), AsResourcesVal::Other, origin)),
		}
	}

	fn prepare(
		self,
		_val: Self::Val,
		_origin: &<T as frame_system::Config>::RuntimeOrigin,
		_call: &<T as frame_system::Config>::RuntimeCall,
		_info: &DispatchInfoOf<<T as frame_system::Config>::RuntimeCall>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		Ok(match _val {
			AsResourcesVal::Other => AsResourcesPre::Other,
			AsResourcesVal::Claim { payer, alias, .. } => AsResourcesPre::Claim { payer, alias },
		})
	}
}
