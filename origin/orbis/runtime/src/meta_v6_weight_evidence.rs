// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Executable calibration evidence for the hand-maintained Meta v6 weights.
//!
//! This is deliberately feature-gated with the runtime benchmark harness, but it is not output
//! from FRAME's benchmark generator. It records the bounded workload owned by each router route
//! and checks that the weight selected by the production extension covers that workload. A future
//! generated benchmark may replace the conservative coefficients without changing these cases.

use frame_support::weights::{RuntimeDbWeight, Weight};

use crate::weights::meta_v6;

const BASE: u64 = 5_000_000;
const CLASSIFIER: u64 = 750_000;
const CRYPTO_VERIFY: u64 = 8_000_000;
const HASH_OP: u64 = 500_000;
const PER_MEMBERSHIP_PROOF_BYTE: u64 = 10_000;

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
	pub declared: Weight,
}

impl RouterCalibration {
	pub fn calibration_floor(self, db: RuntimeDbWeight) -> Weight {
		let cpu = BASE
			.saturating_add(CLASSIFIER)
			.saturating_add(CRYPTO_VERIFY.saturating_mul(self.crypto_verifications))
			.saturating_add(HASH_OP.saturating_mul(self.hash_operations))
			.saturating_add(PER_MEMBERSHIP_PROOF_BYTE.saturating_mul(self.membership_proof_bytes));
		Weight::from_parts(cpu, 0).saturating_add(db.reads_writes(self.reads, self.writes))
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
			declared: meta_v6::personal_alias(db),
		},
		RouterCalibration {
			name: "PersonalIdentityAccount",
			reads: 2,
			writes: 0,
			crypto_verifications: 0,
			hash_operations: 0,
			membership_proof_bytes: 0,
			declared: meta_v6::personal_identity(db),
		},
		RouterCalibration {
			name: "PersonalAliasAccountRevised",
			reads: 5,
			writes: 1,
			crypto_verifications: 1,
			hash_operations: 2,
			membership_proof_bytes: 32,
			declared: meta_v6::personal_alias_revised(db),
		},
		RouterCalibration {
			name: "LitePerson",
			reads: 1,
			writes: 0,
			crypto_verifications: 0,
			hash_operations: 0,
			membership_proof_bytes: 0,
			declared: meta_v6::lite_person(db),
		},
		RouterCalibration {
			name: "LiteAliasAccount",
			reads: 3,
			writes: 0,
			crypto_verifications: 0,
			hash_operations: 0,
			membership_proof_bytes: 0,
			declared: meta_v6::lite_alias(db),
		},
		RouterCalibration {
			name: "LiteAliasAccountRevised",
			reads: 5,
			writes: 1,
			crypto_verifications: 1,
			hash_operations: 2,
			membership_proof_bytes: 32,
			declared: meta_v6::lite_alias_revised(db),
		},
		RouterCalibration {
			name: "ClaimLongTermStorage",
			reads: 6,
			writes: 0,
			crypto_verifications: 1,
			hash_operations: 3,
			membership_proof_bytes: 32,
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
				"PersonalAliasAccount" |
					"PersonalIdentityAccount" |
					"PersonalAliasAccountRevised" |
					"LitePerson" | "LiteAliasAccount" |
					"LiteAliasAccountRevised"
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
			routes.into_iter().filter(|route| route.writes == 1).collect();
		assert_eq!(
			revised.iter().map(|route| route.name).collect::<alloc::vec::Vec<_>>(),
			["PersonalAliasAccountRevised", "LiteAliasAccountRevised"]
		);
		for route in revised {
			route.assert_declared_dominates(db);
			assert!(route.declared.all_gte(db.writes(1)));
		}

		let max_envelope = meta_v6::paid_scope_max(db);
		let expected = Weight::from_parts(
			meta_v6::inspector_ref_time(meta_v6::MAX_CALLS, meta_v6::MAX_DEPTH, meta_v6::MAX_BYTES),
			0,
		)
		.saturating_add(db.reads_writes(2, 2));
		assert!(max_envelope.all_gte(expected));
	}
}
