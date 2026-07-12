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

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use indiv_support::traits::{Alias, PersonalId};
use scale_info::TypeInfo;
use sp_core::ConstU32;
use sp_runtime::BoundedVec;

/// A rolling bitfield tracking attendance over the last 8 games.
///
/// Each bit represents one game: 1 = attended, 0 = absent.
/// Bit 0 is the most recent game. On each new game the history is shifted left
/// and the new attendance flag is written into bit 0.
///
/// Defaults to `0xFF` (all attended) so that new participants are not
/// immediately penalised for games that occurred before they joined.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, Eq, PartialEq, Debug, Clone, Copy)]
pub struct AttendanceHistory(u8);

impl Default for AttendanceHistory {
	fn default() -> Self {
		// All bits set = assumed attended for every prior game.
		AttendanceHistory(0xFF)
	}
}

impl AttendanceHistory {
	/// Store the last game's attendance flag
	///
	/// Note: Only the most recent 8 attendances are tracked
	pub fn store_attendance(&mut self, attended: bool) {
		self.0 = (self.0 << 1) | u8::from(attended);
	}

	/// Count the number of missed games within the most recent `window` games.
	///
	/// `window` must not exceed 8
	pub fn misses_in_window(&self, window: u8) -> u8 {
		debug_assert!(window <= 8, "window {window} exceeds AttendanceHistory bit width of 8");
		let window = window.min(8);
		// Mask for the lowest `window` bits
		let mask: u8 = if window >= 8 { 0xFF } else { (1u8 << window) - 1 };
		let attended_bits = self.0 & mask;
		window.saturating_sub(attended_bits.count_ones() as u8)
	}
}

/// A streak of attendance or absence.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, Eq, PartialEq, Debug, Clone)]
pub enum Streak {
	/// The person has attended the given number of times.
	Attended(u32),
	/// The person has been absent the given number of times.
	Absent(u32),
}

impl Default for Streak {
	fn default() -> Self {
		Streak::Attended(0)
	}
}

impl Streak {
	/// Add to the attendance streak or switch to attendance if previously absent
	pub fn add_attendance(&mut self, amount: u32) {
		match self {
			Streak::Attended(count) => {
				*count = count.saturating_add(amount);
			},
			Streak::Absent(_) => {
				*self = Streak::Attended(amount);
			},
		}
	}

	/// Add to the absence streak or switch to absence if previously attended
	pub fn add_absence(&mut self, amount: u32) {
		match self {
			Streak::Absent(count) => {
				*count = count.saturating_add(amount);
			},
			Streak::Attended(_) => {
				*self = Streak::Absent(amount);
			},
		}
	}

	/// Get the number of consecutive attendances by the person.
	pub fn attendance(&self) -> u32 {
		match self {
			Streak::Attended(count) => *count,
			Streak::Absent(_) => 0,
		}
	}

	/// Get the number of consecutive absences by the person.
	pub fn absence(&self) -> u32 {
		match self {
			Streak::Absent(count) => *count,
			Streak::Attended(_) => 0,
		}
	}

	/// Used by test cases
	pub fn set_attendance_to_at_least(&mut self, count: u32) {
		*self = match self {
			Streak::Attended(c) => Streak::Attended((*c).max(count)),
			Streak::Absent(_) => Streak::Attended(count),
		};
	}

	/// Reset the attendance streak to 0.
	/// If it was in absent streak then no-op.
	pub fn reset_attendance(&mut self) {
		match self {
			Streak::Attended(_) => *self = Streak::Attended(0),
			Streak::Absent(_) => (),
		}
	}
}

/// A tier mapping a population size to a personhood score threshold.
///
/// Tiers are stored in ascending order by `population_size_threshold`. When
/// looking up the threshold for the current population size, the first tier
/// `active_participants <= population_size_threshold` is selected.
#[derive(
	Encode, Decode, MaxEncodedLen, TypeInfo, Eq, PartialEq, Debug, Clone, DecodeWithMemTracking,
)]
pub struct PersonhoodThresholdTier {
	/// Upper bound (inclusive) of the active participants for this tier.
	pub population_size_threshold: u32,
	/// The score required to achieve personhood at this population level.
	pub score_threshold: u32,
}

/// A bounded list of personhood-threshold tiers (at most
/// [`MAX_PERSONHOOD_THRESHOLD_TIERS`](crate::MAX_PERSONHOOD_THRESHOLD_TIERS)), sorted by ascending
/// `population_size_threshold`. Used as the storage type for the
/// runtime-configurable schedule and as the input to
/// `set_personhood_threshold_schedule`.
pub type PersonhoodThresholdTiers = BoundedVec<
	PersonhoodThresholdTier,
	ConstU32<{ crate::pallet::MAX_PERSONHOOD_THRESHOLD_TIERS }>,
