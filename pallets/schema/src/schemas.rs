// CORD Chain Node – https://cord.network
// Copyright (C) 2019-2022 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;
use sp_std::str;

/// An on-chain schema details mapped to an identifier.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct SchemaDetails<T: Config> {
	/// Schema Version.
	pub version: VersionOf,
	/// Schema identifier.
	pub schema_id: IdentifierOf,
	/// Schema creator.
	pub creator: CordAccountOf<T>,
	/// \[OPTIONAL\] Schema Parent hash.
	pub parent: Option<HashOf<T>>,
	/// \[OPTIONAL\] IPFS CID.
	pub cid: Option<CidOf>,
	/// The flag indicating schema type.
	pub permissioned: StatusOf,
	/// The flag indicating the status of the schema.
	pub revoked: StatusOf,
}

impl<T: Config> SchemaDetails<T> {
	pub fn is_valid_cid(incoming: &CidOf) -> DispatchResult {
		let cid_str = str::from_utf8(incoming).map_err(|_err| Error::<T>::InvalidCidEncoding)?;
		let cid_details: Cid = cid_str.parse().map_err(|_err| Error::<T>::InvalidCidEncoding)?;
		ensure!(
			(cid_details.version() == CidType::V1 || cid_details.version() == CidType::V0),
			Error::<T>::InvalidCidVersion
		);
		Ok(())
	}
	pub fn is_valid_identifier(id: &IdentifierOf, id_ident: u16) -> DispatchResult {
		let identifier = str::from_utf8(id).map_err(|_| Error::<T>::InvalidIdentifier)?;
		let data = identifier.from_base58().map_err(|_| Error::<T>::InvalidIdentifier)?;
		ensure!(
			(identifier.len() > 2 && identifier.len() < 50),
			Error::<T>::InvalidIdentifierLength
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
		ensure!(ident == id_ident, Error::<T>::InvalidIdentifier);
		Ok(())
	}
	pub fn schema_status(
		tx_schema: &IdentifierOf,
		requestor: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		let schema_hash = <SchemaId<T>>::get(&tx_schema).ok_or(Error::<T>::SchemaNotFound)?;
		let schema_details = <Schemas<T>>::get(schema_hash).ok_or(Error::<T>::SchemaNotFound)?;
		ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);

		if schema_details.creator != requestor && schema_details.permissioned {
			let delegates = <Delegations<T>>::get(schema_details.schema_id);
			ensure!(
				(delegates.iter().find(|&delegate| *delegate == requestor) == Some(&requestor)),
				Error::<T>::UnauthorizedOperation
			);
		}
		Ok(())
	}
}

/// Base58-to-text encoding
///
/// Based on https://github.com/trezor/trezor-crypto/blob/master/base58.c
///          https://github.com/debris/base58/blob/master/src/lib.rs

const B58_DIGITS_MAP: &'static [i8] = &[
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
	-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
	-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, -1, -1, -1, -1, -1, -1, -1, 9, 10, 11, 12, 13, 14, 15, 16, -1,
	17, 18, 19, 20, 21, -1, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, -1, -1, -1, -1, -1, -1, 33,
	34, 35, 36, 37, 38, 39, 40, 41, 42, 43, -1, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56,
	57, -1, -1, -1, -1, -1,
];
// const PREFIX: &[u8] = b"IDFRPRE";

/// Errors that can occur when decoding base58 encoded string.
#[derive(Debug, PartialEq)]
pub enum FromBase58Error {
	/// The input contained a character which is not a part of the base58 format.
	InvalidBase58Character(char, usize),
	/// The input had invalid length.
	InvalidBase58Length,
}

/// A trait for converting base58 encoded values.
pub trait FromBase58 {
	/// Convert a value of `self`, interpreted as base58 encoded data, into an owned vector of bytes, returning a vector.
	fn from_base58(&self) -> Result<Vec<u8>, FromBase58Error>;
}

impl FromBase58 for str {
	fn from_base58(&self) -> Result<Vec<u8>, FromBase58Error> {
		let mut bin = [0u8; 132];
		let mut out = [0u32; (132 + 3) / 4];
		let bytesleft = (bin.len() % 4) as u8;
		let zeromask = match bytesleft {
			0 => 0u32,
			_ => 0xffffffff << (bytesleft * 8),
		};

		let zcount = self.chars().take_while(|x| *x == '1').count();
		let mut i = zcount;
		let b58: Vec<u8> = self.bytes().collect();

		while i < self.len() {
			if (b58[i] & 0x80) != 0 {
				// High-bit set on invalid digit
				return Err(FromBase58Error::InvalidBase58Character(b58[i] as char, i));
			}

			if B58_DIGITS_MAP[b58[i] as usize] == -1 {
				// // Invalid base58 digit
				return Err(FromBase58Error::InvalidBase58Character(b58[i] as char, i));
			}

			let mut c = B58_DIGITS_MAP[b58[i] as usize] as u64;
			let mut j = out.len();
			while j != 0 {
				j -= 1;
				let t = out[j] as u64 * 58 + c;
				c = (t & 0x3f00000000) >> 32;
				out[j] = (t & 0xffffffff) as u32;
			}

			if c != 0 {
				// Output number too big (carry to the next int32)
				return Err(FromBase58Error::InvalidBase58Length);
			}

			if (out[0] & zeromask) != 0 {
				// Output number too big (last int32 filled too far)
				return Err(FromBase58Error::InvalidBase58Length);
			}

			i += 1;
		}

		let mut i = 1;
		let mut j = 0;

		bin[0] = match bytesleft {
			3 => ((out[0] & 0xff0000) >> 16) as u8,
			2 => ((out[0] & 0xff00) >> 8) as u8,
			1 => {
				j = 1;
				(out[0] & 0xff) as u8
			},
			_ => {
				i = 0;
				bin[0]
			},
		};

		while j < out.len() {
			bin[i] = ((out[j] >> 0x18) & 0xff) as u8;
			bin[i + 1] = ((out[j] >> 0x10) & 0xff) as u8;
			bin[i + 2] = ((out[j] >> 8) & 0xff) as u8;
			bin[i + 3] = ((out[j] >> 0) & 0xff) as u8;
			i += 4;
			j += 1;
		}

		let leading_zeros = bin.iter().take_while(|x| **x == 0).count();
		Ok(bin[leading_zeros - zcount..].to_vec())
	}
}
