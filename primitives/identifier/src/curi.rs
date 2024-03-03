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

#![allow(clippy::unused_unit)]

use crate::*;
use blake2_rfc::blake2b::{Blake2b, Blake2bResult};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{ensure, sp_runtime::RuntimeDebug, traits::ConstU32, BoundedVec};
use scale_info::TypeInfo;
use sp_std::{
	fmt::Debug,
	prelude::{Clone, Vec},
	str, vec,
};

/// CORD Identifier Prefix
const PREFIX: &[u8] = b"CRDIDFR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifierType {
	Authorization,
	Space,
	Schema,
	Statement,
	Entity,
	Template,
	Asset,
	AssetInstance,
	Rating,
}

impl IdentifierType {
	const IDENT_AUTH: u16 = 2092;
	const IDENT_SPACE: u16 = 3390;
	const IDENT_SCHEMA: u16 = 7366;
	const IDENT_STATEMENT: u16 = 8902;
	const IDENT_ENTITY: u16 = 6480;
	const IDENT_TEMPLATE: u16 = 8911;
	const IDENT_ASSET: u16 = 2348;
	const IDENT_RATING: u16 = 6077;
	const IDENT_ASSET_INSTANCE: u16 = 11380;

	fn ident_value(&self) -> u16 {
		match self {
			IdentifierType::Authorization => Self::IDENT_AUTH,
			IdentifierType::Space => Self::IDENT_SPACE,
			IdentifierType::Schema => Self::IDENT_SCHEMA,
			IdentifierType::Statement => Self::IDENT_STATEMENT,
			IdentifierType::Entity => Self::IDENT_ENTITY,
			IdentifierType::Template => Self::IDENT_TEMPLATE,
			IdentifierType::Asset => Self::IDENT_ASSET,
			IdentifierType::AssetInstance => Self::IDENT_ASSET_INSTANCE,
			IdentifierType::Rating => Self::IDENT_RATING,
		}
	}
	fn from_u16(value: u16) -> Option<Self> {
		match value {
			2092 => Some(IdentifierType::Authorization),
			3390 => Some(IdentifierType::Space),
			7366 => Some(IdentifierType::Schema),
			8902 => Some(IdentifierType::Statement),
			6480 => Some(IdentifierType::Entity),
			8911 => Some(IdentifierType::Template),
			2348 => Some(IdentifierType::Asset),
			6077 => Some(IdentifierType::AssetInstance),
			11380 => Some(IdentifierType::Rating),
			_ => None,
		}
	}
}

/// The minimum length of a valid identifier.
pub const MINIMUM_IDENTIFIER_LENGTH: usize = 2;
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
	/// Identifier timeline update failed
	UpdateFailed,
	MaxEventsHistoryExceeded,
}

pub trait IdentifierCreator {
	fn create_identifier(
		data: &[u8],
		id_type: IdentifierType,
	) -> Result<Ss58Identifier, IdentifierError>;
}

impl IdentifierCreator for Ss58Identifier {
	fn create_identifier(
		data: &[u8],
		id_type: IdentifierType,
	) -> Result<Ss58Identifier, IdentifierError> {
		let id_ident = id_type.ident_value();
		Ss58Identifier::from_encoded(data, id_ident)
	}
}

pub trait CordIdentifierType {
	fn get_type(&self) -> Result<IdentifierType, IdentifierError>;
}

impl CordIdentifierType for Ss58Identifier {
	fn get_type(&self) -> Result<IdentifierType, IdentifierError> {
		let identifier_type_u16 = self.get_identifier_type()?;

		IdentifierType::from_u16(identifier_type_u16).ok_or(IdentifierError::InvalidIdentifier)
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
			Vec::<u8>::from(bs58::encode(v).into_string())
				.try_into()
				.map_err(|_| IdentifierError::InvalidIdentifier)?,
		))
	}

	pub fn inner(&self) -> &[u8] {
		&self.0
	}

	pub fn get_identifier_type(&self) -> Result<u16, IdentifierError> {
		let identifier =
			str::from_utf8(self.inner()).map_err(|_| IdentifierError::InvalidFormat)?;

		let data = bs58::decode(identifier)
			.into_vec()
			.map_err(|_| IdentifierError::InvalidIdentifier)?;

		if data.len() < 2 {
			return Err(IdentifierError::InvalidIdentifierLength);
		}

		// Ensure the identifier length is within the expected range
		ensure!(
			identifier.len() > 2 && identifier.len() < 50,
			IdentifierError::InvalidIdentifierLength
		);

		// Extract the identifier type
		match data[0] {
			0..=63 => Ok(data[0] as u16),
			64..=127 => {
				let lower = (data[0] << 2) | (data[1] >> 6);
				let upper = data[1] & 0b00111111;
				Ok((lower as u16) | ((upper as u16) << 8))
			},
			_ => Err(IdentifierError::InvalidPrefix),
		}
	}

	pub fn default_error() -> Self {
		let error_value_base58 = bs58::encode([0]).into_string();

		// Convert the Base58 encoded string to a byte vector.
		let error_value_bytes = error_value_base58.into_bytes();

		// Attempt to convert the byte vector into a BoundedVec.
		let bounded_error_value = BoundedVec::try_from(error_value_bytes)
			.expect("Should not fail as the length is within bounds");

		Ss58Identifier(bounded_error_value)
	}
}

pub struct IdentifierTimeline;

impl IdentifierTimeline {
	pub fn update_timeline<T: Config>(
		id: &IdentifierOf,
		id_type: IdentifierTypeOf,
		entry: EventEntryOf,
	) -> Result<(), IdentifierError>
	where
		Pallet<T>: IdentifierUpdate<IdentifierOf, IdentifierTypeOf, EventEntryOf, IdentifierError>,
	{
		<Pallet<T> as IdentifierUpdate<
			IdentifierOf,
			IdentifierTypeOf,
			EventEntryOf,
			IdentifierError,
		>>::update_timeline(id, id_type, entry)
		.map_err(|_| IdentifierError::MaxEventsHistoryExceeded)
	}
}

impl AsRef<[u8]> for Ss58Identifier {
	fn as_ref(&self) -> &[u8] {
		&self.0[..]
	}
}
