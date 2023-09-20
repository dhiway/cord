// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

use codec::Encode;
use fluent_uri::Uri;
use scale_info::prelude::format;
use sp_runtime::traits::Hash;
use sp_std::vec::Vec;

use crate::{did_details::DidPublicKey, AccountIdOf, Config, KeyIdOf};
// URI base used to test validity of provided service IDs (URI fragments).
const TEST_URI_BASE: &str = "did:cord:test-did";

pub fn calculate_key_id<T: Config>(key: &DidPublicKey<AccountIdOf<T>>) -> KeyIdOf<T> {
	let hashed_values: Vec<u8> = key.encode();
	T::Hashing::hash(&hashed_values)
}

/// Verifies that an input string contains only traditional (non-extended) ASCII
/// characters.
pub(crate) fn is_valid_ascii_string(input: &str) -> bool {
	input.chars().all(|c| c.is_ascii())
}

/// Verifies that an input is a valid URI according to W3C RFC3986.
pub(crate) fn is_valid_uri(input: &str) -> bool {
	Uri::parse(input).is_ok()
}

/// Verifies that an input string contains only characters allowed
/// for a URI fragment according to W3C RFC3986.
pub(crate) fn is_valid_uri_fragment(input: &str) -> bool {
	// We compose a valid prefix so that we can test if the provided input is a
	// valid fragment.
	let full_test_uri = format!("{}#{}", TEST_URI_BASE, input);
	Uri::parse(&full_test_uri).is_ok()
}

#[test]
fn check_is_valid_ascii_string() {
	let test_cases = [
		("dway.io", true),
		("super.long.domain.com:12345/path/to/directory#fragment?arg=value", true),
		("super.long.domain.com:12345/path/to/directory/file.txt", true),
		("domain.with.only.valid.characters.:/?#[]@!$&'()*+,;=-._~", true),
		("invalid.châracter.domain.org", false),
		("âinvalid.character.domain.org", false),
		("invalid.character.domain.orgâ", false),
		("", true),
		("गूगल.ट्रांसलेट.भारत", false),
		("dway.io/%3Ctag%3E/encoded_upper_case_ascii.com", true),
		("dway.io/%3ctag%3e/encoded_lower_case_ascii.com", true),
		("https://gist.github.com/keeguon/2310008", true),
	];

	test_cases.iter().for_each(|(input, expected_result)| {
		assert_eq!(
			is_valid_ascii_string(input),
			*expected_result,
			"Test case for \"{}\" returned wrong result.",
			input
		);
	});
}

#[test]
fn check_is_valid_uri() {
	let test_cases = [
		("dway.io", true),
		("super.long.domain.com:12345/path/to/directory#fragment?arg=value", true),
		("super.long.domain.com:12345/path/to/directory/file.txt", true),
		// Will fail because '[' it's an invalid character for a fragment (after the '#' symbol).
		("domain.with.only.valid.characters.:/?#[]@!$&'()*+,;=-._~", false),
		// Will fail because 'â' is an invalid URI character.
		("invalid.châracter.domain.org", false),
		// Will fail because 'â' is an invalid URI character.
		("âinvalid.character.domain.org", false),
		// Will fail because 'â' is an invalid URI character.
		("invalid.character.domain.orgâ", false),
		("", true),
		("गूगल.ट्रांसलेट.भारत", false),
		("dway.io/%3Ctag%3E/encoded_upper_case_ascii.com", true),
		("dway.io/%3ctag%3e/encoded_lower_case_ascii.com", true),
		("https://gist.github.com/keeguon/2310008", true),
	];

	test_cases.iter().for_each(|(input, expected_result)| {
		assert_eq!(
			is_valid_uri(input),
			*expected_result,
			"Test case for \"{}\" returned wrong result.",
			input
		);
	});
}

#[test]
fn check_is_valid_uri_fragment_string() {
	let test_cases = [
		("dway.io", true),
		// Will fail because a fragment cannot have a '#' inside it.
		("super.long.domain.com:12345/path/to/directory#fragment?arg=value", false),
		("super.long.domain.com:12345/path/to/directory/file.txt", true),
		// Will fail because a fragment cannot have a '#' inside it.
		("domain.with.only.valid.characters.:/?#[]@!$&'()*+,;=-._~", false),
		// Will fail because a fragment cannot have a 'â' inside it.
		("invalid.châracter.domain.org", false),
		// Will fail because a fragment cannot have a 'â' inside it.
		("âinvalid.character.domain.org", false),
		// Will fail because a fragment cannot have a 'â' inside it.
		("invalid.character.domain.orgâ", false),
		("", true),
		("गूगल.ट्रांसलेट.भारत", false),
		("dway.io/%3Ctag%3E/encoded_upper_case_ascii.com", true),
		("dway.io/%3ctag%3e/encoded_lower_case_ascii.com", true),
		("https://gist.github.com/keeguon/2310008", true),
	];

	test_cases.iter().for_each(|(input, expected_result)| {
		assert_eq!(
			is_valid_uri_fragment(input),
			*expected_result,
			"Test case for \"{}\" returned wrong result.",
			input
		);
	});
}
