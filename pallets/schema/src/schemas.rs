// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
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

use crate::*;
use codec::{Decode, Encode};

/// An on-chain schema details mapped to an identifier.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct SchemaDetails<T: Config> {
	/// Schema hash.
	pub schema_hash: HashOf<T>,
	/// Schema creator.
	pub controller: CordAccountOf<T>,
	/// \[OPTIONAL\] Space ID.
	pub space: Option<IdentifierOf>,
	/// The flag indicating the status of the schema.
	pub revoked: StatusOf,
}

impl<T: Config> SchemaDetails<T> {
	pub fn from_schema_identities(
		tx_schema: &IdentifierOf,
		requestor: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		mark::from_known_format(tx_schema, SCHEMA_IDENTIFIER_PREFIX)
			.map_err(|_| Error::<T>::InvalidSchemaIdentifier)?;

		let schema_details = <Schemas<T>>::get(&tx_schema).ok_or(Error::<T>::SchemaNotFound)?;
		ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);

		if schema_details.controller != requestor {
			let delegates = <Delegations<T>>::get(tx_schema);
			ensure!(
				(delegates.iter().find(|&delegate| *delegate == requestor) == Some(&requestor)),
				Error::<T>::UnauthorizedOperation
			);
		}
		Ok(())
	}
}
