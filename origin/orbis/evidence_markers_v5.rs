// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test-only Slice 2 v5 evidence registry. Gate 6 is reserved, never emitted while pending.

pub const EVIDENCE_MARKERS_V5: &[(&str, &str, &str, &str)] = &[
	("slice2-surfaces", "S2-SURFACES-01", "Concrete direct and paid Meta Score/Honour surfaces preserve actor, nonce, payment, quota, business state, and rejection invariants.", "b4c8410c960d0ecb7148d29871877a53efbcc9f46a59cbfd086dd01142d4ea84"),
	("slice2-fixtures", "S2-FIXTURES-01", "Four checked compiled-metadata Score/Honour fixtures execute positive and exact mutation rejection paths.", "e2f5cfadab8e085d4675c8af155a9ed66c8ed36829a5406aee27d5ea09009a9f"),
	("slice2-payout", "S2-PAYOUT-01", "Payout-account rotation watches every liability and preserves atomic fund conservation.", "fdf76b39c2763e10e8c2d3ecbefbd7bd7c694e6db4a8058a4fb60c1a0e0b2c87"),
	("slice2-benchmark", "S2-BENCHMARK-01", "Runtime benchmarks register and execute Score/Honour and configured payout weight dominates the recorded Wasm measurement.", "e93e5ae882881dfb17ef5dea15df7381785a169957ae1e780184db39d4b31564"),
	("slice2-migrations", "S2-MIGRATIONS-01", "Full migration tuple accepts clean/current state and fails closed without partial progress for dirty Score or Honour v0.", "7dcbc8e654a8d3c95243f6ba6181fe4f0a907fd7b055c323ba7b0fa9d9c245f7"),
	("slice2-gate6", "GATE-6-SLICE2-EVIDENCE", "Independent Architect and Critic evidence reviews clear the bounded Slice 2 v5 evidence transition.", "9476759094595e6986b4b66233eda648252df74d6826c0cdee304b4c1e994deb"),
];

pub const RESERVED_GATE6_V5: (&str, &str, &str, &str) = EVIDENCE_MARKERS_V5[5];

pub fn emit_evidence_marker_v5(group: &str) {
	assert_ne!(group, RESERVED_GATE6_V5.0, "Gate 6 remains pending review");
	let matches = EVIDENCE_MARKERS_V5[..5]
		.iter()
		.filter(|row| row.0 == group)
		.collect::<alloc::vec::Vec<_>>();
	assert_eq!(matches.len(), 1, "each Slice 2 evidence command owns exactly one marker");
	let (_, id, contract, digest) = *matches[0];
	assert_eq!(
		sp_io::hashing::sha2_256(contract.as_bytes())
			.iter()
			.map(|byte| alloc::format!("{byte:02x}"))
			.collect::<alloc::string::String>(),
		digest
	);
	println!("assertion={id}:{digest}");
}
