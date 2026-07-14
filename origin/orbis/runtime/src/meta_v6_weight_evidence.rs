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

//! Executable calibration evidence for the hand-maintained Meta v6 weights.
//!
//! This is deliberately feature-gated with the runtime benchmark harness, but it is not output
//! from FRAME's benchmark generator. It records the bounded workload owned by each router route
//! and checks that the weight selected by the production extension covers that workload. The
//! current benchmark setup is intentionally included and conservative. Slice15 must separate setup
//! from measured execution before generated extension weights replace these floors.

use frame_support::weights::{RuntimeDbWeight, Weight};

use crate::weights::meta_v6;

const BASE: u64 = 5_000_000;
const CLASSIFIER: u64 = 750_000;
const CRYPTO_VERIFY: u64 = 35_850_000_000;
const HASH_OP: u64 = 500_000;
const PER_MEMBERSHIP_PROOF_BYTE: u64 = 10_000;
pub const MAX_MEMBERSHIP_PROOF_BYTES: u64 = 1_267;
pub const MAX_STORAGE_POV_BYTES: u64 = 5_137;

/// The database and cryptographic workload exercised by a successful production route.
///
/// `membership_proof_bytes` is input processed by membership verification, not a claim about a
/// Substrate PoV proof-size measurement. The latter must come from generated FRAME benchmarks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouterCalibration {
	pub name: &'static str,
	pub reads: u64,
	pub writes: u64,
	pub crypto_verifications: u64,
	pub hash_operations: u64,
	pub membership_proof_bytes: u64,
	pub storage_pov_bytes: u64,
	pub declared: Weight,
}

impl RouterCalibration {
	pub fn calibration_floor(self, db: RuntimeDbWeight) -> Weight {
		let cpu = BASE
			.saturating_add(CLASSIFIER)
			.saturating_add(CRYPTO_VERIFY.saturating_mul(self.crypto_verifications))
			.saturating_add(HASH_OP.saturating_mul(self.hash_operations))
			.saturating_add(PER_MEMBERSHIP_PROOF_BYTE.saturating_mul(self.membership_proof_bytes));
		Weight::from_parts(cpu, self.storage_pov_bytes)
			.saturating_add(db.reads_writes(self.reads, self.writes))
	}

	pub fn assert_declared_dominates(self, db: RuntimeDbWeight) {
		assert!(
			self.declared.all_gte(self.calibration_floor(db)),
			"declared Meta v6 weight does not cover calibration workload for {}",
			self.name,
		);
	}
}

/// Produce evidence for every route accepted by `MetaAccountBoundPoliciesV6::classify`.
pub fn router_calibrations(db: RuntimeDbWeight) -> [RouterCalibration; 7] {
	[
		RouterCalibration {
			name: "PersonalAliasAccount",
			reads: 3,
			writes: 0,
			crypto_verifications: 0,
			hash_operations: 0,
			membership_proof_bytes: 0,
			storage_pov_bytes: 0,
			declared: meta_v6::personal_alias(db),
		},
		RouterCalibration {
			name: "PersonalIdentityAccount",
			reads: 2,
			writes: 0,
			crypto_verifications: 0,
			hash_operations: 0,
			membership_proof_bytes: 0,
			storage_pov_bytes: 0,
			declared: meta_v6::personal_identity(db),
		},
		RouterCalibration {
			name: "PersonalAliasAccountRevised",
			reads: 5,
			writes: 2,
			crypto_verifications: 1,
			hash_operations: 2,
			membership_proof_bytes: MAX_MEMBERSHIP_PROOF_BYTES,
			storage_pov_bytes: MAX_STORAGE_POV_BYTES,
			declared: meta_v6::personal_alias_revised(db),
		},
		RouterCalibration {
			name: "LitePerson",
			reads: 1,
			writes: 0,
			crypto_verifications: 0,
			hash_operations: 0,
			membership_proof_bytes: 0,
			storage_pov_bytes: 0,
			declared: meta_v6::lite_person(db),
		},
		RouterCalibration {
			name: "LiteAliasAccount",
			reads: 3,
			writes: 0,
			crypto_verifications: 0,
			hash_operations: 0,
			membership_proof_bytes: 0,
			storage_pov_bytes: 0,
			declared: meta_v6::lite_alias(db),
		},
		RouterCalibration {
			name: "LiteAliasAccountRevised",
			reads: 5,
			writes: 2,
			crypto_verifications: 1,
			hash_operations: 2,
			membership_proof_bytes: MAX_MEMBERSHIP_PROOF_BYTES,
			storage_pov_bytes: MAX_STORAGE_POV_BYTES,
			declared: meta_v6::lite_alias_revised(db),
		},
		RouterCalibration {
			name: "ClaimLongTermStorage",
			reads: 6,
			writes: 0,
			crypto_verifications: 1,
			hash_operations: 3,
			membership_proof_bytes: MAX_MEMBERSHIP_PROOF_BYTES,
			storage_pov_bytes: MAX_STORAGE_POV_BYTES,
			declared: meta_v6::resources_claim(db),
		},
	]
}

