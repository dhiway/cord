// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2021 Dhiway
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
	/// Schema identifier.
	pub schema_hash: HashOf<T>,
	/// \[OPTIONAL\] Schema CID.
	pub cid: Option<IdentifierOf>,
	/// \[OPTIONAL\] Schema previous CID.
	pub parent_cid: Option<IdentifierOf>,
	/// Schema creator.
	pub creator: CordAccountOf<T>,
	/// Schema block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating schema type.
	pub permissioned: StatusOf,
	/// The flag indicating the status of the schema.
	pub revoked: StatusOf,
}

impl<T: Config> SchemaDetails<T> {
	pub fn is_valid(incoming: &IdentifierOf) -> DispatchResult {
		let cid_str = str::from_utf8(incoming).unwrap();
		let cid_details: Cid = cid_str.parse().map_err(|_err| Error::<T>::InvalidCidEncoding)?;
		ensure!(
			(cid_details.version() == Version::V1 || cid_details.version() == Version::V0),
			Error::<T>::InvalidCidVersion
		);
		Ok(())
	}

	pub fn schema_status(tx_schema: &IdOf<T>, requestor: CordAccountOf<T>) -> Result<(), Error<T>> {
		let schema_details = <Schemas<T>>::get(tx_schema).ok_or(Error::<T>::SchemaNotFound)?;
		ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);
		if schema_details.creator != requestor && schema_details.permissioned {
			let delegates = <Delegations<T>>::get(tx_schema);
			ensure!(
				(delegates.iter().find(|&delegate| *delegate == requestor) == Some(&requestor)),
				Error::<T>::UnauthorizedOperation
			);
		}
		Ok(())
	}
}

/// An on-chain commit details.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct SchemaCommit<T: Config> {
	/// schema hash.
	pub schema_hash: HashOf<T>,
	/// \[OPTIONAL\] schema storage ID
	pub cid: Option<IdentifierOf>,
	/// schema tx block number
	pub block: BlockNumberOf<T>,
	/// schema tx request type
	pub commit: SchemaCommitOf,
}

impl<T: Config> SchemaCommit<T> {
	pub fn store_tx(identifier: &IdOf<T>, tx_commit: SchemaCommit<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(identifier).unwrap_or_default();
		commit.push(tx_commit);
		<Commits<T>>::insert(identifier, commit);
		Ok(())
	}
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo)]
pub enum SchemaCommitOf {
	Genesis,
	Update,
	Delegates,
	RevokeDelegates,
	Permission,
	StatusChange,
}
