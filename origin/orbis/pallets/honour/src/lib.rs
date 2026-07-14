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

//! Honour pallet.
//!
//! Baseline reputation system for the People Chain. Individuals can direct up to 256 points
//! toward arbitrary subjects, each either honourable (+1) or dishonourable (-1).
//!
//! Once a point is bestowed, it can be redirected to another subject (or the same subject with a
//! different direction) after the timeout of [`Config::PointFreezeDuration`] blocks has passed.
//!
//! To submit a vote, the transaction must use the [`VoterAuth`](extension::VoterAuth) extension.
//! For the transaction to succeed, a valid ring proof must be attached, generated for a ring
//! belonging to the `indiv-pallet-people` pallet, thus proving the voter is a verified human.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod bench_helpers;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
pub mod extension;
pub mod types;
pub mod weights;

pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use alloc::vec::Vec;
	use frame_support::{
		dispatch::{DispatchInfo, GetDispatchInfo, PostDispatchInfo},
		pallet_prelude::*,
		traits::{IsSubType, UnixTime},
		Blake2_128Concat,
	};
	use frame_system::pallet_prelude::*;
	use indiv_support::traits::{
		ContextualAlias, MembershipMultiProver, RevisionIndex, RingIndex, PEOPLE_IDENTIFIER,
	};
	use sp_runtime::traits::{BadOrigin, Dispatchable};
	use verifiable::GenerateVerifiable;

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::origin]
	#[derive(
		Clone, PartialEq, Eq, Debug, Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking,
	)]
	#[scale_info(skip_type_params(T))]
	pub enum Origin<T: Config> {
		Voter { account: T::AccountId, aliases: VoteAliases },
	}

	#[pallet::config]
	pub trait Config:
		frame_system::Config<
		RuntimeOrigin: From<Origin<Self>>
		                   + From<<Self::RuntimeOrigin as OriginTrait>::PalletsOrigin>
		                   + OriginTrait<
			PalletsOrigin: From<Origin<Self>>
			                   + TryInto<
				Origin<Self>,
				Error = <Self::RuntimeOrigin as OriginTrait>::PalletsOrigin,
			>,
		>,
		RuntimeCall: Parameter
		                 + GetDispatchInfo
		                 + IsSubType<Call<Self>>
		                 + Dispatchable<
			RuntimeOrigin = Self::RuntimeOrigin,
			Info = DispatchInfo,
			PostInfo = PostDispatchInfo,
		>,
	>
	{
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		/// Ring-membership prover used to verify multi-context proofs sent through
		/// the [`VoterAuth`](extension::VoterAuth) extension.
		type MemberService: MembershipMultiProver<
			Crypto: GenerateVerifiable<Proof: Send + Sync + DecodeWithMemTracking>,
		>;

		/// Clock used to track the current time.
		type Clock: UnixTime;

		/// Duration in seconds that must pass before a point can be reused.
		#[pallet::constant]
		type PointFreezeDuration: Get<Seconds>;

		/// Duration in seconds a `bestow` call remains valid after its `call_valid_from`.
		#[pallet::constant]
		type CallMortality: Get<Seconds>;

		/// Helper that seeds the membership ring and produces a valid proof for benchmarks.
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: benchmarking::BenchmarkHelper<Self>;
	}

	/// Points that have been used by voters.
	///
	/// Records in [`Points`] track used points to prevent double-spending and enforce rate-limiting
	/// based on the last usage time.
	#[pallet::storage]
	pub type Points<T: Config> = StorageMap<_, Blake2_128Concat, PointAlias, PointInfo>;

	/// Votes that have been bestowed.
	///
	/// Prevents double-voting on the same subject by the same voter, as [`SubjectAlias`] is
	/// uniquely derived from [`SubjectId`] for each voter.
	#[pallet::storage]
	pub type Votes<T: Config> = StorageMap<_, Blake2_128Concat, SubjectAlias, ()>;

	/// Absolute honour score of a subject. Initialized to -1 to offset self-votes.
	#[pallet::storage]
	pub type Tally<T: Config> = StorageMap<_, Blake2_128Concat, SubjectId, Honour>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A subject was voted on with a previously unused point.
		VoteCast {
			/// The subject that was voted on.
			subject: SubjectId,
			/// The direction of the vote.
			direction: Direction,
		},
		/// A point was redirected from one subject to another.
		VoteReused {
			/// The subject that previously held the redirected point.
			old_subject: SubjectId,
			/// The previous direction of the point.
			old_direction: Direction,
			/// The subject that now holds the redirected point.
			new_subject: SubjectId,
			/// The new direction of the vote.
			new_direction: Direction,
		},
		/// The honour score of a subject has changed.
		HonourChanged {
			/// The subject whose honour score changed.
			subject: SubjectId,
			/// The honour value before the update.
			old_value: Honour,
			/// The honour value after the update.
			new_value: Honour,
		},
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// Arithmetic error like over/underflow, division by zero or similar.
		Arithmetic,
		/// There is already a vote by the same voter for the same subject.
		SubjectAlreadyVoted,
		/// The provided ring proof failed verification.
		InvalidProof,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn integrity_test() {
			let freeze = T::PointFreezeDuration::get();
			let mortality = T::CallMortality::get();
			assert!(
				mortality <= freeze.saturating_mul(2),
				"CallMortality must be <= 2 * PointFreezeDuration to prevent vote-redirect replay"
			);
		}
	}

	#[pallet::call(weight(<T as Config>::WeightInfo))]
	impl<T: Config> Pallet<T> {
		/// Bestow a vote.
		///
		/// Accepts only the [`Origin::Voter`] origin, which is created by verifying a ring proof
		/// in the [`VoterAuth`](extension::VoterAuth) transaction extension.
		#[pallet::call_index(0)]
		pub fn bestow(
			origin: OriginFor<T>,
			vote: VoteData,
			_call_valid_from: Seconds,
		) -> DispatchResult {
			let aliases = match origin.into_caller().try_into() {
				Ok(Origin::Voter { aliases, .. }) => aliases,
				Err(_) => return Err(BadOrigin.into()),
			};

			Self::do_bestow(vote, aliases).map_err(Into::into)
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn do_bestow(vote: VoteData, aliases: VoteAliases) -> Result<(), Error<T>> {
			let now = T::Clock::now().as_secs();

			if let Some(old_point_info) = Points::<T>::take(aliases.point_alias) {
				Votes::<T>::remove(old_point_info.subject_alias);

				let old_subject = old_point_info.subject;
				let old_direction = old_point_info.direction;

				Pallet::<T>::deposit_event(Event::<T>::VoteReused {
					old_subject,
					old_direction,
					new_subject: vote.subject,
					new_direction: vote.direction,
				});

				let result = Self::tally(&old_subject, old_direction.opposite());

				Pallet::<T>::deposit_event(Event::<T>::HonourChanged {
					subject: old_subject,
					old_value: result.old_value,
					new_value: result.new_value,
				});
			} else {
				Pallet::<T>::deposit_event(Event::<T>::VoteCast {
					subject: vote.subject,
					direction: vote.direction,
				});
			}

			if Votes::<T>::contains_key(aliases.subject_alias) {
				return Err(Error::<T>::SubjectAlreadyVoted);
			}

			Votes::<T>::insert(aliases.subject_alias, ());

			Points::<T>::insert(
				aliases.point_alias,
				PointInfo {
					subject: vote.subject,
					subject_alias: aliases.subject_alias,
					direction: vote.direction,
					last_used_at: now,
				},
			);

			let result = Self::tally(&vote.subject, vote.direction);

			Pallet::<T>::deposit_event(Event::<T>::HonourChanged {
				subject: vote.subject,
				old_value: result.old_value,
				new_value: result.new_value,
			});

			Ok(())
		}

		/// Amend the tally of a subject without knowing whether it exists.
		fn tally(subject: &SubjectId, direction: Direction) -> TallyResult {
			let old_value = Tally::<T>::get(subject).unwrap_or(SUBJECT_DEFAULT_SCORE);
			let new_value = old_value.saturating_add(direction.score());

			if new_value == SUBJECT_DEFAULT_SCORE {
				Tally::<T>::remove(subject);
			} else {
				Tally::<T>::insert(subject, new_value);
			}

			TallyResult { old_value, new_value }
		}

		/// Check if a point is currently frozen.
		///
		/// A point is frozen if it was used within the last [`Config::PointFreezeDuration`] secs.
		pub(crate) fn is_point_frozen(point_alias: &PointAlias, now: Seconds) -> bool {
			if let Some(point) = Points::<T>::get(point_alias) {
				point.last_used_at.saturating_add(T::PointFreezeDuration::get()) > now
			} else {
				false
			}
		}

		/// Verify the ring proof and return the aliases corresponding to the vote.
		pub(crate) fn validate_vote_proof(
			vote: &VoteData,
			message: &[u8],
			proof: &RingProofOf<T>,
			ring_index: RingIndex,
			revision: RevisionIndex,
		) -> Result<VoteAliases, Error<T>> {
			let contexts = vote.get_contexts();

			let aliases = T::MemberService::verify_membership_multi_context_at_rev(
				PEOPLE_IDENTIFIER,
				proof,
				ring_index,
				revision,
				&contexts,
				message,
			)
			.map_err(|_| Error::<T>::InvalidProof)?;

			let arr: [ContextualAlias; 2] =
				aliases.try_into().map_err(|_: Vec<_>| Error::<T>::InvalidProof)?;

			// should be impossible, but check anyway
			if arr[0].context != contexts[0] || arr[1].context != contexts[1] {
				return Err(Error::<T>::InvalidProof);
			}

			Ok(VoteAliases::from_aliases([arr[0].alias, arr[1].alias]))
		}
	}

	impl<T: Config> inspect::Score for Pallet<T> {
		fn read(subject: &SubjectId) -> Honour {
			Tally::<T>::get(subject).unwrap_or(SUBJECT_DEFAULT_SCORE)
		}
	}

	struct TallyResult {
		old_value: Honour,
		new_value: Honour,
	}
}
