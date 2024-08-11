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

use super::*;
use crate::mock::*;
use codec::Encode;
use frame_support::{assert_err, assert_ok};

#[test]
fn creating_a_invalid_identifier_length_should_fail() {
	let space1 = [2u8; 1].to_vec();
	let space2 = [2u8; 50].to_vec();
	let space3 = [2u8; 30].to_vec();

	new_test_ext().execute_with(|| {
		assert_err!(
			Ss58Identifier::create_identifier(&(space1).encode()[..], IdentifierType::Space),
			IdentifierError::InvalidIdentifierLength
		);
		assert_err!(
			Ss58Identifier::create_identifier(&(space2).encode()[..], IdentifierType::Space),
			IdentifierError::InvalidIdentifierLength
		);
		assert_ok!(Ss58Identifier::create_identifier(
			&(space3).encode()[..],
			IdentifierType::Space
		));
	});
}
