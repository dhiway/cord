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
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

/// schema type details.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct SchemaType<T: Config> {
	/// Schema hash.
	pub digest: HashOf<T>,
	/// Schema delegator.
	pub controller: CordAccountOf<T>,
	/// \[OPTIONAL\] Registry Identifier.
	pub register: Option<IdentifierOf>,
}

impl<T: Config> sp_std::fmt::Debug for SchemaType<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

/// An on-chain schema details mapped to an identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct SchemaDetails<T: Config> {
	/// Schema Type.
	pub schema: SchemaType<T>,
	/// The flag indicating the status of the schema.
	pub revoked: StatusOf,
	/// The flag indicating the status of the metadata.
	pub metadata: StatusOf,
}

impl<T: Config> sp_std::fmt::Debug for SchemaDetails<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config> SchemaDetails<T> {
	pub fn from_schema_identities(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		ss58identifier::from_known_format(tx_ident, SCHEMA_PREFIX)
			.map_err(|_| Error::<T>::InvalidSchemaIdentifier)?;

		let schema_details = <Schemas<T>>::get(&tx_ident).ok_or(Error::<T>::SchemaNotFound)?;
		ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);
		Self::from_schema_delegates(tx_ident, schema_details.schema.controller, requestor)
			.map_err(Error::<T>::from)?;

		Ok(())
	}
	pub fn set_schema_metadata(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
		status: bool,
	) -> Result<(), Error<T>> {
		let schema_details = <Schemas<T>>::get(&tx_ident).ok_or(Error::<T>::SchemaNotFound)?;
		ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);

		Self::from_schema_delegates(tx_ident, schema_details.schema.controller.clone(), requestor)
			.map_err(Error::<T>::from)?;

		<Schemas<T>>::insert(&tx_ident, SchemaDetails { metadata: status, ..schema_details });

		Ok(())
	}

	pub fn from_schema_delegates(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
		controller: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		if controller != requestor {
			let delegates = <SchemaDelegations<T>>::get(tx_ident);
			ensure!(
				(delegates.iter().find(|&delegate| *delegate == requestor) == Some(&requestor)),
				Error::<T>::UnauthorizedOperation
			);
		}
		Ok(())
	}
}

/// An on-chain schema details mapped to an identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct SchemaParams<T: Config> {
	/// Schema identifier
	pub identifier: IdentifierOf,
	/// Schema Type.
	pub schema: SchemaType<T>,
}

impl<T: Config> sp_std::fmt::Debug for SchemaParams<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}