>;

/// A single tier in the absence-grace schedule.
///
/// Tiers are stored in ascending order by population size threshold. When
/// looking up the ratio for the current population size, the first tier
/// `active_participants <= population_size_threshold` is selected.
///
/// `allowed_misses = 0` disables grace entirely: any absence immediately
/// suspends personhood.
#[derive(
	Encode, Decode, MaxEncodedLen, TypeInfo, Eq, PartialEq, Debug, Clone, DecodeWithMemTracking,
)]
pub struct AbsenceGraceTier {
	/// Upper bound (inclusive) of the active participants for this tier.
	pub population_size_threshold: u32,
	/// The number of recent games over which misses are counted.
	pub window: u8,
	/// Number of misses allowed within `window` games before suspending personhood.
	pub allowed_misses: u8,
}

/// A bounded list of up to 8 absence-grace tiers, sorted by ascending
/// `population_size_threshold`. Used as the storage type for the runtime-configurable
/// schedule and as the input to `set_absence_grace_schedule`.
pub type AbsenceGraceTiers = BoundedVec<AbsenceGraceTier, ConstU32<8>>;

// TODO: change score and streak to u8.
/// The participant informations.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, Eq, PartialEq, Debug)]
pub struct Participant<Balance> {
	/// Cumulative score of the participant determining personhood
	/// Persists across streak changes
	pub score: u32,
	/// The streak of the participant, referring to the current consecutive run of
	/// attendances/absences
	pub streak: Streak,
	/// Participant's attendance for recent games
	pub attendance_history: AttendanceHistory,
	/// The credit of the participant.
	pub credit: Balance,
	/// The participant has cashed out in this game, reset to false after next attendance.
	pub cashed_out: bool,
	/// Whether the participant has reached personhood score in its last attendance or onboarding.
	pub reached_personhood: bool,
	/// Whether the participant has ever reached personhood score.
	pub has_ever_reached_personhood: bool,
	/// The recognition status of the participant **in `People`**.
	pub recognition: Recognition,
	/// The game index of the last attended game if any.
	/// A non-attended game does not update this field.
	pub last_attended_game: Option<u32>,
}

/// The recognition status of a participant in `People`.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, Eq, PartialEq, Debug)]
pub enum Recognition {
	/// The participant is recognized through another DIM.
	ExternallyRecognized,
	/// The participant is not recognized in indiv-pallet-score.
	NotRecognized,
	/// The participant has been recognized in indiv-pallet-score, and has been suspended
	Suspended(PersonalId),
	/// The participant is recognized as a person by indiv-pallet-score.
	///
	/// A participant may have reached personhood score, it is not recognized until he submit his
	/// key and calls `register`.
	Recognized(PersonalId),
}

impl Recognition {
	pub fn is_recognized(&self) -> bool {
		matches!(self, Recognition::Recognized(_) | Recognition::ExternallyRecognized)
	}

	pub fn is_externally_recognized(&self) -> bool {
		matches!(self, Recognition::ExternallyRecognized)
	}
}

/// Describes the current reward payout round parameters.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub struct PayoutSchedule<Balance, BlockNumber> {
	/// The number of payout rounds remaining.
	pub remaining: u32,
	/// The funds used to fund each round.
	pub amount_per_round: Balance,
	/// The duration for each round.
	pub duration: BlockNumber,
}

/// The type for a round index.
pub type RoundIndex = u32;

/// An account or a person.
#[derive(
	PartialEq, Eq, Clone, Encode, Decode, Debug, TypeInfo, MaxEncodedLen, DecodeWithMemTracking,
)]
pub enum AccountOrPerson<AccountId> {
	/// An account.
	Account(AccountId),
	/// A person.
	Person(Alias),
}

impl<AccountId> AccountOrPerson<AccountId> {
	/// Get the account if it is an account.
	pub fn account(&self) -> Option<&AccountId> {
		match &self {
			AccountOrPerson::Account(account) => Some(account),
			AccountOrPerson::Person(_) => None,
		}
	}
}

/// The plannification of a round, points are accumulated in a different type.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub struct RoundInfo<BlockNumber, Balance> {
	/// When the round finishes and is ready to start paying out.
	pub finish_at: BlockNumber,
	/// The total number of credit for this round.
	pub credit: Balance,
}

