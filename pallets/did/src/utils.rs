// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2021 BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

use crate::*;
use codec::Encode;
use sp_runtime::traits::Hash;
use sp_std::vec::Vec;

pub fn calculate_key_id<T: Config>(key: &DidPublicKey) -> KeyIdOf<T> {
	let hashed_values: Vec<u8> = key.encode();
	T::Hashing::hash(&hashed_values)
}

/// Verifies that an input string contains only URL-allowed ASCII characters.
/// For more info about what those characters are, please visit the official RFC
/// 3986.
pub fn is_valid_ascii_url(input: &str) -> bool {
	// Matches [0-9], [a-z], [A-Z], plus the symbols as in the RFC.
	input.chars().all(|c| {
		matches!(c, ':' | '/' | '?' | '#' | '[' | ']' | '@' | '!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';'
	| '=' | '-' | '.' | '_' | '~' | '%' | '0'..='9' | 'a'..='z' | 'A'..='Z')
	})
}

/// Verifies that an input string contains only Base-32 ASCII characters.
/// For more info about what those characters are, please visit the official RFC
/// 4648.
pub fn is_base_32(input: &str) -> bool {
	// Matches [A-Z], and [2-7].
	// At the moment, no check is performed the verify that padding characters are
	// only at the end of the char sequence.
	input.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7' | '='))
}

/// Verifies that an input string contains only Base-58 ASCII characters.
/// For more info about what those characters are, please visit the official
/// IETF draft.
pub fn is_base_58(input: &str) -> bool {
	// Matches [A-H], [J-N], [P-Z], [a-k], [m-z], and [1-9].
	input
		.chars()
		.all(|c| matches!(c, 'A'..='H' | 'J'..='N' | 'P'..='Z' | 'a'..='k' | 'm'..='z' | '1'..='9'))
}

#[test]
fn check_is_valid_ascii_url() {
	let test_cases = [
		("kilt.io", true),
		("super.long.domain.com:12345/path/to/directory#fragment?arg=value", true),
		("super.long.domain.com:12345/path/to/directory/file.txt", true),
		("domain.with.only.valid.characters.:/?#[]@!$&'()*+,;=-._~", true),
		("invalid.châracter.domain.org", false),
		("âinvalid.character.domain.org", false),
		("invalid.character.domain.orgâ", false),
		("", true),
		("kilt.io/<tag>/invalid_ascii.com", false),
		("<kilt.io/<tag>/invalid_ascii.com", false),
		("kilt.io/<tag>/invalid_ascii.com>", false),
		("kilt.io/%3Ctag%3E/encoded_upper_case_ascii.com", true),
		("kilt.io/%3ctag%3e/encoded_lower_case_ascii.com", true),
	];

	test_cases.iter().for_each(|(input, expected_result)| {
		assert_eq!(
			is_valid_ascii_url(input),
			*expected_result,
			"Test case for \"{}\" returned wrong result.",
			input
		);
	});
}

#[test]
fn check_is_base_32() {
	let test_cases = [
		("ABCDEFGHIJKLMNOPQRSTUVWXYZ", true),
		("234567", true),
		("ABCDEFGHIJKLMNOPQRSTUVWXYZ234567", true),
		("abcdefghijklmnopqrstuvwxyz", false),
		("", true),
		("1", false),
		("8", false),
		("9", false),
		("&", false),
		("‹", false),
		("a", false),
		("z", false),
		("1A", false),
		("A1", false),
		("8A", false),
		("A8", false),
		("9A", false),
		("A9", false),
		("&A", false),
		("A&", false),
		("‹A", false),
		("A‹", false),
		("aA", false),
		("Aa", false),
		("zA", false),
		("Az", false),
	];

	test_cases.iter().for_each(|(input, expected_result)| {
		assert_eq!(
			is_base_32(input),
			*expected_result,
			"Test case for \"{}\" returned wrong result.",
			input
		);
	});
}

#[test]
fn check_is_base_58() {
	let test_cases = [
		("ABCDEFGHJKLMNPQRSTUVWXYZ", true),
		("abcdefghijkmnopqrstuvwxyz", true),
		("123456789", true),
		("ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz123456789", true),
		("", true),
		("I", false),
		("O", false),
		("l", false),
		("0", false),
		("IA", false),
		("AI", false),
		("OA", false),
		("AO", false),
		("lC", false),
		("Al", false),
		("0C", false),
		("A0", false),
	];

	test_cases.iter().for_each(|(input, expected_result)| {
		assert_eq!(
			is_base_58(input),
			*expected_result,
			"Test case for \"{}\" returned wrong result.",
			input
		);
	});
}
