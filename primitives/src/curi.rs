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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
use crate::*;
use base58::{FromBase58, ToBase58};
use blake2_rfc::blake2b::{Blake2b, Blake2bResult};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::ensure;
use frame_support::{sp_runtime::RuntimeDebug, traits::ConstU32, BoundedVec};
use scale_info::prelude::string::String;
use scale_info::TypeInfo;
use sp_std::{fmt::Debug, prelude::Clone, str, vec};

/// CORD Identifier Prefix
const PREFIX: &[u8] = b"CRDIDFR";
/// CORD idents
const IDENT_REG: u16 = 7101;
const IDENT_AUTH: u16 = 2604;
const IDENT_SCHEMA: u16 = 8902;
const IDENT_STREAM: u16 = 11992;
const IDENT_OPEN_STREAM: u16 = 5802;

/// The minimum length of a valid identifier.
pub const MINIMUM_IDENTIFIER_LENGTH: usize = 2;
// const MINIMUM_IDENTIFIER_LENGTH_U32: u32 = MINIMUM_IDENTIFIER_LENGTH as u32;
/// The maximum length of a valid identifier.
pub const MAXIMUM_IDENTIFIER_LENGTH: usize = 49;
const MAXIMUM_IDENTIFIER_LENGTH_U32: u32 = MAXIMUM_IDENTIFIER_LENGTH as u32;

#[derive(
	Clone, Eq, PartialEq, Ord, PartialOrd, RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo,
)]
pub struct Ss58Identifier(pub(crate) BoundedVec<u8, ConstU32<MAXIMUM_IDENTIFIER_LENGTH_U32>>);

// The Result of the signature verification.
pub type IdentifierVerificationResult = Result<str, IdentifierError>;

// The Result of the signature verification.
pub type IdentVerificationResult = Result<(), IdentifierError>;

/// An error with the interpretation of a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifierError {
	/// The data format is invalid.
	InvalidFormat,
	/// The data format is invalid.
	InvalidPrefix,
	/// The identifier is not valid.
	InvalidIdentifier,
	/// The identifier has an invalid length.
	InvalidIdentifierLength,
}

/// An error with the interpretation of a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifierType {
	Registry,
	Authorization,
	Schema,
	Stream,
}

impl TryFrom<Vec<u8>> for Ss58Identifier {
	type Error = &'static str;

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		let identifier = Ss58Identifier::to_registry_id(&value[..])
			.map_err(|_| "Cannot convert provided input to a valid identifier.")?;

		Ok(identifier)
	}
}

#[cfg(feature = "std")]
impl TryFrom<String> for Ss58Identifier {
	type Error = &'static str;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		Self::try_from(value.into_bytes())
	}
}

impl Ss58Identifier {
	/// Generate Blake2b Hash
	pub fn ss58hash(data: &[u8]) -> Blake2bResult {
		let mut context = Blake2b::new(64);
		context.update(PREFIX);
		context.update(data);
		context.finalize()
	}

	/// Create a new cryptographic identifier.
	pub fn from_encoded<I>(data: I, id_ident: u16) -> Result<Self, IdentifierError>
	where
		I: AsRef<[u8]> + Into<Vec<u8>>,
	{
		let input = data.as_ref();

		ensure!(
			(input.len() > MINIMUM_IDENTIFIER_LENGTH && input.len() < MAXIMUM_IDENTIFIER_LENGTH),
			IdentifierError::InvalidIdentifierLength
		);

		let ident: u16 = id_ident & 0b0011_1111_1111_1111;

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
		let r = Self::ss58hash(&v);
		v.extend(&r.as_bytes()[0..2]);

		Ok(Self(
			Vec::<u8>::from(v.to_base58())
				.try_into()
				.map_err(|_| IdentifierError::InvalidIdentifierLength)?,
		))
	}

	/// Create a new cryptographic identifier.
	pub fn from_string_encoded<I>(data: I, id_ident: u16) -> String
	where
		I: AsRef<[u8]> + Into<Vec<u8>>,
	{
		// let input = data.as_ref();

		let ident: u16 = id_ident & 0b0011_1111_1111_1111;

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
		let r = Self::ss58hash(&v);
		v.extend(&r.as_bytes()[0..2]);
		v.to_base58()
	}
	pub fn to_authorization_id(data: &[u8]) -> Result<Self, IdentifierError> {
		Self::from_encoded(data, IDENT_AUTH)
	}

