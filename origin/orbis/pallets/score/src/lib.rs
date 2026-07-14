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

#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "128"]

extern crate alloc;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
mod extension;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod types;
pub mod weights;

pub use extension::{ScoreAsParticipant, ScoreAsParticipantData};
pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

use codec::Encode;
use frame_support::{
	storage::with_storage_layer,
	traits::{
		fungible::{Inspect, InspectHold, Mutate, MutateHold},
		tokens::{Precision, Preservation},
		Defensive, EnsureOriginWithArg, OriginTrait,
	},
	transactional,
};
use frame_system::{
	offchain::{CreateInherent, SubmitTransaction},
	EnsureSigned,
};
use indiv_support::traits::{AddOnlyPeopleTrait, Alias, Context, CountedMembers, PeopleTrait};
use sp_runtime::{
	traits::{BadOrigin, Zero},
	Saturating,
};
use types::Recognition::*;
use verifiable::GenerateVerifiable;
use xcm::v5::Location;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	pub const SCORE_CONTEXT: Context = *b"pop:polkadot.network/score      ";

	/// Upper bound on how many tiers a personhood-threshold schedule may contain.
	///
	/// This caps the [`PersonhoodThresholdTiers`] backing
	/// [`PersonhoodThresholdSchedule`]. Governance still validates schedules; a larger bound
	/// only increases maximum encoded size and worst-case validation work.
	pub const MAX_PERSONHOOD_THRESHOLD_TIERS: u32 = 16;

	const LOG_TARGET: &str = "runtime::indiv-pallet-score";

	/// The maximum value for the personhood threshold and participant score.
	/// The score is capped at this value.
	const MAX_PERSONHOOD_THRESHOLD: u32 = 21;

	/// Custom error code for unsigned transaction validation indicating the transaction is too
	/// far in the future.
	const CUSTOM_ERROR_FAR_FUTURE: u8 = 87;

	pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// Default personhood-threshold tiers, used as the storage default for
	/// [`PersonhoodThresholdSchedule`] until governance overrides them via
	/// [`Pallet::set_personhood_threshold_schedule`].
	///
	/// The last tier uses `u32::MAX` so it catches all population sizes above
	/// 50,000.
	#[pallet::type_value]
	pub fn DefaultPersonhoodThresholdTiers<T: Config>() -> PersonhoodThresholdTiers {
		BoundedVec::truncate_from(alloc::vec![
			PersonhoodThresholdTier { population_size_threshold: 5_000, score_threshold: 1 },
			PersonhoodThresholdTier { population_size_threshold: 10_000, score_threshold: 3 },
			PersonhoodThresholdTier { population_size_threshold: 20_000, score_threshold: 6 },
			PersonhoodThresholdTier { population_size_threshold: 35_000, score_threshold: 10 },
			PersonhoodThresholdTier { population_size_threshold: 50_000, score_threshold: 15 },
			PersonhoodThresholdTier {
				population_size_threshold: u32::MAX,
				score_threshold: MAX_PERSONHOOD_THRESHOLD,
			},
		])
	}

	/// Default absence-grace tiers, used as the storage default for
	/// [`AbsenceGraceSchedule`] until governance overrides them via
	/// [`Pallet::set_absence_grace_schedule`].
	///
	/// The last tier uses `u32::MAX` so it catches all population sizes above
	/// 50,000. At low population 5 misses are tolerated in a 6-game window,
	/// tightening as the network grows until at scale only 1 miss per 6 games
	/// is allowed (5-out-of-6 attendance).
	#[pallet::type_value]
	pub fn DefaultAbsenceGraceTiers<T: Config>() -> AbsenceGraceTiers {
		BoundedVec::truncate_from(alloc::vec![
			AbsenceGraceTier { population_size_threshold: 5_000, window: 6, allowed_misses: 5 },
			AbsenceGraceTier { population_size_threshold: 10_000, window: 5, allowed_misses: 4 },
			AbsenceGraceTier { population_size_threshold: 20_000, window: 4, allowed_misses: 3 },
			AbsenceGraceTier { population_size_threshold: 35_000, window: 3, allowed_misses: 2 },
			AbsenceGraceTier { population_size_threshold: 50_000, window: 2, allowed_misses: 1 },
			AbsenceGraceTier { population_size_threshold: u32::MAX, window: 6, allowed_misses: 1 },
		])
	}

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;
	pub type MemberOf<T> = <<T as Config>::People as AddOnlyPeopleTrait>::Member;

	#[pallet::config]
	pub trait Config:
		frame_system::Config<
			RuntimeOrigin: From<Origin<Self>> + OriginTrait<PalletsOrigin: TryInto<Origin<Self>>>,
		> + CreateInherent<Call<Self>>
		+ Send
		+ Sync
	{
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		/// Ensure origin is a person.
		type EnsurePerson: EnsureOriginWithArg<OriginFor<Self>, Context, Success = Alias>
			+ CountedMembers;

		/// Who to tell when we recognize or suspend personhood.
		type People: PeopleTrait;

		/// Initial enterprise payout account. Root may rotate it only when liabilities are zero.
		type PayoutAccountDefault: Get<Self::AccountId>;

		/// Currency used for cash out payout.
		type Currency: Inspect<Self::AccountId>
			+ Mutate<Self::AccountId>
			+ MutateHold<Self::AccountId, Reason: From<HoldReason>>;

		/// The location of the used currency. It is informational only.
		#[pallet::constant]
		type CurrencyLocationInfo: Get<Location>;

		/// The origin that can schedule payout rounds.
		type ManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Initial named enterprise manager. Sudo/root may rotate this account.
		type ManagerAccountDefault: Get<Option<Self::AccountId>>;

		/// Maximum number of payout schedules that can be registered.
		type MaxPayoutRoundSchedules: Get<u32>;

		/// The interval at which offchain worker runs.
		#[pallet::constant]
		type OffchainWorkInterval: Get<BlockNumberFor<Self>>;

		/// Trait allowing cryptographic operation for the member key.
		type Crypto: GenerateVerifiable<Member = MemberOf<Self>>;

		/// Additional configuration for benchmarking.
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: benchmarking::BenchmarkHelper<Self>;
	}

	#[pallet::origin]
	#[derive(
		CloneNoBound,
		PartialEqNoBound,
		EqNoBound,
		DebugNoBound,
		Encode,
		Decode,
		MaxEncodedLen,
		TypeInfo,
		DecodeWithMemTracking,
	)]
	#[scale_info(skip_type_params(T))]
	pub enum Origin<T: Config> {
		/// A participant using an account.
		AccountParticipant(T::AccountId),
	}

	#[pallet::extra_constants]
	impl<T: Config> Pallet<T> {
		/// Current named payout account.
		pub fn score_pot_id() -> T::AccountId {
			PayoutAccount::<T>::get()
		}

		/// The context used for the proofs required to authenticate as a personal alias in score
		/// pallet.
		pub fn score_context() -> Context {
			SCORE_CONTEXT
		}
	}

	/// The participants informations.
	#[pallet::storage]
	pub type Participants<T: Config> =
		StorageMap<_, Blake2_128Concat, AccountOrPerson<T::AccountId>, Participant<BalanceOf<T>>>;

	/// The score threshold required to reach personhood.
	#[pallet::storage]
	pub type PersonhoodThreshold<T> = StorageValue<_, u32, ValueQuery>;

	/// Runtime-configurable schedule of personhood-threshold tiers, sorted by
	/// ascending `population_size_threshold`. Each tier specifies the score a
	/// participant must reach to be recognized as a person at that population
	/// level.
	///
	/// Defaults to [`DefaultPersonhoodThresholdTiers`] until governance
	/// overrides it via [`Pallet::set_personhood_threshold_schedule`].
	///
	/// A new schedule takes effect at the start of the next report session
	/// when `update_thresholds()` recalculates `PersonhoodThreshold`.
	#[pallet::storage]
	pub type PersonhoodThresholdSchedule<T: Config> =
		StorageValue<_, PersonhoodThresholdTiers, ValueQuery, DefaultPersonhoodThresholdTiers<T>>;

	/// Runtime-configurable schedule of absence-grace tiers, sorted by ascending
	/// population size threshold. Each tier specifies how many misses are
	/// tolerated within recent games before a participant's personhood is
	/// suspended.
	///
	/// Defaults to [`DefaultAbsenceGraceTiers`] until governance overrides it
	/// via [`Pallet::set_absence_grace_schedule`]. An empty schedule (set
	/// explicitly by governance) disables the grace period entirely.
	#[pallet::storage]
	pub type AbsenceGraceSchedule<T: Config> =
		StorageValue<_, AbsenceGraceTiers, ValueQuery, DefaultAbsenceGraceTiers<T>>;

	/// The currently active absence-grace ratio `(allowed_misses, window)`,
	/// derived from `AbsenceGraceSchedule` and the current active-person count.
	///
	/// Updated each time `update_thresholds()` runs (at the start of every
	/// attendance report session). Read by `set_attendance` to decide whether
	/// a participant should be suspended.
	#[pallet::storage]
	pub type AbsenceGraceRatio<T> = StorageValue<_, (u8, u8), ValueQuery>;

	/// The accumulated points in the current round.
	// Could be merged in `Round` potentially.
	#[pallet::storage]
	pub type CurrentRoundPoints<T: Config> = StorageValue<_, u32, ValueQuery>;

	/// The index of the current round.
	///
	/// This must be the sum of all points accumulated in `RoundsPointsForParticipant` for the
	/// current round.
	///
	/// **WARNING**: This storage must be consistent with `RoundsPointsForParticipant`, use
	/// `fn add_points_to_participant` to update it.
	// Could be merged in `Round` potentially.
	#[pallet::storage]
	pub type CurrentRoundIndex<T> = StorageValue<_, RoundIndex, ValueQuery>;

	/// The points accumulated by a participant in a round.
	///
	/// For the current round, this storage must be updated alongside `CurrentRoundPoints`.
	///
	/// **WARNING**: This storage must be consistent with `CurrentRoundPoints`, use
	/// `fn add_points_to_participant` to update it.
	// Could be merged in `Participants` potentially.
	#[pallet::storage]
	pub type RoundsPointsForParticipant<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		RoundIndex,
		Blake2_128Concat,
		AccountOrPerson<T::AccountId>,
		u32,
		ValueQuery,
	>;

	/// The rounds that are currently paying out.
	///
	/// Rounds are paying out after they have finished accumulating points.
	/// When all their points have been paid out, they are removed from storage.
	#[pallet::storage]
	pub type RoundPayouts<T: Config> =
		StorageMap<_, Blake2_128Concat, RoundIndex, RoundPayout<BalanceOf<T>>>;

	/// The planning of the current round that is accumulating points.
	///
	/// If none the current round is ongoing and will be planned on the next schedule.
	#[pallet::storage]
	pub type RoundPlanning<T: Config> =
		StorageValue<_, RoundInfo<BlockNumberFor<T>, BalanceOf<T>>, OptionQuery>;

	/// Describes the schedules of the payout rounds.
	#[pallet::storage]
	pub type RoundSchedules<T> = StorageValue<
		_,
		BoundedVec<
			PayoutSchedule<BalanceOf<T>, BlockNumberFor<T>>,
			<T as Config>::MaxPayoutRoundSchedules,
		>,
		ValueQuery,
	>;

	/// Named enterprise account allowed to perform manager operations.
	#[pallet::storage]
	pub type ManagerAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	/// Named enterprise payout account used by every schedule, hold, and redemption path.
	#[pallet::type_value]
	pub fn DefaultPayoutAccount<T: Config>() -> T::AccountId {
		T::PayoutAccountDefault::get()
	}

	#[pallet::storage]
	pub type PayoutAccount<T: Config> =
		StorageValue<_, T::AccountId, ValueQuery, DefaultPayoutAccount<T>>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub manager_account: Option<T::AccountId>,
		pub payout_account: T::AccountId,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				manager_account: T::ManagerAccountDefault::get(),
				payout_account: T::PayoutAccountDefault::get(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			if let Some(manager) = &self.manager_account {
				ManagerAccount::<T>::put(manager);
			}
			PayoutAccount::<T>::put(&self.payout_account);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A person has claimed credit.
		CreditClaimed {
			/// The person who claimed credit.
			who: AccountOrPerson<T::AccountId>,
			/// Destination account that received the transfer.
			destination: T::AccountId,
			/// Amount transferred.
			amount: BalanceOf<T>,
		},
		/// Personhood was recognized for an account.
		PersonhoodRecognized {
			/// The account whose personhood was recognized.
			who: AccountOrPerson<T::AccountId>,
			/// Whether this was resuming from suspension or first-time recognition.
			resumed: bool,
		},
		/// Payout rounds have been scheduled.
		PayoutRoundsScheduled {
			/// The amount per round.
			amount: BalanceOf<T>,
			/// The number of rounds.
			count: u32,
			/// The duration per round in blocks.
			duration: BlockNumberFor<T>,
		},
		/// A payout schedule has been removed.
		PayoutScheduleRemoved {
			/// The index of the removed schedule.
			index: u32,
		},
		/// A round has been transitioned.
		RoundTransitioned {
			/// The round index that was transitioned.
			round_index: RoundIndex,
		},
		/// A payout round has been operated (credit distributed to participants).
		PayoutRoundOperated {
			/// The round index that was operated.
			round_index: RoundIndex,
		},
		/// A participant has cashed out score for points.
		CashedOut {
			/// The participant who cashed out.
			who: AccountOrPerson<T::AccountId>,
		},
		/// The personhood-threshold schedule has been set.
		PersonhoodThresholdScheduleSet,
		/// The absence-grace schedule has been set.
		AbsenceGraceScheduleSet,
		/// The named enterprise manager account has changed.
		ManagerAccountSet { manager: Option<T::AccountId> },
		/// The named payout account has changed after all prior funds were transferred.
		PayoutAccountSet { old: T::AccountId, new: T::AccountId, transferred: BalanceOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The calling origin is not a person.
		NotPerson,
		/// The person didn't reach personhood.
		HasNotReachedPersonhood,
		/// No reward available.
		NoReward,
		/// The person has no associated score.
		NoScore,
		/// A suspended participant cannot mutate Score state or claim Score value.
		ParticipantSuspended,
		/// Payout-account rotation is forbidden while any payout liability remains.
		PayoutAccountLiability,
		/// No payout schedule available.
		NoSchedule,
		/// Too many payout schedules already registered.
		TooManySchedules,
		/// The participant is recognized or has been recognized as a person.
		Recognized,
		/// The participant has already cashed out in this era.
		CashOutCooldown,
		/// The round is on going or no schedule.
		RoundOnGoingOrNoSchedule,
		/// The payout round has not started.
		NoRound,
		/// The origin is neither a person nor a signed account.
		BadOriginNotPersonNotSigned,
		/// The origin is neither a person nor a signed account nor an account participant.
		BadOriginNotPersonNotSignedNotAccountParticipant,
		/// The origin is neither a signed account nor an account participant.
		BadOriginNotSignedNotAccountParticipant,
		/// The participant is already participating.
		AlreadyParticipating,
		/// The key must be provided.
		KeyMustBeProvided,
		/// The key must not be provided.
		KeyMustNotBeProvided,
		/// Has reached personhood in the past.
		HasReachedPersonhood,
		/// The proof of ownership is invalid.
		InvalidProofOfOwnership,
		/// An absence grace tier has a window exceeding the maximum trackable history (8).
		WindowTooLarge,
		/// The allowed misses must be strictly less than the window (or both zero).
		AllowedMissesTooLarge,
		/// Absence-grace tiers must be sorted by ascending `population_size_threshold`.
		AbsenceScheduleNotSorted,
		/// The personhood-threshold schedule must contain at least one tier.
		PersonhoodScheduleEmpty,
		/// A personhood-threshold tier has `score_threshold == 0`.
		PersonhoodScoreThresholdZero,
		/// A personhood-threshold tier exceeds `MAX_PERSONHOOD_THRESHOLD`.
		PersonhoodScoreThresholdTooLarge,
		/// Personhood-threshold tiers must be sorted by ascending
		/// `population_size_threshold`.
		PersonhoodScheduleNotSorted,
		/// Personhood-threshold `score_threshold` values must be non-decreasing
		/// across tiers (a larger population must not have a lower bar).
		PersonhoodScheduleNotMonotonic,
		/// The last personhood-threshold tier must cover all populations
		/// (`population_size_threshold == u32::MAX`).
		PersonhoodScheduleNotTotal,
	}

	/// A reason for the pallet placing a hold on funds.
	///
	/// The pot account of the pallet will be funded by the treasury periodically (e.g. once every 6
	/// months) but less frequently than payout rounds will being initiated (e.g. once every 2
	/// weeks). At any given point in time, we expect to have a lot more funds in the pot account
	/// than we need for a given round. Funds made available for voters during a payout round are
	/// held for this `Payout` reason. When a voter claims funds from there, they are not yet theirs
	/// and remain in the pot account under the `Credit` reason until the payout transfer is
	/// triggered.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// The Pallet has reserved it for the current credit payout round.
		Payout,
		/// The Pallet has reserved it for storing users' credit until payout transfer is
		/// triggered.
		Credit,
	}

	/// Validates that personhood threshold tiers satisfy all invariants:
	/// - non-empty (curve must resolve a value for any population size)
	/// - per-tier: `0 < score_threshold <= MAX_PERSONHOOD_THRESHOLD`
	/// - tiers sorted ascending by `population_size_threshold`
	/// - `score_threshold` non-decreasing across tiers (larger populations cannot have a lower bar)
	/// - last tier covers all populations (`population_size_threshold == u32::MAX`) so the lookup
	///   is total
	fn validate_personhood_threshold_tiers<T>(
		tiers: &[PersonhoodThresholdTier],
	) -> Result<(), Error<T>> {
		ensure!(!tiers.is_empty(), Error::<T>::PersonhoodScheduleEmpty);

		let mut prev_pop = 0u32;
		let mut prev_score = 0u32;
		for tier in tiers {
			ensure!(tier.score_threshold > 0, Error::<T>::PersonhoodScoreThresholdZero);
			ensure!(
				tier.score_threshold <= MAX_PERSONHOOD_THRESHOLD,
				Error::<T>::PersonhoodScoreThresholdTooLarge,
			);
			ensure!(
				tier.population_size_threshold >= prev_pop,
				Error::<T>::PersonhoodScheduleNotSorted,
			);
			ensure!(tier.score_threshold >= prev_score, Error::<T>::PersonhoodScheduleNotMonotonic);
			prev_pop = tier.population_size_threshold;
			prev_score = tier.score_threshold;
		}

		ensure!(prev_pop == u32::MAX, Error::<T>::PersonhoodScheduleNotTotal);
		Ok(())
	}

	/// Validates that absence grace tiers satisfy all invariants:
	/// - window must not exceed 8 (AttendanceHistory bit width)
	/// - allowed_misses < window, or both must be zero
	/// - tiers must be sorted by ascending population_size_threshold
	fn validate_absence_grace_tiers<T>(tiers: &[AbsenceGraceTier]) -> Result<(), Error<T>> {
		let mut prev_threshold = 0u32;

		for tier in tiers {
			ensure!(tier.window <= 8, Error::<T>::WindowTooLarge);

			// (0, 0) is valid: no grace, any absence suspends.
			// Otherwise allowed_misses must be strictly less than window.
			ensure!(
				tier.allowed_misses < tier.window || (tier.window == 0 && tier.allowed_misses == 0),
				Error::<T>::AllowedMissesTooLarge,
			);

			ensure!(
				tier.population_size_threshold >= prev_threshold,
				Error::<T>::AbsenceScheduleNotSorted,
			);

			prev_threshold = tier.population_size_threshold;
		}

		Ok(())
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Performs payout round state machine
		///
		/// Transitions the current round to next one if needed. Then, operate all
		/// active payout rounds for up to 1000 participants, converting the
		/// accumulated points into credits.
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			let interval = T::OffchainWorkInterval::get();
			if interval == 0u32.into() || !(block_number % interval).is_zero() {
				return;
			}

			let current_round_index = CurrentRoundIndex::<T>::get();
			if Self::validate_transition_round(current_round_index).is_ok() {
				let res = Self::submit_unsigned_transaction(Call::transition_round {
					round_index: current_round_index,
				});
				Self::log_offchain_worker_tx_submit_result(res, "transition_round");
			}

			for (round_index, _) in RoundPayouts::<T>::iter() {
				if Self::validate_operate_payout_round(round_index).is_ok() {
					let res = Self::submit_unsigned_transaction(Call::operate_payout_round {
						round_index,
						limit: 1000,
					});
					Self::log_offchain_worker_tx_submit_result(res, "operate_payout_round");
				}
			}
		}

		fn integrity_test() {
			assert!(
				validate_personhood_threshold_tiers::<T>(
					&DefaultPersonhoodThresholdTiers::<T>::get()
				)
				.is_ok(),
				"invalid DefaultPersonhoodThresholdTiers"
			);

			assert!(
				validate_absence_grace_tiers::<T>(&DefaultAbsenceGraceTiers::<T>::get()).is_ok(),
				"invalid DefaultAbsenceGraceTiers"
			);
		}
	}

	fn build_transaction_validity(provides: impl Encode) -> TransactionValidity {
		ValidTransaction::with_tag_prefix("indiv-pallet-score")
			.and_provides(provides)
			.propagate(true)
			.build()
	}

	// TODO: Migrate to `#[pallet::authorize]` with `frame_system::AuthorizeCall` before
	// `ValidateUnsigned` is removed (deadline April 2027). See
	// https://github.com/paritytech/polkadot-sdk/issues/2415.
	#[pallet::validate_unsigned]
	#[allow(deprecated)]
	#[allow(clippy::let_unit_value)]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match call {
				Call::transition_round { round_index } => {
					Self::validate_transition_round(*round_index).map_err(|_| {
						let current_round_index = CurrentRoundIndex::<T>::get();
						if *round_index == current_round_index.saturating_add(1)
							|| *round_index == current_round_index
						{
							InvalidTransaction::Future
						} else if *round_index > current_round_index {
							InvalidTransaction::Custom(CUSTOM_ERROR_FAR_FUTURE)
						} else {
							InvalidTransaction::Stale
						}
					})?;
					build_transaction_validity(("TransitionRound", round_index))
				},
				Call::operate_payout_round { round_index, .. } => {
					Self::validate_operate_payout_round(*round_index).map_err(|_| {
						if *round_index == CurrentRoundIndex::<T>::get() {
							InvalidTransaction::Future
						} else if *round_index > CurrentRoundIndex::<T>::get() {
							InvalidTransaction::Custom(CUSTOM_ERROR_FAR_FUTURE)
						} else {
							InvalidTransaction::Stale
						}
					})?;
					build_transaction_validity(("PayoutRound", round_index))
				},
				_ => Err(TransactionValidityError::Invalid(InvalidTransaction::Call)),
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Schedule payout rounds.
		///
		/// Called from `ManagerOrigin` or root.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::schedule_payout_rounds())]
		pub fn schedule_payout_rounds(
			origin: OriginFor<T>,
			amount: BalanceOf<T>,
			count: u32,
			duration: BlockNumberFor<T>,
		) -> DispatchResultWithPostInfo {
			Self::ensure_manager(origin)?;
			let pot_account = Self::score_pot_id();
			// Funds to be paid out are held during the scheduled payout rounds. Any unclaimed
			// funds from last round will be recycled.
			T::Currency::hold(
				&HoldReason::Payout.into(),
				&pot_account,
				amount.saturating_mul(count.into()),
			)?;
			RoundSchedules::<T>::try_mutate(|schedules| {
				schedules
					.try_push(PayoutSchedule {
						remaining: count,
						amount_per_round: amount,
						duration,
					})
					.map_err(|_| Error::<T>::TooManySchedules)
			})?;
			Self::deposit_event(Event::PayoutRoundsScheduled { amount, count, duration });
			Ok(Pays::No.into())
		}

		/// Remove a scheduled payout round.
		///
		/// Called from `ManagerOrigin` or root.
		#[pallet::weight(T::WeightInfo::remove_payout_schedule())]
		#[pallet::call_index(2)]
		pub fn remove_payout_schedule(
			origin: OriginFor<T>,
			index: u32,
		) -> DispatchResultWithPostInfo {
			Self::ensure_manager(origin)?;
			let pot_account = Self::score_pot_id();
			let mut round_schedules = RoundSchedules::<T>::get();
			ensure!((index as usize) < round_schedules.len(), Error::<T>::NoSchedule);
			let schedule = round_schedules.remove(index as usize);
			let amount_to_release =
				schedule.amount_per_round.saturating_mul(schedule.remaining.into());
			T::Currency::release(
				&HoldReason::Payout.into(),
				&pot_account,
				amount_to_release,
				Precision::BestEffort,
			)?;
			RoundSchedules::<T>::put(round_schedules);
			Self::deposit_event(Event::PayoutScheduleRemoved { index });
			Ok(Pays::No.into())
		}

		/// Start a new round.
		///
		/// This is valid if the current round is finished, or if the current round doesn't have
		/// planning and a schedule exists to plan it.
		///
		/// This is a task, and can be called from anybody.
		#[pallet::call_index(3)]
		// The weight must include the cost of [ValidateUnsigned::pre_dispatch].
		#[pallet::weight(T::WeightInfo::transition_round())]
		pub fn transition_round(origin: OriginFor<T>, round_index: u32) -> DispatchResult {
			ensure_none(origin)?;

			Self::validate_transition_round(round_index)
				.defensive_proof(
					"indiv-pallet-score: transition round validation checked in pre-dispatch",
				)
				.map_err(|_| Error::<T>::RoundOnGoingOrNoSchedule)?;

			// All pre-conditions are met, start the operation, we can't fail after this point.

			// If planning for the current round exists, move that round to payout.
			if let Some(current_round) = RoundPlanning::<T>::take() {
				let round_accumulated_points = CurrentRoundPoints::<T>::take();
				let current_round_index = CurrentRoundIndex::<T>::get();
				let credit = current_round.credit;
				let points: BalanceOf<T> = round_accumulated_points.into();

				let point_price = credit.checked_div(&points).unwrap_or_default();

				// Calculate the remainder from integer division
				let total_from_point_price = point_price.saturating_mul(points);
				let remainder = credit.saturating_sub(total_from_point_price);

				RoundPayouts::<T>::insert(
					current_round_index,
					RoundPayout {
						remaining_balance: credit,
						point_price,
						remainder,
						total_points: round_accumulated_points,
					},
				);

				CurrentRoundIndex::<T>::put(current_round_index + 1);
			}

			// Plan the current round.
			let mut schedules = RoundSchedules::<T>::get();
			let first_schedule = schedules.get_mut(0);
			if let Some(first_schedule) = first_schedule {
				// Update the current schedule and set new round.
				first_schedule.remaining.saturating_dec();
				let round_amount = first_schedule.amount_per_round;

				RoundPlanning::<T>::put(RoundInfo {
					finish_at: frame_system::Pallet::<T>::block_number()
						.saturating_add(first_schedule.duration),
					credit: round_amount,
				});

				if first_schedule.remaining == 0 {
					// This will be a removal followed by a shift of all the remaining elements, but
					// we expect this list to be short.
					schedules.remove(0);
				}
				RoundSchedules::<T>::put(schedules);
			}

			Self::deposit_event(Event::RoundTransitioned { round_index });
			Ok(())
		}

		/// Operate some round paying out.
		///
		/// Drains round's participants, up to a limit. For each participant, add the calculated
		/// reward (`base_reward + remainder_portion`) to their `credit` balance.
		///
		/// Then, moves funds on the pot account from Payout to Credit, so they are owed to specific
		/// participants rather than the round pool.
		///
		/// Finally, release the leftover from the Payout hold back to the pot's free balance
		/// (funds are recycled).
		///
		/// This is a task, and can be called from anybody.
		///
		/// * round_index: The index of the round to operate.
		/// * limit: The maximum number of participants to operate in this call.
		#[pallet::call_index(4)]
		// The weight must take into account the cost of `ValidateUnsigned::pre_dispatch`.
		#[pallet::weight(T::WeightInfo::operate_payout_round(*limit))]
		pub fn operate_payout_round(
			origin: OriginFor<T>,
			round_index: RoundIndex,
			limit: u32,
		) -> DispatchResult {
			ensure_none(origin)?;

			let mut round = Self::validate_operate_payout_round(round_index)
				.defensive_proof(
					"indiv-pallet-score: operate payout round validation checked in pre-dispatch",
				)
				.map_err(|_| Error::<T>::NoRound)?;

			// All pre-conditions are met, start the operation, we can't fail after this point.

			// Release and hold operation can fail unexpectedly.
			// We protect against it by reverting the storage and setting the round as finished.
			let op_res = with_storage_layer::<_, DispatchError, _>(|| {
				// the total credited in this operation.
				let mut credited_in_op = BalanceOf::<T>::zero();
				let mut drain = RoundsPointsForParticipant::<T>::drain_prefix(round_index);
				let mut no_more_points = false;
				for _ in 0..limit {
					match drain.next() {
						Some((participant, points)) => {
							// Participant may have left the game, in this case they are skipped.
							if let Some(mut score) = Participants::<T>::get(&participant) {
								let base_reward = round.point_price.saturating_mul(points.into());

								// Distribute the remainder proportionally to points:
								// (remainder * points) / total_points
								let points_balance: BalanceOf<T> = points.into();
								let total_points_balance: BalanceOf<T> = round.total_points.into();
								let remainder_portion = round
									.remainder
									.saturating_mul(points_balance)
									.checked_div(&total_points_balance)
									.unwrap_or_default();

								let credited = base_reward.saturating_add(remainder_portion);
								score.credit = score.credit.saturating_add(credited);
								credited_in_op = credited_in_op.saturating_add(credited);

								Participants::<T>::insert(&participant, score);
							}
						},
						None => {
							no_more_points = true;
							break;
						},
					}
				}
				let pot = Self::score_pot_id();
				let released = T::Currency::release(
					&HoldReason::Payout.into(),
					&pot,
					credited_in_op,
					Precision::Exact,
				)?;
				defensive_assert!(credited_in_op == released);
				T::Currency::hold(&HoldReason::Credit.into(), &pot, released)?;

				round.remaining_balance = round
					.remaining_balance
					.checked_sub(&credited_in_op)
					.defensive_unwrap_or_default();

				if no_more_points {
					// Recycle funds and remove the round payout.
					let released = T::Currency::release(
						&HoldReason::Payout.into(),
						&pot,
						round.remaining_balance,
						Precision::Exact,
					)?;
					defensive_assert!(round.remaining_balance == released);
					RoundPayouts::<T>::remove(round_index);
				} else {
					// Continue the round payout.
					RoundPayouts::<T>::insert(round_index, round);
				}

				Ok(())
			});

			match op_res {
				Ok(()) => {
					Self::deposit_event(Event::PayoutRoundOperated { round_index });
					Ok(())
				},
				Err(e) => {
					defensive!("Unexpected error in this round, set the round finished.");
					log::error!(
						target: LOG_TARGET,
						"Unexpected error in operate_payout_round: round: {round_index:?}, error: {e:?}."
					);
					RoundPayouts::<T>::remove(round_index);
					Ok(())
				},
			}
		}

		/// Cash out half of the score, rounded up. Converts the score into points for the current
		/// payout round. Caller must have never reached personhood since onboarding.
		///
		/// It can be called once per game session (era).
		///
		/// Origin must be signed or participant (signed extrinsic using ScoreAsParticipant
		/// transaction extension)
		///
		/// Alias origin is not allowed as they can't cash out.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::cash_out())]
		pub fn cash_out(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = AccountOrPerson::Account(Self::ensure_signed_or_participant(origin)?);
			let mut score = Self::ensure_active_participant(&who)?;
			ensure!(!score.has_ever_reached_personhood, Error::<T>::HasReachedPersonhood);
			ensure!(!score.cashed_out, Error::<T>::CashOutCooldown);
			let reduction = score.score.saturating_add(1) / 2;
			score.score = score.score.saturating_sub(reduction);
			// Reset streak when they cash out.
			score.streak.reset_attendance();
			score.cashed_out = true;
			Self::award_participant_payout_points(&who, reduction);

			// Update score.
			Participants::<T>::insert(&who, score);
			Self::deposit_event(Event::CashedOut { who });
			Ok(Pays::No.into())
		}

		/// Redeem full accumulated credit, transferring it to the provided destination account.
		///
		/// Credit is converted from points during payout processing.
		///
		/// Origin must be a person alias, a signed account or a participant (signed extrinsic
		/// using ScoreAsParticipant transaction extension).
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::redeem_credit())]
		pub fn redeem_credit(
			origin: OriginFor<T>,
			destination: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let who = Self::ensure_signed_or_participant_or_person(origin)?;
			let mut score = Self::ensure_active_participant(&who)?;

			let pot = Self::score_pot_id();
			if score.credit.is_zero() {
				return Err(Error::<T>::NoReward.into());
			}
			let payout_amount = score.credit;
			score.credit = Zero::zero();

			let released = T::Currency::release(
				&HoldReason::Credit.into(),
				&pot,
				payout_amount,
				Precision::Exact,
			)?;
			debug_assert_eq!(payout_amount, released);

			let transferred =
				T::Currency::transfer(&pot, &destination, released, Preservation::Expendable)?;
			debug_assert_eq!(released, transferred);

			Participants::<T>::insert(&who, score);
			Self::deposit_event(Event::CreditClaimed { who, destination, amount: transferred });
			Ok(Pays::No.into())
		}

		/// Register as a person, or resume personhood after suspension.
		///
		/// Requires score >= personhood threshold (or having previously reached it).
		///
		/// If the participant was previously recognised and is now suspended, they must not provide
		/// a key. The existing key is reused.
		///
		/// If the participant was never recognised, they must provide a key and a proof of
		/// ownership (`key` parameter): a signature over `"pop register using" ||
		/// sender_account_id`.
		///
		/// Origin must be signed or a participant (signed extrinsic using ScoreAsParticipant
		/// transaction extension).
		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::register())]
		pub fn register(
			origin: OriginFor<T>,
			key: Option<(MemberOf<T>, <T::Crypto as GenerateVerifiable>::Signature)>,
		) -> DispatchResultWithPostInfo {
			let account = Self::ensure_signed_or_participant(origin)?;
			let who = AccountOrPerson::Account(account.clone());
			let mut score = Participants::<T>::get(&who).ok_or(Error::<T>::NoScore)?;

			ensure!(
				score.reached_personhood || score.score >= PersonhoodThreshold::<T>::get(),
				Error::<T>::HasNotReachedPersonhood
			);

			let (id, resumed) = match score.recognition {
				Suspended(id) => {
					if key.is_some() {
						return Err(Error::<T>::KeyMustNotBeProvided.into());
					}

					T::People::recognize_personhood(id, None)?;
					(id, true)
				},
				NotRecognized => {
					let Some((member_key, proof_of_ownership)) = key else {
						return Err(Error::<T>::KeyMustBeProvided.into());
					};

					let msg =
						account.using_encoded(|bytes| [&b"pop register using"[..], bytes].concat());
					ensure!(
						T::Crypto::verify_signature(&proof_of_ownership, &msg[..], &member_key),
						Error::<T>::InvalidProofOfOwnership
					);

					let id = T::People::reserve_new_id();
					T::People::recognize_personhood(id, Some(member_key))?;
					(id, false)
				},
				ExternallyRecognized | Recognized(_) => return Err(Error::<T>::Recognized.into()),
			};

			score.recognition = Recognized(id);
			score.reached_personhood = true;
			score.has_ever_reached_personhood = true;
			Participants::<T>::insert(&who, score);

			Self::deposit_event(Event::PersonhoodRecognized { who, resumed });

			Ok(Pays::No.into())
		}

		/// Set the absence grace schedule.
		///
		/// Every tier must satisfy: `0 <= window <= 8` and `allowed_misses < window`.
		/// `allowed_misses = 0` disables grace entirely (any absence immediately
		/// suspends).
		///
		/// Tiers must be provided in ascending order of `population_size_threshold`.
		///
		/// An empty schedule disables the grace period entirely (immediate
		/// suspension on any missed game). The new ratio takes effect at the start
		/// of the next report session when `update_thresholds()` recalculates
		/// `AbsenceGraceRatio`.
		///
		/// Called from `ManagerOrigin` or root.
		#[pallet::call_index(8)]
		#[pallet::weight(T::WeightInfo::set_absence_grace_schedule())]
		pub fn set_absence_grace_schedule(
			origin: OriginFor<T>,
			schedule: AbsenceGraceTiers,
		) -> DispatchResultWithPostInfo {
			Self::ensure_manager(origin)?;
			validate_absence_grace_tiers::<T>(&schedule)?;
			AbsenceGraceSchedule::<T>::put(schedule);
			Self::deposit_event(Event::AbsenceGraceScheduleSet);
			Ok(Pays::No.into())
		}

		/// Set the personhood-threshold schedule.
		///
		/// Tiers must be:
		/// - non-empty,
		/// - sorted ascending by `population_size_threshold`,
		/// - capped by a final tier with `population_size_threshold == u32::MAX`,
		/// - per-tier: `0 < score_threshold <= MAX_PERSONHOOD_THRESHOLD` (= 21),
		/// - have non-decreasing `score_threshold` across tiers.
		///
		/// The new curve takes effect at the start of the next report session
		/// when `update_thresholds()` recalculates `PersonhoodThreshold`.
		///
		/// Already-recognized participants are NOT retroactively suspended:
		/// the new bar only gates future score evaluations in `set_attendance`.
		///
		/// Called from `ManagerOrigin` or root.
		#[pallet::call_index(9)]
		#[pallet::weight(T::WeightInfo::set_personhood_threshold_schedule())]
		pub fn set_personhood_threshold_schedule(
			origin: OriginFor<T>,
			schedule: PersonhoodThresholdTiers,
		) -> DispatchResultWithPostInfo {
			Self::ensure_manager(origin)?;
			validate_personhood_threshold_tiers::<T>(&schedule)?;
			PersonhoodThresholdSchedule::<T>::put(schedule);
			Self::deposit_event(Event::PersonhoodThresholdScheduleSet);
			Ok(Pays::No.into())
		}

		/// Rotate or clear the named enterprise manager. Root/Sudo only.
		#[pallet::call_index(10)]
		#[pallet::weight(T::WeightInfo::set_manager_account())]
		pub fn set_manager_account(
			origin: OriginFor<T>,
			manager: Option<T::AccountId>,
		) -> DispatchResultWithPostInfo {
			T::ManagerOrigin::ensure_origin(origin)?;
			match &manager {
				Some(account) => ManagerAccount::<T>::put(account),
				None => ManagerAccount::<T>::kill(),
			}
			Self::deposit_event(Event::ManagerAccountSet { manager });
			Ok(Pays::No.into())
		}

		/// Rotate the named payout account after proving the old account has no liabilities.
		#[pallet::call_index(11)]
		#[pallet::weight(T::WeightInfo::set_payout_account())]
		#[transactional]
		pub fn set_payout_account(
			origin: OriginFor<T>,
			new: T::AccountId,
		) -> DispatchResultWithPostInfo {
			T::ManagerOrigin::ensure_origin(origin)?;
			let old = PayoutAccount::<T>::get();
			if old == new {
				return Ok(Pays::No.into());
			}
			ensure!(RoundSchedules::<T>::get().is_empty(), Error::<T>::PayoutAccountLiability);
			ensure!(RoundPlanning::<T>::get().is_none(), Error::<T>::PayoutAccountLiability);
			ensure!(
				RoundPayouts::<T>::iter_keys().next().is_none(),
				Error::<T>::PayoutAccountLiability
			);
			ensure!(CurrentRoundPoints::<T>::get() == 0, Error::<T>::PayoutAccountLiability);
			ensure!(
				T::Currency::balance_on_hold(&HoldReason::Payout.into(), &old).is_zero()
					&& T::Currency::balance_on_hold(&HoldReason::Credit.into(), &old).is_zero(),
				Error::<T>::PayoutAccountLiability
			);
			let transferred = T::Currency::total_balance(&old);
			if !transferred.is_zero() {
				T::Currency::transfer(&old, &new, transferred, Preservation::Expendable)?;
			}
			PayoutAccount::<T>::put(&new);
			Self::deposit_event(Event::PayoutAccountSet { old, new, transferred });
			Ok(Pays::No.into())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn ensure_active_participant(
			who: &AccountOrPerson<T::AccountId>,
		) -> Result<Participant<BalanceOf<T>>, DispatchError> {
			let participant = Participants::<T>::get(who).ok_or(Error::<T>::NoScore)?;
			ensure!(
				!matches!(participant.recognition, Suspended(_)),
				Error::<T>::ParticipantSuspended
			);
			Ok(participant)
		}

		fn ensure_manager(origin: OriginFor<T>) -> DispatchResult {
			match T::ManagerOrigin::try_origin(origin) {
				Ok(_) => Ok(()),
				Err(origin) => {
					let who = ensure_signed(origin)?;
					ensure!(ManagerAccount::<T>::get().as_ref() == Some(&who), BadOrigin);
					Ok(())
				},
			}
		}

		/// Whether account can onboard for recognition.
		///
		/// Fails if the account is already participating.
		pub fn can_onboard_for_recognition(who: &T::AccountId) -> Result<(), DispatchError> {
			ensure!(
				!Participants::<T>::contains_key(AccountOrPerson::Account(who.clone())),
				Error::<T>::AlreadyParticipating
			);
			Ok(())
		}

		/// Onboard a new account for recognition.
		///
		/// Fails if the account is already participating.
		pub fn onboard_for_recognition(who: &T::AccountId) -> Result<(), DispatchError> {
			Self::can_onboard_for_recognition(who)?;
			frame_system::Pallet::<T>::inc_sufficients(who);
			Participants::<T>::insert(
				AccountOrPerson::Account(who.clone()),
				Participant {
					score: 0,
					credit: 0u32.into(),
					streak: Streak::default(),
					attendance_history: AttendanceHistory::default(),
					recognition: NotRecognized,
					cashed_out: false,
					reached_personhood: false,
					has_ever_reached_personhood: false,
					last_attended_game: None,
				},
			);

			Ok(())
		}

		/// Onboard a new externally recognized person.
		///
		/// Fails if the person is already participating.
		pub fn onboard_externally_recognized(who: &Alias) -> Result<(), DispatchError> {
			ensure!(
				!Participants::<T>::contains_key(AccountOrPerson::Person(*who)),
				Error::<T>::AlreadyParticipating
			);
			Participants::<T>::insert(
				AccountOrPerson::Person(*who),
				Participant {
					score: PersonhoodThreshold::<T>::get(),
					credit: 0u32.into(),
					streak: Streak::default(),
					attendance_history: AttendanceHistory::default(),
					recognition: ExternallyRecognized,
					cashed_out: false,
					reached_personhood: true,
					has_ever_reached_personhood: true,
					last_attended_game: None,
				},
			);

			Ok(())
		}

		/// Offboard a participant, e.g. voluntarily leaves or is kicked out for inactivity.
		pub fn offboard(who: &AccountOrPerson<T::AccountId>) {
			if let AccountOrPerson::Account(account) = who {
				frame_system::Pallet::<T>::dec_sufficients(account);
			}
			Participants::<T>::remove(who);
		}

		/// Start a new attendance report session.
		///
		/// Updates the personhood threshold depending on the number of active people. Then, locks
		/// the people set for mutation.
		///
		/// Afterwards, [Self::set_attendance] can be called for participants.
		///
		/// This is a semaphore, multiple sessions can happen concurrently. It can fail if the state
		/// doesn't allow to start a session for the moment.
		pub fn start_attendance_report_session() -> DispatchResult {
			Self::update_thresholds();
			T::People::start_people_set_mutation_session()
		}

		/// End the attendance report session.
		///
		/// This is a semaphore, multiple sessions can happen concurrently. It can fail if there
		/// is no session ongoing.
		pub fn end_attendance_report_session() -> DispatchResult {
			T::People::end_people_set_mutation_session()
		}

		/// Return whether the attendance report session can be started.
		///
		/// The result of this operation holds until any call to `start_attendance_report_session`,
		/// `end_attendance_report_session` or `People` implementation is made.
		pub fn can_start_attendance_report_session() -> bool {
			T::People::can_start_people_set_mutation_session()
		}

		/// The participant reached personhood score at their last attendance or onboarding.
		pub fn reached_personhood(who: &AccountOrPerson<T::AccountId>) -> bool {
			Participants::<T>::get(who).is_some_and(|p| p.reached_personhood)
		}

		/// Set the attendance of a participant for `game_index`.
		///
		/// Updates scores, streaks and recognition. When `attended` is `true`,
		/// `last_attended_game` is pinned to `game_index`; when `false`, `last_attended_game`
		/// is left unchanged so it continues to point at the participant's most recent
		/// actual attendance.
		///
		/// Must be called within attendance report session. Attendance report session is started
		/// and ended with `start_attendance_report_session` and `end_attendance_report_session`.
		/// Multiple sessions can happen concurrently, it is implemented as a semaphore.
		#[transactional]
		pub fn set_attendance(
			who: &AccountOrPerson<T::AccountId>,
			attended: bool,
			game_index: u32,
		) -> Result<Participant<BalanceOf<T>>, DispatchError> {
			let mut score = Self::ensure_active_participant(who)?;

			let personhood_threshold = PersonhoodThreshold::<T>::get();

			// If they are externally recognized, their score is always set to the threshold.
			if score.recognition.is_externally_recognized() {
				score.score = personhood_threshold;
			}

			// Update last_attended_game.
			if attended {
				score.last_attended_game = Some(game_index);
			}

			// Update score and streak given the attendance.
			if attended {
				score.streak.add_attendance(1);
				let uncapped_score = score.score.saturating_add(score.streak.attendance());
				// Cap score at maximum personhood threshold.
				score.score = uncapped_score.min(MAX_PERSONHOOD_THRESHOLD);
			} else {
				score.streak.add_absence(1);
				// Externally recognized persons do not lose score.
				if !score.recognition.is_externally_recognized() {
					let penalty = score.streak.absence();
					score.score = score.score.saturating_sub(penalty);
				}
			}

			// Record attendance in the rolling history (used for ratio-based grace).
			score.attendance_history.store_attendance(attended);

			let (allowed_misses, window) = AbsenceGraceRatio::<T>::get();
			let misses = score.attendance_history.misses_in_window(window);

			let acquire_personhood =
				!score.reached_personhood && score.score >= personhood_threshold;

			let suspend_personhood = !attended
				&& score.reached_personhood
				&& match score.recognition {
					// Participants never lose personhood once they are externally recognised or
					// suspended
					ExternallyRecognized | Suspended(_) => false,

					// The rolling window governs when recognised persons are suspended:
					// exceeding the allowed misses within the window triggers suspension.
					// A window of 0 (with allowed_misses 0) means no grace: any absence
					// immediately suspends.
					Recognized(_) => window == 0 || misses > allowed_misses,

					// NotRecognized participants additionally lose personhood when their
					// score drops below threshold, otherwise a stale flag would let them
					// call `register()` later when the threshold has increased —
					// bypassing it entirely.
					NotRecognized => {
						window == 0 || misses > allowed_misses || score.score < personhood_threshold
					},
				};

			if acquire_personhood {
				score.reached_personhood = true;
			} else if suspend_personhood {
				score.reached_personhood = false;
			}

			score.has_ever_reached_personhood |= score.reached_personhood;

			score.cashed_out = false;

			match score.recognition {
				Recognized(id) => {
					if suspend_personhood {
						let _ = T::People::suspend_personhood(&[id])
							.defensive_proof("indiv-pallet-score: failed to suspend person");
						score.recognition = Suspended(id);
					}
				},
				NotRecognized | Suspended(_) | ExternallyRecognized => (),
			}

			Participants::<T>::insert(who, &score);

			Ok(score)
		}

		/// Ensure the origin is either a person or a signed account.
		pub fn ensure_signed_or_person(
			origin: OriginFor<T>,
		) -> Result<AccountOrPerson<T::AccountId>, DispatchError> {
			<EnsureSigned<_> as EnsureOrigin<_>>::try_origin(origin)
				.map(AccountOrPerson::Account)
				.or_else(|origin| {
					T::EnsurePerson::try_origin(origin, &SCORE_CONTEXT)
						.map(AccountOrPerson::Person)
						.map_err(|_| Error::<T>::BadOriginNotPersonNotSigned.into())
				})
		}

		/// Ensure the origin is either a person or a signed account or a participant account.
		pub fn ensure_signed_or_participant_or_person(
			origin: OriginFor<T>,
		) -> Result<AccountOrPerson<T::AccountId>, DispatchError> {
			T::EnsurePerson::try_origin(origin, &SCORE_CONTEXT)
				.map(AccountOrPerson::Person)
				.or_else(|origin| {
					Self::ensure_signed_or_participant(origin)
						.map(AccountOrPerson::Account)
						.map_err(|_| {
							Error::<T>::BadOriginNotPersonNotSignedNotAccountParticipant.into()
						})
				})
		}

		/// Ensure the origin is either a signed account or a participant account.
		pub fn ensure_signed_or_participant(
			origin: OriginFor<T>,
		) -> Result<T::AccountId, DispatchError> {
			<EnsureSigned<_> as EnsureOrigin<_>>::try_origin(origin).or_else(|origin| match origin
				.into_caller()
				.try_into()
			{
				Ok(Origin::AccountParticipant(account)) => Ok(account),
				_ => Err(Error::<T>::BadOriginNotSignedNotAccountParticipant.into()),
			})
		}

		/// Ensure the origin is a person.
		pub fn ensure_person(origin: OriginFor<T>) -> Result<Alias, DispatchError> {
			Ok(T::EnsurePerson::ensure_origin(origin, &SCORE_CONTEXT)?)
		}

		/// Validates the unsigned transaction for [Self::transition_round].
		fn validate_transition_round(round_index: u32) -> Result<(), ()> {
			if round_index != CurrentRoundIndex::<T>::get() {
				return Err(());
			}

			let current_round = RoundPlanning::<T>::get();
			let end_current_round = current_round
				.as_ref()
				.is_some_and(|r| frame_system::Pallet::<T>::block_number() >= r.finish_at);
			let schedule_exists =
				RoundSchedules::<T>::get().first().map(|s| s.remaining > 0).unwrap_or(false);
			let start_new_round_planning = current_round.is_none() && schedule_exists;

			if end_current_round || start_new_round_planning {
				Ok(())
			} else {
				Err(())
			}
		}

		// Validation of the unsigned transaction for the call `operate_payout_round`.
		fn validate_operate_payout_round(
			round_index: RoundIndex,
		) -> Result<RoundPayout<BalanceOf<T>>, ()> {
			RoundPayouts::<T>::get(round_index).ok_or(())
		}

		fn log_offchain_worker_tx_submit_result(res: Result<(), ()>, operation: &str) {
			match res {
				Ok(_) => {
					log::info!(target: LOG_TARGET, "offchain_worker - {operation} transaction submitted")
				},
				Err(e) => {
					log::error!(target: LOG_TARGET, "offchain_worker - failed to submit {operation} transaction: {e:?}")
				},
			}
		}

		fn submit_unsigned_transaction(call: Call<T>) -> Result<(), ()> {
			let xt = T::create_bare(call.into());
			SubmitTransaction::<T, Call<T>>::submit_transaction(xt)
		}

		/// Add points to a participant for the current round.
		fn award_participant_payout_points(who: &AccountOrPerson<T::AccountId>, points: u32) {
			if points == 0 {
				return;
			}

			let round_index = CurrentRoundIndex::<T>::get();
			let round_points = CurrentRoundPoints::<T>::get();
			if let Some(new_round_points) = round_points.checked_add(points) {
				RoundsPointsForParticipant::<T>::mutate(round_index, who, |p| {
					*p = p.saturating_add(points);
				});
				CurrentRoundPoints::<T>::put(new_round_points);
			} else {
				log::warn!(target: LOG_TARGET, "indiv-pallet-score: round points overflowed");
			}
		}

		/// Calculate the personhood threshold based on the number of active people.
		///
		/// Reads [`PersonhoodThresholdSchedule`], which defaults to
		/// [`DefaultPersonhoodThresholdTiers`] when unset. The validator
		/// guarantees the configured schedule is total (last tier covers
		/// `u32::MAX`), so the `unwrap_or` defensively catches an empty /
		/// malformed schedule that somehow bypassed validation.
		pub(crate) fn calculate_personhood_threshold(active_count: u32) -> u32 {
			PersonhoodThresholdSchedule::<T>::get()
				.into_iter()
				.find(|t| active_count <= t.population_size_threshold)
				.map(|t| t.score_threshold)
				.unwrap_or(MAX_PERSONHOOD_THRESHOLD)
		}

		/// Look up the absence-grace ratio `(allowed_misses, window)` for the
		/// given the number of active participants.
		///
		/// Reads [`AbsenceGraceSchedule`], which defaults to
		/// [`DefaultAbsenceGraceTiers`] when unset. An empty schedule (only
		/// reachable via an explicit governance call) means no grace period,
		/// i.e. `(0, 0)`.
		pub(crate) fn calculate_absence_grace_ratio(active_count: u32) -> (u8, u8) {
			let tiers = AbsenceGraceSchedule::<T>::get();

			if tiers.is_empty() {
				return (0, 0);
			}

			let tier = tiers
				.iter()
				.find(|t| active_count <= t.population_size_threshold)
				// If active_count exceeds all tier thresholds, fall back to the most
				// permissive tier as a catch-all.
				.or_else(|| tiers.last())
				.unwrap_or(&AbsenceGraceTier {
					population_size_threshold: u32::MAX,
					window: 1,
					allowed_misses: 0,
				});

			(tier.allowed_misses, tier.window)
		}

		/// Update the personhood threshold and absence grace ratio based on the
		/// number of active people.
		fn update_thresholds() {
			let active_count = T::EnsurePerson::active_count();

			let personhood_threshold = Self::calculate_personhood_threshold(active_count);
			let (allowed_misses, window) = Self::calculate_absence_grace_ratio(active_count);

			PersonhoodThreshold::<T>::put(personhood_threshold);
			AbsenceGraceRatio::<T>::put((allowed_misses, window));

			log::info!(
				target: LOG_TARGET,
				"personhood threshold: {personhood_threshold}, absence grace: {allowed_misses} misses in {window} games, active people: {active_count}"
			);
		}
	}
}
