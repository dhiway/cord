// This file is part of Cord – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// Cord is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cord is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cord. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use crate::*;
use base58::{FromBase58, ToBase58};
use blake2_rfc::blake2b::{Blake2b, Blake2bResult};
use frame_support::ensure;
use scale_info::prelude::string::String;
use sp_std::{fmt::Debug, prelude::Clone, str, vec};

/// CORD Identifier Prefix
const PREFIX: &[u8] = b"IDFRPRE";

// The Result of the signature verification.
pub type IdentifierVerificationResult = Result<(), IdentifierVerificationError>;

/// An error with the interpretation of a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifierVerificationError {
	/// The data format is invalid.
	InvalidFormat,
	/// The identifier is not valid.
	InvalidIdentifier,
	/// The identifier has an invalid length.
	InvalidIdentifierLength,
}

/// Generate Blake2b Hash
pub fn ss58hash(data: &[u8]) -> Blake2bResult {
	let mut context = Blake2b::new(64);
	context.update(PREFIX);
	context.update(data);
	context.finalize()
}

/// Create a new cryptographic identifier.
pub fn generate(data: &[u8], id_ident: u16) -> String {
	let ident: u16 = u16::from(id_ident) & 0b0011_1111_1111_1111;

	let mut v = match ident {
		0..=63 => vec![ident as u8],
		64..=16_383 => {
			// upper six bits of the lower byte(!)
			let first = ((ident & 0b0000_0000_1111_1100) as u8) >> 2;
			// lower two bits of the lower byte in the high pos,
			// lower bits of the upper byte in the low pos
			let second = ((ident >> 8) as u8) | ((ident & 0b0000_0000_0000_0011) as u8) << 6;
			vec![first | 0b01000000, second]
		},
		_ => unreachable!("masked out the upper two bits; qed"),
	};
	v.extend(data.as_ref());
	let r = ss58hash(&v);
	v.extend(&r.as_bytes()[0..2]);
	v.to_base58()
}

pub fn from_known_format(id: &IdentifierOf, id_ident: u16) -> IdentifierVerificationResult {
	let identifier = str::from_utf8(id).map_err(|_| IdentifierVerificationError::InvalidFormat)?;
	let data = identifier
		.from_base58()
		.map_err(|_| IdentifierVerificationError::InvalidIdentifier)?;
	ensure!(
		(identifier.len() > 2 && identifier.len() < 50),
		IdentifierVerificationError::InvalidIdentifierLength
	);
	let (_prefix_len, ident) = match data[0] {
		0..=63 => (1, data[0] as u16),
		64..=127 => {
			let lower = (data[0] << 2) | (data[1] >> 6);
			let upper = data[1] & 0b00111111;
			(2, (lower as u16) | ((upper as u16) << 8))
		},
		_ => unreachable!("identifier not within the range; qed"),
	};
	ensure!(ident == id_ident, IdentifierVerificationError::InvalidIdentifierLength);
	Ok(())
}
