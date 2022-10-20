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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo)]
pub struct SchemaInput<HashOf, CordAccountOf, Signature, InputSchemaMetatOf> {
	/// Schema hash.
	pub digest: HashOf,
	/// Schema controller.
	pub controller: CordAccountOf,
	/// Controller Signature
	pub signature: Signature,
	/// An optional opaque blob representing the metadata for the schema. Could
	/// be JSON, a link, a Hash, or raw text. Up to the community to decide how
	/// exactly to use this.
	pub meta: Option<InputSchemaMetatOf>,
}

/// An on-chain schema details mapped to an identifier.
#[derive(
	Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]

pub struct SchemaEntry<HashOf, CordAccountOf, InputSchemaMetatOf, BlockNumber> {
	/// Schema hash.
	pub digest: HashOf,
	/// Schema controller.
	pub controller: CordAccountOf,
	/// An optional opaque blob representing the metadata for the schema. Could
	/// be JSON, a link, a Hash, or raw text. Up to the community to decide how
	/// exactly to use this.
	pub meta: Option<InputSchemaMetatOf>,
	/// The block number in which schema is included
	pub block_number: BlockNumber,
}

// impl<T: Config> SchemaDetails<T> {
// 	pub fn from_schema_identities(
// 		tx_ident: &IdentifierOf,
// 		requestor: CordAccountOf<T>,
// 	) -> Result<(), Error<T>> {
// 		ss58identifier::from_known_format(tx_ident, SCHEMA_PREFIX)
// 			.map_err(|_| Error::<T>::InvalidSchemaIdentifier)?;

// 		let schema_details =
// <Schemas<T>>::get(&tx_ident).ok_or(Error::<T>::SchemaNotFound)?; 		ensure!(!
// schema_details.revoked, Error::<T>::SchemaRevoked);
// 		Self::from_schema_delegates(tx_ident, schema_details.schema.controller,
// requestor) 			.map_err(Error::<T>::from)?;

// 		Ok(())
// 	}
// 	pub fn set_schema_metadata(
// 		tx_ident: &IdentifierOf,
// 		requestor: CordAccountOf<T>,
// 		status: bool,
// 	) -> Result<(), Error<T>> {
// 		let schema_details =
// <Schemas<T>>::get(&tx_ident).ok_or(Error::<T>::SchemaNotFound)?; 		ensure!(!
// schema_details.revoked, Error::<T>::SchemaRevoked);

// 		Self::from_schema_delegates(tx_ident,
// schema_details.schema.controller.clone(), requestor)
// 			.map_err(Error::<T>::from)?;

// 		<Schemas<T>>::insert(&tx_ident, SchemaDetails { meta: status,
// ..schema_details });

// 		Ok(())
// 	}

// 	pub fn from_schema_delegates(
// 		tx_ident: &IdentifierOf,
// 		requestor: CordAccountOf<T>,
// 		controller: CordAccountOf<T>,
// 	) -> Result<(), Error<T>> {
// 		if controller != requestor {
// 			let delegates = <SchemaDelegations<T>>::get(tx_ident);
// 			ensure!(
// 				(delegates.iter().find(|&delegate| *delegate == requestor) ==
// Some(&requestor)), 				Error::<T>::UnauthorizedOperation
// 			);
// 		}
// 		Ok(())
// 	}
// }

// /// An on-chain schema details mapped to an identifier.
// #[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
// #[scale_info(skip_type_params(T))]
// pub struct SchemaParams<T: Config> {
// 	/// Schema identifier
// 	pub identifier: IdentifierOf,
// 	/// Schema Type.
// 	pub schema: SchemaType<T>,
// }

// impl<T: Config> sp_std::fmt::Debug for SchemaParams<T> {
// 	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
// 		Ok(())
// 	}
// }
