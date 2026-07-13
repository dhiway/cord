// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test-only Slice 2 v5 evidence registry. Gate 6 is reserved, never emitted while pending.

pub const EVIDENCE_MARKERS_V5: &[(&str, &str, &str)] = &[
	(
		"slice2-surfaces",
		"S2-SURFACES-01",
		"1b41d987f8802a8deab44f53bde3e557721a6e178100ebe67034966fc6118214",
	),
	(
		"slice2-fixtures",
		"S2-FIXTURES-01",
		"038dacc96a878fb4c9d461d0a53006479031b43658b40fac76a982e56c1a9b8f",
	),
	(
		"slice2-payout",
		"S2-PAYOUT-01",
		"1f1be67bd9978efc827ede24ec4ae039185f4ae78d9860aa5f712d4f86b35a93",
	),
	(
		"slice2-benchmark",
		"S2-BENCHMARK-01",
		"2beaf4e53e181a95b167ca162bc4b4b758fbe466c832c70dc42b9d132e79d461",
	),
	(
		"slice2-migrations",
		"S2-MIGRATIONS-01",
		"86a06bce962c5f7c7b88b83c5f17607ee63e25bb23ab4f05e1359989f053faec",
	),
	(
		"slice2-gate6",
		"GATE-6-SLICE2-EVIDENCE",
		"473b3bad0f24383e1df54b80bae4148766565f4a034acb14de41ded1be3f6856",
	),
];

pub const RESERVED_GATE6_V5: (&str, &str, &str) = EVIDENCE_MARKERS_V5[5];

pub fn emit_evidence_marker_v5(group: &str) {
	assert_ne!(group, RESERVED_GATE6_V5.0, "Gate 6 remains pending review");
	let mut emitted = 0usize;
	for &(marker_group, id, digest) in &EVIDENCE_MARKERS_V5[..5] {
		if marker_group == group {
			println!("assertion={id}:{digest}");
			emitted += 1;
		}
	}
	assert_eq!(emitted, 1, "each Slice 2 evidence command owns exactly one marker");
}
