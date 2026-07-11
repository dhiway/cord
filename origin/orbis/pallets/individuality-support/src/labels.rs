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

//! Label (username) format validators.
//!
//! Mirrors `StringUtils._isDnsLabel` and `StringUtils.isLitePersonLabel` from
//! the `paritytech/dotns` contracts so that names produced on chain round-trip
//! into dotNS without format changes.

/// Minimum number of trailing digits required in a lite-person PoP label
/// suffix (the part after the `.`).
///
/// Must match `StringUtils.MIN_LITE_SUFFIX_DIGITS` on the contract side.
pub const MIN_LITE_USERNAME_DIGITS: usize = 2;

/// Returns whether `bytes` is a stricter "person" label: non-empty and only
/// lowercase ASCII letters `[a-z]` (no digits, no hyphens).
///
/// Used for full-person registrations.
pub fn is_person_label(bytes: &[u8]) -> bool {
	!bytes.is_empty() && bytes.iter().all(|b| b.is_ascii_lowercase())
}

/// Returns whether `bytes` matches the lite-person PoP label format
/// `<dns-stem>.<digits{MIN_LITE_USERNAME_DIGITS,}>` (e.g. `alice.42`).
///
/// Mirrors `StringUtils.isLitePersonLabel`: exactly one `.`, a DNS-valid
/// stem before it, and at least `MIN_LITE_USERNAME_DIGITS` ASCII digits after.
pub fn is_lite_person_label(bytes: &[u8]) -> bool {
	let length = bytes.len();
	// Shortest valid label is `x.NN` with `NN` being `MIN_LITE_USERNAME_DIGITS` digits.
	if length < MIN_LITE_USERNAME_DIGITS + 2 {
		return false;
	}

	// Finding the single `.` separator; rejecting multi-dot inputs.
	let mut dot_index = length;
	for (i, b) in bytes.iter().enumerate() {
		if *b == b'.' {
			if dot_index != length {
				return false;
			}
			dot_index = i;
		}
	}
	if dot_index == length {
		return false;
	}

	if !is_dns_label_range(bytes, 0, dot_index) {
		return false;
	}

	let suffix_start = dot_index + 1;
	if length - suffix_start < MIN_LITE_USERNAME_DIGITS {
		return false;
	}
	bytes[suffix_start..].iter().all(|b| b.is_ascii_digit())
}

/// Core DNS-label predicate over `bytes[start..end]`.
///
/// Mirrors `StringUtils._isDnsLabel`: the range must be non-empty, contain
/// only `[a-z0-9-]`, and start/end with a non-hyphen character.
fn is_dns_label_range(bytes: &[u8], start: usize, end: usize) -> bool {
	if end <= start {
		return false;
	}
	if bytes[start] == b'-' || bytes[end - 1] == b'-' {
		return false;
	}
	bytes[start..end]
		.iter()
		.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn person_label_accepts_lowercase_letters_only() {
		assert!(is_person_label(b"a"));
		assert!(is_person_label(b"alice"));
		assert!(is_person_label(b"bob"));
	}

	#[test]
	fn person_label_rejects_empty() {
		assert!(!is_person_label(b""));
	}

	#[test]
	fn person_label_rejects_digits_or_hyphens() {
		assert!(!is_person_label(b"alice42"));
		assert!(!is_person_label(b"alice-bob"));
		assert!(!is_person_label(b"123"));
		assert!(!is_person_label(b"-"));
	}

	#[test]
	fn person_label_rejects_uppercase_or_punctuation() {
		assert!(!is_person_label(b"Alice"));
		assert!(!is_person_label(b"alice.bob"));
		assert!(!is_person_label(b"ali_ce"));
		assert!(!is_person_label(b"ali ce"));
	}

	#[test]
	fn lite_person_label_accepts_valid() {
		assert!(is_lite_person_label(b"a.42"));
		assert!(is_lite_person_label(b"alice.42"));
		assert!(is_lite_person_label(b"alice.123456"));
		assert!(is_lite_person_label(b"a-b.99"));
		assert!(is_lite_person_label(b"123.99"));
	}

	#[test]
	fn lite_person_label_rejects_missing_or_short_suffix() {
		assert!(!is_lite_person_label(b"alice"));
		assert!(!is_lite_person_label(b"alice."));
		assert!(!is_lite_person_label(b"alice.1"));
	}

	#[test]
	fn lite_person_label_rejects_bad_suffix_characters() {
		assert!(!is_lite_person_label(b"alice.4a"));
		assert!(!is_lite_person_label(b"alice.-42"));
	}

	#[test]
	fn lite_person_label_rejects_multi_dot_or_empty_stem() {
		assert!(!is_lite_person_label(b"alice..42"));
		assert!(!is_lite_person_label(b"a.b.42"));
		assert!(!is_lite_person_label(b".42"));
	}

	#[test]
	fn lite_person_label_rejects_hyphen_adjacent_to_dot() {
		assert!(!is_lite_person_label(b"-alice.42"));
		assert!(!is_lite_person_label(b"alice-.42"));
	}

	#[test]
	fn lite_person_label_rejects_too_short() {
		assert!(!is_lite_person_label(b""));
		assert!(!is_lite_person_label(b"a"));
		assert!(!is_lite_person_label(b"a."));
		assert!(!is_lite_person_label(b".1"));
	}
}