/// The payout of a round. Points have already been collected, it is currently distributing the
/// credit according to points
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug, TypeInfo, MaxEncodedLen)]
pub struct RoundPayout<Balance> {
	/// The remaining balance for this round.
	pub remaining_balance: Balance,
	/// The credit associated for each point in this round (integer division result).
	pub point_price: Balance,
	/// The remainder from integer division of credit by total points.
	/// This is distributed proportionally to participants based on their points.
	pub remainder: Balance,
	/// The total points accumulated in this round (used for calculating remainder distribution).
	pub total_points: u32,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn default_is_all_attended() {
		let h = AttendanceHistory::default();
		assert_eq!(h.misses_in_window(8), 0);
		assert_eq!(h.misses_in_window(6), 0);
		assert_eq!(h.misses_in_window(1), 0);
	}

	#[test]
	fn single_miss_is_recorded() {
		let mut h = AttendanceHistory::default();
		h.store_attendance(false);
		assert_eq!(h.misses_in_window(1), 1);
		assert_eq!(h.misses_in_window(2), 1);
		assert_eq!(h.misses_in_window(8), 1);
	}

	#[test]
	fn attendance_after_miss_keeps_miss_in_window() {
		let mut h = AttendanceHistory::default();
		h.store_attendance(false); // game 1: miss
		h.store_attendance(true); // game 2: attend
							// Window of 2 should see 1 miss
		assert_eq!(h.misses_in_window(2), 1);
		// Window of 1 should see 0 misses (most recent was attended)
		assert_eq!(h.misses_in_window(1), 0);
	}

	#[test]
	fn miss_falls_out_of_window() {
		let mut h = AttendanceHistory::default();
		h.store_attendance(false); // game 1: miss
							 // Attend enough games to push the miss out of a window of 3
		h.store_attendance(true); // game 2
		h.store_attendance(true); // game 3
		h.store_attendance(true); // game 4
							// Window of 3 covers games 2,3,4 — all attended
		assert_eq!(h.misses_in_window(3), 0);
		// Window of 4 covers games 1,2,3,4 — 1 miss
		assert_eq!(h.misses_in_window(4), 1);
	}

	#[test]
	fn two_misses_in_window() {
		let mut h = AttendanceHistory::default();
		h.store_attendance(false); // miss
		h.store_attendance(true); // attend
		h.store_attendance(false); // miss
		assert_eq!(h.misses_in_window(3), 2);
		assert_eq!(h.misses_in_window(6), 2);
	}

	#[test]
	fn all_misses() {
		let mut h = AttendanceHistory::default();
		for _ in 0..8 {
			h.store_attendance(false);
		}
		assert_eq!(h.misses_in_window(8), 8);
		assert_eq!(h.misses_in_window(6), 6);
		assert_eq!(h.misses_in_window(1), 1);
	}

	#[test]
	fn window_zero_always_returns_zero() {
		let mut h = AttendanceHistory::default();
		h.store_attendance(false);
		assert_eq!(h.misses_in_window(0), 0);
	}

	#[test]
	#[cfg_attr(
		debug_assertions,
		should_panic = "window 100 exceeds AttendanceHistory bit width of 8"
	)]
	fn window_larger_than_eight_panics_in_debug() {
		let mut h = AttendanceHistory::default();
		h.store_attendance(false);
		h.misses_in_window(100);
	}

	#[test]
	fn ratio_one_miss_per_two_games() {
		let mut h = AttendanceHistory::default();
		let allowed_misses: u8 = 1;
		let window: u8 = 2;

		// Miss once — within allowance
		h.store_attendance(false);
		assert!(h.misses_in_window(window) <= allowed_misses);

		// Attend — resets the window
		h.store_attendance(true);
		assert!(h.misses_in_window(window) <= allowed_misses);

		// Miss again — window covers [attend, miss] = 1 miss, still OK
		h.store_attendance(false);
		assert!(h.misses_in_window(window) <= allowed_misses);
	}

	#[test]
	fn ratio_one_miss_per_six_triggers_on_second_miss() {
		let mut h = AttendanceHistory::default();
		let allowed_misses: u8 = 1;
		let window: u8 = 6;

		// Miss once in 6 — fine
		h.store_attendance(false);
		assert!(h.misses_in_window(window) <= allowed_misses);

		// Attend 3 then miss again — 2 misses in window of 6
		h.store_attendance(true);
		h.store_attendance(true);
		h.store_attendance(true);
		h.store_attendance(false);
		assert_eq!(h.misses_in_window(window), 2);
		assert!(h.misses_in_window(window) > allowed_misses);
	}

	#[test]
	fn consecutive_misses_trigger_immediately() {
		let mut h = AttendanceHistory::default();
		h.store_attendance(false);
		h.store_attendance(false);
		// Any window >= 2 sees 2 misses
		assert_eq!(h.misses_in_window(2), 2);
		assert_eq!(h.misses_in_window(6), 2);
	}
}