	pub fn to_registry_id(data: &[u8]) -> Result<Self, IdentifierError> {
		Self::from_encoded(data, IDENT_REG)
	}

	pub fn to_schema_id(data: &[u8]) -> Result<Self, IdentifierError> {
		Self::from_encoded(data, IDENT_SCHEMA)
	}

	pub fn to_stream_id(data: &[u8]) -> Result<Self, IdentifierError> {
		Self::from_encoded(data, IDENT_STREAM)
	}

	pub fn to_open_stream_id(data: &[u8]) -> Result<Self, IdentifierError> {
		Self::from_encoded(data, IDENT_OPEN_STREAM)
	}
	pub fn inner(&self) -> &[u8] {
		&self.0
	}

	pub fn get_ident(id: Self, id_ident: u16) -> IdentVerificationResult {
		let identifier = str::from_utf8(id.inner()).map_err(|_| IdentifierError::InvalidFormat)?;
		let data = identifier.from_base58().map_err(|_| IdentifierError::InvalidIdentifier)?;
		if data.len() < 2 {
			return Err(IdentifierError::InvalidIdentifierLength);
		}
		ensure!(
			(identifier.len() > 2 && identifier.len() < 50),
			IdentifierError::InvalidIdentifierLength
		);
		let (_prefix_len, ident) = match data[0] {
			0..=63 => (1, data[0] as u16),
			64..=127 => {
				let lower = (data[0] << 2) | (data[1] >> 6);
				let upper = data[1] & 0b00111111;
				(2, (lower as u16) | ((upper as u16) << 8))
			},
			_ => return Err(IdentifierError::InvalidPrefix),
		};

		ensure!(ident == id_ident, IdentifierError::InvalidPrefix);
		Ok(())
	}

	pub fn is_stream_id(data: Ss58Identifier) -> Result<(), IdentifierError> {
		Self::get_ident(data, IDENT_STREAM)
	}
	pub fn is_open_stream_id(data: Ss58Identifier) -> Result<(), IdentifierError> {
		Self::get_ident(data, IDENT_OPEN_STREAM)
	}
	pub fn is_schema_id(data: Ss58Identifier) -> Result<(), IdentifierError> {
		Self::get_ident(data, IDENT_SCHEMA)
	}
	pub fn is_registry_id(data: Ss58Identifier) -> Result<(), IdentifierError> {
		Self::get_ident(data, IDENT_REG)
	}
	pub fn is_authorization_id(data: Ss58Identifier) -> Result<(), IdentifierError> {
		Self::get_ident(data, IDENT_AUTH)
	}

	// pub fn from_known_identifier(id: &Ss58Identifier) -> IdentifierVerificationResult {
	// 	let identifier = str::from_utf8(id.inner()).map_err(|_| IdentifierError::InvalidFormat)?;
	// 	let data = identifier.from_base58().map_err(|_| IdentifierError::InvalidIdentifier)?;
	// 	if data.len() < 2 {
	// 		return Err(IdentifierError::InvalidIdentifierLength);
	// 	}
	// 	ensure!(
	// 		(identifier.len() > 2 && identifier.len() < 50),
	// 		IdentifierError::InvalidIdentifierLength
	// 	);
	// 	let (_prefix_len, ident) = match data[0] {
	// 		0..=63 => (1, data[0] as u16),
	// 		64..=127 => {
	// 			let lower = (data[0] << 2) | (data[1] >> 6);
	// 			let upper = data[1] & 0b00111111;
	// 			(2, (lower as u16) | ((upper as u16) << 8))
	// 		},
	// 		_ => return Err(IdentifierError::InvalidPrefix),
	// 	};

	// 	let identifier_type = match ident {
	// 		IDENT_AUTH => "authorization",
	// 		IDENT_REG => "registry",
	// 		IDENT_SCHEMA => "schema",
	// 		IDENT_STREAM => "stream",
	// 		_ => return Err(IdentifierError::InvalidPrefix),
	// 	};

	// 	Ok(identifier_type)
	// }
}

impl AsRef<[u8]> for Ss58Identifier {
	fn as_ref(&self) -> &[u8] {
		&self.0[..]
	}
}
