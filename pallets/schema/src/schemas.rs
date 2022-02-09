// CORD Blockchain – https://dhiway.network
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

/// An on-chain schema details mapped to an identifier.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct SchemaDetails<T: Config> {
	/// Schema Version.
	pub version: VersionOf,
	/// Schema identifier.
	pub schema_id: IdOf<T>,
	/// Schema creator.
	pub creator: CordAccountOf<T>,
	/// \[OPTIONAL\] IPFS CID.
	pub cid: Option<CidOf>,
	/// \[OPTIONAL\] Schema Parent hash.
	pub parent: Option<HashOf<T>>,
	/// The flag indicating schema type.
	pub permissioned: StatusOf,
	/// The flag indicating the status of the schema.
	pub revoked: StatusOf,
}

impl<T: Config> SchemaDetails<T> {
	pub fn is_valid(incoming: &CidOf) -> DispatchResult {
		let cid_str = str::from_utf8(incoming).unwrap();
		let cid_details: Cid = cid_str.parse().map_err(|_err| Error::<T>::InvalidCidEncoding)?;
		ensure!(
			(cid_details.version() == CidType::V1 || cid_details.version() == CidType::V0),
			Error::<T>::InvalidCidVersion
		);
		Ok(())
	}

	pub fn schema_status(tx_schema: &IdOf<T>, requestor: CordAccountOf<T>) -> Result<(), Error<T>> {
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
