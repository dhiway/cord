// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conservative weights for the bounded Orbis Meta v6 remediation.

use frame_support::weights::{RuntimeDbWeight, Weight};

pub const MAX_CALLS: u64 = 32;
pub const MAX_DEPTH: u64 = 4;
pub const MAX_BYTES: u64 = 65_536;
const BASE: u64 = 5_000_000;
const PER_CALL: u64 = 500_000;
const PER_DEPTH: u64 = 250_000;
const PER_64_BYTES: u64 = 25_000;
const MIRROR_DECODE_HASH: u64 = 4_000_000;
const CRYPTO_VERIFY: u64 = 8_000_000;
const HASH_OP: u64 = 500_000;
const PER_PROOF_BYTE: u64 = 10_000;

pub const fn inspector_ref_time(calls: u64, depth: u64, bytes: u64) -> u64 {
	BASE.saturating_add(PER_CALL.saturating_mul(calls))
		.saturating_add(PER_DEPTH.saturating_mul(depth))
		.saturating_add(PER_64_BYTES.saturating_mul(bytes.saturating_add(63) / 64))
		.saturating_add(MIRROR_DECODE_HASH)
}

pub fn paid_scope_max(db: RuntimeDbWeight) -> Weight {
	Weight::from_parts(inspector_ref_time(MAX_CALLS, MAX_DEPTH, MAX_BYTES), 0)
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
	Weight::from_parts(router_ref_time(reads, writes, crypto, hashes, proof_bytes), 0)
		.saturating_add(db.reads_writes(reads, writes))
}

pub fn malformed_max(db: RuntimeDbWeight) -> Weight {
	paid_scope_max(db).saturating_add(router(db, 6, 1, 1, 3, 32))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn maxima_and_db_ownership_are_exact() {
		let db = RuntimeDbWeight { read: 1_000, write: 2_000 };
		assert_eq!(
			paid_scope_max(db),
			Weight::from_parts(inspector_ref_time(32, 4, 65_536) + 6_000, 0)
		);
		assert_eq!(
			malformed_max(db),
			paid_scope_max(db).saturating_add(router(db, 6, 1, 1, 3, 32))
		);
	}
}
