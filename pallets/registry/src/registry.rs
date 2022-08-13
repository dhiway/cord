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

/// An on-chain registry details mapped to an identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct RegistryType<T: Config> {
	/// Registry hash.
	pub digest: HashOf<T>,
	/// Registry creator.
	pub controller: CordAccountOf<T>,
}

impl<T: Config> sp_std::fmt::Debug for RegistryType<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

/// An on-chain registry details mapped to an identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct RegistryDetails<T: Config> {
	/// Registry type.
	pub register: RegistryType<T>,
	/// \[OPTIONAL\] Schema Identifier
	pub schema: Option<IdentifierOf>,
	/// The flag indicating the status of the registry.
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
	pub fn from_registry_identities(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		ss58identifier::from_known_format(tx_ident, REGISTRY_INDEX)
			.map_err(|_| Error::<T>::InvalidRegistryIdentifier)?;

		let registry_details =
			<Registries<T>>::get(&tx_ident).ok_or(Error::<T>::RegistryNotFound)?;
		ensure!(!registry_details.archived, Error::<T>::ArchivedRegistry);

		Self::from_registry_delegates(tx_ident, registry_details.register.controller, requestor)
			.map_err(Error::<T>::from)?;

		Ok(())
	}

	pub fn set_registry_metadata(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
		status: bool,
	) -> Result<(), Error<T>> {
		let registry_details =
			<Registries<T>>::get(&tx_ident).ok_or(Error::<T>::RegistryNotFound)?;
		ensure!(!registry_details.archived, Error::<T>::ArchivedRegistry);

		Self::from_registry_delegates(
			tx_ident,
			registry_details.register.controller.clone(),
			requestor,
		)
		.map_err(Error::<T>::from)?;

		<Registries<T>>::insert(
			&tx_ident,
			RegistryDetails { metadata: status, ..registry_details },
		);

		Ok(())
	}

	pub fn set_registry_schema(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
		tx_schema: IdentifierOf,
	) -> Result<(), Error<T>> {
		let registry_details =
			<Registries<T>>::get(&tx_ident).ok_or(Error::<T>::RegistryNotFound)?;
		ensure!(!registry_details.archived, Error::<T>::ArchivedRegistry);

		Self::from_registry_delegates(
			tx_ident,
			registry_details.register.controller.clone(),
			requestor,
		)
		.map_err(Error::<T>::from)?;

		<Registries<T>>::insert(
			&tx_ident,
			RegistryDetails { schema: Some(tx_schema), ..registry_details },
		);

		Ok(())
	}

	pub fn from_registry_delegates(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
		controller: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		if controller != requestor {
			let delegates = <RegistryDelegations<T>>::get(tx_ident);
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
#[codec(mel_bound())]
pub struct RegistryParams<T: Config> {
	/// Registry identifier
	pub identifier: IdentifierOf,
	/// Registry Type.
	pub register: RegistryType<T>,
}

impl<T: Config> sp_std::fmt::Debug for RegistryParams<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}
