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
use base58::{FromBase58, ToBase58};
use blake2_rfc::blake2b::{Blake2b, Blake2bResult};
use codec::{Decode, Encode};
use scale_info::prelude::string::String;
use sp_runtime::DispatchResult;
use sp_std::vec;

/// An on-chain schema details mapped to an identifier.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct SchemaDetails<T: Config> {
	/// Schema Version.
	pub version: VersionOf,
	/// Schema identifier.
	pub schema_id: IdentifierOf,
	/// Schema creator.
	pub controller: CordAccountOf<T>,
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

		if schema_details.controller != requestor && schema_details.permissioned {
			let delegates = <Delegations<T>>::get(schema_details.schema_id);
			ensure!(
				(delegates.iter().find(|&delegate| *delegate == requestor) == Some(&requestor)),
				Error::<T>::UnauthorizedOperation
			);
		}
		Ok(())
	}
}

/// CORD Identifier Prefix
const PREFIX: &[u8] = b"IDFRPRE";

/// Generate Blake2b Hash
pub fn ss58hash(data: &[u8]) -> Blake2bResult {
	let mut context = Blake2b::new(64);
	context.update(PREFIX);
	context.update(data);
	context.finalize()
}

/// Create a new cryptographic identifier.
pub fn create_identifier(data: &[u8], id_ident: u16) -> String {
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