#[cfg(test)]
mod tests {
	use super::*;

	fn calibration_db() -> RuntimeDbWeight {
		// Unequal, non-zero terms ensure reads and writes cannot accidentally exchange ownership.
		RuntimeDbWeight { read: 1_000_000, write: 2_000_000 }
	}

	#[test]
	fn gate4_all_seven_router_weights_dominate_db_and_membership_proof_work() {
		let db = calibration_db();
		let routes = router_calibrations(db);
		assert_eq!(routes.len(), 7);
		for route in routes {
			route.assert_declared_dominates(db);
		}
	}

	#[test]
	fn gate4_malformed_and_mapping_miss_use_dominating_weights() {
		let db = calibration_db();
		let routes = router_calibrations(db);
		let malformed = meta_v6::malformed_max(db);
		for route in routes {
			assert!(malformed.all_gte(route.declared), "malformed weight missed {}", route.name);
		}

		// An absent account mapping exits after the first lookup. The selected route weight owns
		// the complete success path, and therefore conservatively covers this mapping-miss
		// rejection.
		let one_read_mapping_miss =
			Weight::from_parts(BASE.saturating_add(CLASSIFIER), 0).saturating_add(db.reads(1));
		for route in routes.into_iter().filter(|route| {
			matches!(
				route.name,
				"PersonalAliasAccount"
					| "PersonalIdentityAccount"
					| "PersonalAliasAccountRevised"
					| "LitePerson" | "LiteAliasAccount"
					| "LiteAliasAccountRevised"
			)
		}) {
			assert!(route.declared.all_gte(one_read_mapping_miss));
		}
	}

	#[test]
	fn gate4_revised_writes_and_max_envelope_are_explicitly_charged() {
		let db = calibration_db();
		let routes = router_calibrations(db);
		let revised: alloc::vec::Vec<_> =
			routes.into_iter().filter(|route| route.writes == 2).collect();
		assert_eq!(
			revised.iter().map(|route| route.name).collect::<alloc::vec::Vec<_>>(),
			["PersonalAliasAccountRevised", "LiteAliasAccountRevised"]
		);
		for route in revised {
			route.assert_declared_dominates(db);
			assert!(route.declared.all_gte(db.writes(2)));
		}

		let max_envelope = meta_v6::paid_scope_max(db);
		let expected = Weight::from_parts(
			meta_v6::inspector_ref_time(meta_v6::MAX_CALLS, meta_v6::MAX_DEPTH, meta_v6::MAX_BYTES)
				+ meta_v6::METADATA_IMPLICIT_WEIGHT_DELTA,
			0,
		)
		.saturating_add(db.reads_writes(2, 2));
		assert!(max_envelope.all_gte(expected));
	}

	#[test]
	fn gate4_production_crypto_totals_dominate_generated_proof_routes() {
		type ResourcesWeight = <crate::Runtime as indiv_pallet_resources::Config>::WeightInfo;
		type PeopleWeight = <crate::Runtime as indiv_pallet_people::Config>::WeightInfo;
		type LiteWeight = <crate::Runtime as indiv_pallet_people_lite::Config>::WeightInfo;

		let personal = <ResourcesWeight as indiv_pallet_resources::weights::WeightInfo>::
			meta_policy_personal_alias_revised();
		let personal_generated = <PeopleWeight as indiv_pallet_people::weights::WeightInfo>::
			as_person_alias_with_account_revised();
		assert!(personal.all_gte(personal_generated));

		let lite = <ResourcesWeight as indiv_pallet_resources::weights::WeightInfo>::
			meta_policy_lite_alias_revised();
		let lite_generated = <LiteWeight as indiv_pallet_people_lite::weights::WeightInfo>::
			as_lite_alias_with_account_revised_tx_ext();
		assert!(lite.all_gte(lite_generated));

		let resources_call = <ResourcesWeight as indiv_pallet_resources::weights::WeightInfo>::
			claim_long_term_storage_tx_ext();
		let resources_total = resources_call.saturating_add(
			<ResourcesWeight as indiv_pallet_resources::weights::WeightInfo>::
				meta_policy_resources_claim(),
		);
		assert!(resources_total.all_gte(resources_call));
		assert_eq!(resources_total.proof_size(), MAX_STORAGE_POV_BYTES);

		let malformed =
			<ResourcesWeight as indiv_pallet_resources::weights::WeightInfo>::meta_policy_malformed(
			);
		for selected in [personal, lite, resources_total] {
			assert!(malformed.all_gte(selected));
		}
	}
}
