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

use sp_std::{fmt::Debug, marker::PhantomData, ops::Deref, vec::Vec};

use crate::{Config, Error};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{ensure, sp_runtime::SaturatedConversion, traits::Get, BoundedVec};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

const NAME_SEPARATOR: u8 = b'@';
const NETWORK_SUFFIX: &[u8] = b"cord";

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
		ensure!(value.len() >= T::MinNameLength::get().saturated_into(), Self::Error::NameTooShort);
		let bounded_vec: BoundedVec<u8, T::MaxNameLength> =
			BoundedVec::try_from(value).map_err(|_| Self::Error::NameExceedsMaxLength)?;

		let mut split = bounded_vec.splitn(2, |c| *c == NAME_SEPARATOR);

		let (prefix, suffix) = (split.next(), split.next());

		if let (Some(prefix), Some(suffix)) = (prefix, suffix) {
			ensure!(matches!(suffix, NETWORK_SUFFIX), Self::Error::InvalidSuffix);
			ensure!(
				prefix.len() >= T::MinNameLength::get().saturated_into(),
				Self::Error::NameTooShort
			);
			ensure!(
				prefix.len() <= T::MaxPrefixLength::get().saturated_into(),
				Self::Error::NameExceedsMaxLength
			);
			ensure!(is_valid_did_name_prefix(prefix), Self::Error::InvalidFormat);
		} else {
			Err(Self::Error::InvalidFormat)?
		}

		Ok(Self(bounded_vec, PhantomData))
	}
}

/// Verify that a given slice can be used as a name prefix.
fn is_valid_did_name_prefix(input: &[u8]) -> bool {
	// Check prefix is empty or not
	if input.is_empty() {
		return false;
	}

	// Check first character
	// - Must be a lowercase letter
	if !(input[0].is_ascii_lowercase()) {
		return false;
	}

	// Check characters
	// - Only allow lowercase letters, numbers, and periods
	// - Only allow one period in a row
	let mut is_valid = false;
	let mut prev_char = None;
	for c in input.iter().skip(1) {
		if matches!(c, b'a'..=b'z' | b'0'..=b'9') {
			is_valid = true;
		} else if matches!(c, b'.') {
			if let Some(prev) = prev_char {
				if prev == &b'.' {
					return false;
				}
			}
		} else {
			return false; // unexpected character
		}
		prev_char = Some(c);
	}

	// Check last character
	// - Do not allow a period at the end
	if input.last() == Some(&b'.') {
		return false;
	}
	is_valid
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
	/// The owner of the did name.
	pub owner: Owner,
	/// The block number at which the did name was registered.
	pub registered_at: BlockNumber,
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
			// All ASCII characters allowed
			b"qwertyuiopasdfghjklzxcvbnm@cord".to_vec(),
			b"a123456789@cord".to_vec(),
			b"abc.123@cord".to_vec(),
			b"abc.123.xyz@cord".to_vec(),
		];

		let invalid_inputs = vec![
			// Empty string
			b"".to_vec(),
			// One less than minimum length allowed
			vec![b'a'; MIN_LENGTH.saturated_into::<usize>() - 1usize],
			// One more than maximum length allowed
			vec![b'a'; MAX_LENGTH.saturated_into::<usize>() + 1usize],
			// Invalid ASCII symbol
			b"almostavalidweb3_name!@cord".to_vec(),
			// Invalid ASCII symbol
			b"almostavalidweb3_name!".to_vec(),
			// Non-ASCII character
			String::from("almostavalid_did_name😂").as_bytes().to_owned(),
			b"---@cord".to_vec(),
			b"___@cord".to_vec(),
			b"abc..xyz@cord".to_vec(),
			b"abc@newid".to_vec(),
			b"abc@newid.".to_vec(),
			b"	@cord".to_vec(),
		];

		for valid in valid_inputs {
			assert!(AsciiDidName::<Test>::try_from(valid).is_ok());
		}

		for invalid in invalid_inputs {
			assert!(AsciiDidName::<Test>::try_from(invalid).is_err());
		}
	}
}
