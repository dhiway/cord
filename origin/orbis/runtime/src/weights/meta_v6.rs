// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conservative weights for the bounded Orbis Meta v6 remediation.

use frame_support::weights::{RuntimeDbWeight, Weight};

pub const MAX_CALLS: u64 = 32;
pub const MAX_DEPTH: u64 = 4;
pub const METADATA_IMPLICIT_MAX_BYTES: u64 = 33;
pub const MAX_BYTES: u64 = 65_536;
pub const METADATA_IMPLICIT_WEIGHT_DELTA: u64 = 25_000;
const BASE: u64 = 5_000_000;
const PER_CALL: u64 = 500_000;
const PER_DEPTH: u64 = 250_000;
const PER_64_BYTES: u64 = 25_000;
const MIRROR_DECODE_HASH: u64 = 4_000_000;
// Lower-bounded by the generated proof-bearing People route (35,843,592,000 ps). This is
// intentionally conservative until Slice15 benchmarks the transaction extension in isolation.
const CRYPTO_VERIFY: u64 = 35_850_000_000;
const HASH_OP: u64 = 500_000;
const PER_PROOF_BYTE: u64 = 10_000;
const MAX_MEMBERSHIP_PROOF_BYTES: u64 = 1_267;
const MAX_PROOF_POV: u64 = 5_137;
const CLASSIFIER: u64 = 750_000;
// Distinct route overheads preserve the measured control-flow differences even where two
// routes happen to own the same number of database operations.
const PERSONAL_ALIAS_ROUTE: u64 = 310_000;
const PERSONAL_IDENTITY_ROUTE: u64 = 270_000;
const PERSONAL_ALIAS_REVISED_ROUTE: u64 = 490_000;
const LITE_PERSON_ROUTE: u64 = 190_000;
const LITE_ALIAS_ROUTE: u64 = 290_000;
const LITE_ALIAS_REVISED_ROUTE: u64 = 470_000;
const RESOURCES_CLAIM_ROUTE: u64 = 610_000;

pub const fn inspector_ref_time(calls: u64, depth: u64, bytes: u64) -> u64 {
	BASE.saturating_add(PER_CALL.saturating_mul(calls))
		.saturating_add(PER_DEPTH.saturating_mul(depth))
		.saturating_add(PER_64_BYTES.saturating_mul(bytes.saturating_add(63) / 64))
		.saturating_add(MIRROR_DECODE_HASH)
}

pub fn paid_scope_max(db: RuntimeDbWeight) -> Weight {
	Weight::from_parts(
		inspector_ref_time(MAX_CALLS, MAX_DEPTH, MAX_BYTES)
			.saturating_add(METADATA_IMPLICIT_WEIGHT_DELTA),
		0,
	)
		.saturating_add(db.reads_writes(2, 2))
}

pub const fn router_ref_time(
	reads: u64,
	writes: u64,
	crypto: u64,
	hashes: u64,
	proof_bytes: u64,
) -> u64 {
	BASE.saturating_add(CRYPTO_VERIFY.saturating_mul(crypto))
		.saturating_add(HASH_OP.saturating_mul(hashes))
		.saturating_add(PER_PROOF_BYTE.saturating_mul(proof_bytes))
		// DB is charged separately using the configured runtime database weight.
		.saturating_add(reads.saturating_mul(0))
		.saturating_add(writes.saturating_mul(0))
}

pub fn router(
	db: RuntimeDbWeight,
	reads: u64,
	writes: u64,
	crypto: u64,
	hashes: u64,
	proof_bytes: u64,
) -> Weight {
	Weight::from_parts(
		router_ref_time(reads, writes, crypto, hashes, proof_bytes),
		if crypto > 0 { MAX_PROOF_POV } else { 0 },
	)
	.saturating_add(db.reads_writes(reads, writes))
}

fn classified_router(
	db: RuntimeDbWeight,
	route_overhead: u64,
	reads: u64,
	writes: u64,
	crypto: u64,
	hashes: u64,
	proof_bytes: u64,
) -> Weight {
	Weight::from_parts(CLASSIFIER.saturating_add(route_overhead), 0).saturating_add(router(
		db,
		reads,
		writes,
		crypto,
		hashes,
		proof_bytes,
	))
}

pub fn none() -> Weight {
	Weight::from_parts(CLASSIFIER, 0)
}

pub fn personal_alias(db: RuntimeDbWeight) -> Weight {
	classified_router(db, PERSONAL_ALIAS_ROUTE, 3, 0, 0, 0, 0)
}

pub fn personal_identity(db: RuntimeDbWeight) -> Weight {
	classified_router(db, PERSONAL_IDENTITY_ROUTE, 2, 0, 0, 0, 0)
}

pub fn personal_alias_revised(db: RuntimeDbWeight) -> Weight {
	classified_router(db, PERSONAL_ALIAS_REVISED_ROUTE, 5, 2, 1, 2, MAX_MEMBERSHIP_PROOF_BYTES)
}

pub fn lite_person(db: RuntimeDbWeight) -> Weight {
	classified_router(db, LITE_PERSON_ROUTE, 1, 0, 0, 0, 0)
}

pub fn lite_alias(db: RuntimeDbWeight) -> Weight {
	classified_router(db, LITE_ALIAS_ROUTE, 3, 0, 0, 0, 0)
}

pub fn lite_alias_revised(db: RuntimeDbWeight) -> Weight {
	classified_router(db, LITE_ALIAS_REVISED_ROUTE, 5, 2, 1, 2, MAX_MEMBERSHIP_PROOF_BYTES)
}

pub fn resources_claim(db: RuntimeDbWeight) -> Weight {
	classified_router(db, RESOURCES_CLAIM_ROUTE, 6, 0, 1, 3, MAX_MEMBERSHIP_PROOF_BYTES)
}

pub fn malformed_max(db: RuntimeDbWeight) -> Weight {
	// Each route already consists of one classifier charge plus its route body. Taking their
	// maximum therefore gives exactly `classifier + max(route)`, without charging the classifier
	// twice or letting a malformed pair select a cheaper path.
	resources_claim(db).max(personal_alias_revised(db).max(lite_alias_revised(db)))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn maxima_and_db_ownership_are_exact() {
		let db = RuntimeDbWeight { read: 1_000, write: 2_000 };
		assert_eq!(
			paid_scope_max(db),
			Weight::from_parts(
				inspector_ref_time(32, 4, MAX_BYTES) + METADATA_IMPLICIT_WEIGHT_DELTA + 6_000,
				0,
			)
		);
		assert_eq!(MAX_BYTES, 65_536);
		assert_eq!(METADATA_IMPLICIT_MAX_BYTES, 33);
		assert_eq!(
			malformed_max(db),
			resources_claim(db).max(personal_alias_revised(db).max(lite_alias_revised(db)))
		);
	}

	#[test]
	fn all_seven_routes_are_distinct_and_malformed_dominates() {
		let db = RuntimeDbWeight { read: 1_000, write: 2_000 };
		let routes = [
			personal_alias(db),
			personal_identity(db),
			personal_alias_revised(db),
			lite_person(db),
			lite_alias(db),
			lite_alias_revised(db),
			resources_claim(db),
		];
		for (index, route) in routes.iter().enumerate() {
			assert!(routes.iter().skip(index + 1).all(|other| other != route));
			assert!(malformed_max(db).all_gte(*route));
		}
	}
}
