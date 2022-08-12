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

/// An on-chain space details mapped to an identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct RegistryType<T: Config> {
	/// Collection hash.
	pub digest: HashOf<T>,
	/// Collection creator.
	pub controller: CordAccountOf<T>,
}

impl<T: Config> sp_std::fmt::Debug for RegistryType<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

/// An on-chain space details mapped to an identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct RegistryDetails<T: Config> {
	/// Collection type.
	pub registry: RegistryType<T>,
	/// The flag indicating the status of the collection.
	pub archived: StatusOf,
	/// The flag indicating the status of the metadata.
	pub metadata: StatusOf,
}

impl<T: Config> sp_std::fmt::Debug for RegistryDetails<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config> RegistryDetails<T> {
	pub fn from_collection_identities(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		ss58identifier::from_known_format(tx_ident, REGISTRY_INDEX)
			.map_err(|_| Error::<T>::InvalidCollectionIdentifier)?;

		let registry_details =
			<Registries<T>>::get(&tx_ident).ok_or(Error::<T>::CollectionNotFound)?;
		ensure!(!registry_details.archived, Error::<T>::ArchivedCollection);

		if registry_details.registry.controller != requestor {
			let delegates = <Delegations<T>>::get(tx_ident);
			ensure!(
				(delegates.iter().find(|&delegate| *delegate == requestor) == Some(&requestor)),
				Error::<T>::UnauthorizedOperation
			);
		}
		Ok(())
	}
	pub fn from_collection_delegates(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
		controller: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		if controller != requestor {
			let delegates = <Delegations<T>>::get(tx_ident);
			ensure!(
				(delegates.iter().find(|&delegate| *delegate == requestor) == Some(&requestor)),
				Error::<T>::UnauthorizedOperation
			);
		}
		Ok(())
	}
	pub fn set_collection_metadata(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
		status: bool,
	) -> Result<(), Error<T>> {
		let registry_details =
			<Registries<T>>::get(&tx_ident).ok_or(Error::<T>::CollectionNotFound)?;
		ensure!(!registry_details.archived, Error::<T>::ArchivedCollection);

		if registry_details.registry.controller != requestor {
			let delegates = <Delegations<T>>::get(tx_ident);
			ensure!(
				(delegates.iter().find(|&delegate| *delegate == requestor) == Some(&requestor)),
				Error::<T>::UnauthorizedOperation
			);
		}
		<Registries<T>>::insert(
			&tx_ident,
			RegistryDetails { metadata: status, ..registry_details },
		);

		Ok(())
	}
}

/// An on-chain schema details mapped to an identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct RegistryParams<T: Config> {
	/// Collection identifier
	pub identifier: IdentifierOf,
	/// Collection Type.
	pub registry: RegistryType<T>,
}

impl<T: Config> sp_std::fmt::Debug for RegistryParams<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}
