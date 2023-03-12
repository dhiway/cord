// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) 2023 Dhiway.
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

use sp_std::{fmt::Debug, marker::PhantomData, ops::Deref, vec::Vec};

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure, sp_runtime::SaturatedConversion, traits::Get, BoundedVec, RuntimeDebug,
};
use scale_info::TypeInfo;

use crate::{Config, Error};

#[derive(
	Copy, Clone, Eq, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
pub struct Timepoint<BlockNumber> {
	/// The height of the chain at the point in time.
	pub height: BlockNumber,
	/// The index of the extrinsic at the point in time.
	pub index: u32,
}

/// A DID name.
///
/// It is bounded in size (inclusive range [MinLength, MaxLength]) and can only
/// contain a subset of ASCII characters.
#[derive(Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T, MinLength, MaxLength))]
#[codec(mel_bound())]
pub struct AsciiDidName<T: Config>(
	pub(crate) BoundedVec<u8, T::MaxNameLength>,
	PhantomData<(T, T::MinNameLength)>,
);

impl<T: Config> Deref for AsciiDidName<T> {
	type Target = BoundedVec<u8, T::MaxNameLength>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: Config> From<AsciiDidName<T>> for Vec<u8> {
	fn from(name: AsciiDidName<T>) -> Self {
		name.0.into_inner()
	}
}

impl<T: Config> TryFrom<Vec<u8>> for AsciiDidName<T> {
	type Error = Error<T>;

	/// Fallible initialization from a provided byte vector if it is below the
	/// minimum or exceeds the maximum allowed length or contains invalid ASCII
	/// characters.
	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		ensure!(value.len() >= T::MinNameLength::get().saturated_into(), Self::Error::TooShort);
		let bounded_vec: BoundedVec<u8, T::MaxNameLength> =
			BoundedVec::try_from(value).map_err(|_| Self::Error::TooLong)?;
		ensure!(is_valid_did_name(&bounded_vec), Self::Error::InvalidCharacter);
		Ok(Self(bounded_vec, PhantomData))
	}
}

/// Verify that a given slice can be used as a web3 name.
fn is_valid_did_name(input: &[u8]) -> bool {
	input
		.iter()
		.all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'| b'.'| b'@'))
}

// FIXME: did not find a way to automatically implement this.
impl<T: Config> PartialEq for AsciiDidName<T> {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
	}
}

// FIXME: did not find a way to automatically implement this.
impl<T: Config> Clone for AsciiDidName<T> {
	fn clone(&self) -> Self {
		Self(self.0.clone(), self.1)
	}
}

/// DID name ownership details.
#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct DidNameOwnership<Owner, BlockNumber> {
	/// The owner of the web3 name.
	pub owner: Owner,
	/// The block number at which the web3 name was claimed.
	pub claimed_at: Timepoint<BlockNumber>,
}

#[cfg(test)]
mod tests {
	use sp_runtime::SaturatedConversion;

	use crate::{did_name::AsciiDidName, mock::Test, Config};

	const MIN_LENGTH: u32 = <Test as Config>::MinNameLength::get();
	const MAX_LENGTH: u32 = <Test as Config>::MaxNameLength::get();

	#[test]
	fn valid_did_name_inputs() {
		let valid_inputs = vec![
			// Minimum length allowed
			vec![b'a'; MIN_LENGTH.saturated_into()],
			// Maximum length allowed
			vec![b'a'; MAX_LENGTH.saturated_into()],
			// All ASCII characters allowed
			b"qwertyuiopasdfghjklzxcvbnm".to_vec(),
			b"0123456789".to_vec(),
			b"---".to_vec(),
			b"___".to_vec(),
		];

		let invalid_inputs = vec![
			// Empty string
			b"".to_vec(),
			// One less than minimum length allowed
			vec![b'a'; MIN_LENGTH.saturated_into::<usize>() - 1usize],
			// One more than maximum length allowed
			vec![b'a'; MAX_LENGTH.saturated_into::<usize>() + 1usize],
			// Invalid ASCII symbol
			b"almostavalidweb3_name!".to_vec(),
			// Non-ASCII character
			String::from("almostavalid_did_name😂").as_bytes().to_owned(),
		];

		for valid in valid_inputs {
			assert!(AsciiDidName::<Test>::try_from(valid).is_ok());
		}

		for invalid in invalid_inputs {
			assert!(AsciiDidName::<Test>::try_from(invalid).is_err());
		}
	}
}
