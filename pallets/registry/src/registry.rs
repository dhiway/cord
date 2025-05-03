// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
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

use super::*;
use alloc::vec::Vec;
use codec::Encode;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use sp_runtime::traits::Hash;

use crate::{
	pallet::Pallet, Delegates, Error, HashOf, Identifier, Permissions, Registries, RegistryDetails,
	RegistryIdentifierOf, Status,
};

/// Create a new registry.
pub fn create_registry<T: crate::Config>(
	tx_hash: HashOf<T>,
	profile_id: ProfileIdOf,
) -> Result<RegistryIdentifierOf, sp_runtime::DispatchError> {
	let mut data = Vec::with_capacity(256);
	data.extend_from_slice(tx_hash.as_ref());
	data.extend_from_slice(&profile_id.encode());

	let digest = T::Hashing::hash(&data);
	let pallet_name = <crate::pallet::Pallet<T> as frame_support::traits::PalletInfoAccess>::name();

	let registry_id =
		<pallet_identifier::Pallet<T> as Identifier<T>>::build(&(digest).encode()[..], pallet_name)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

	ensure!(!Registries::<T>::contains_key(&registry_id), Error::<T>::RegistryAlreadyExists);

	let details = RegistryDetails {
		creator: profile_id.clone(),
		tx_hash,
		doc_id: None,
		doc_author_profile_id: None,
		doc_node_id: None,
		status: Status::Active,
	};

	Pallet::<T>::record_activity(&registry_id, digest, b"RegistryCreated")?;
	Registries::<T>::insert(&registry_id, details);
	Delegates::<T>::insert(&registry_id, &profile_id, Permissions::all());

	Ok(registry_id)
}

/// Create a new registry store.
pub fn create_registry_store<T: crate::Config>(
	tx_hash: HashOf<T>,
	doc_id: Vec<u8>,
	doc_author_profile_id: ProfileIdOf,
	doc_node_id: Vec<u8>,
	profile_id: ProfileIdOf,
) -> Result<RegistryIdentifierOf, sp_runtime::DispatchError> {
	let bounded_doc_id: DocIdOf =
		doc_id.try_into().map_err(|_| Error::<T>::InvalidIdentifierLength)?;
	let bounded_doc_node_id: DocNodeIdOf =
		doc_node_id.try_into().map_err(|_| Error::<T>::InvalidIdentifierLength)?;

	let mut data = Vec::with_capacity(256);
	data.extend_from_slice(tx_hash.as_ref());

	data.extend_from_slice(&bounded_doc_id.encode());
	data.extend_from_slice(&bounded_doc_node_id.encode());

	data.extend_from_slice(&doc_author_profile_id.encode());
	data.extend_from_slice(&profile_id.encode());

	let digest = T::Hashing::hash(&data);
	let pallet_name = <crate::pallet::Pallet<T> as frame_support::traits::PalletInfoAccess>::name();

	let registry_id =
		<pallet_identifier::Pallet<T> as Identifier<T>>::build(&(digest).encode()[..], pallet_name)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

	ensure!(!Registries::<T>::contains_key(&registry_id), Error::<T>::RegistryAlreadyExists);

	let details = RegistryDetails {
		creator: profile_id.clone(),
		tx_hash: tx_hash.clone(),
		doc_id: Some(bounded_doc_id),
		doc_author_profile_id: Some(doc_author_profile_id.clone()),
		doc_node_id: Some(bounded_doc_node_id),
		status: Status::Active,
	};

	Pallet::<T>::record_activity(&registry_id, tx_hash, b"RegistryStoreCreated")?;
	Registries::<T>::insert(&registry_id, details);
	Delegates::<T>::insert(&registry_id, &profile_id, Permissions::all());

	/* Add the doc_author as a delegate for having permission to create entry */
	Delegates::<T>::insert(&registry_id, &doc_author_profile_id, Permissions::ENTRY);

	Ok(registry_id)
}

/// Update a existing registry.
pub fn update_registry_hash<T: crate::Config>(
	registry_id: &RegistryIdentifierOf,
	who: &ProfileIdOf,
	tx_hash: &HashOf<T>,
) -> DispatchResult {
	Registries::<T>::try_mutate(registry_id, |maybe_registry| -> DispatchResult {
		let registry = maybe_registry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;

		ensure!(
			Pallet::<T>::has_permission(registry_id, &who, Permissions::ADMIN),
			Error::<T>::UnauthorizedOperation
		);

		ensure!(registry.status == Status::Active, Error::<T>::ArchivedRegistry);

		registry.tx_hash = *tx_hash;

		Ok(())
	})?;
	Pallet::<T>::record_activity(registry_id, *tx_hash, b"RegistryHashUpdated")?;

	Ok(())
}

/// Archive a registry.
pub fn archive_registry<T: crate::Config>(
	registry_id: &RegistryIdentifierOf,
	who: &ProfileIdOf,
) -> DispatchResult {
	let tx_hash =
		Registries::<T>::try_mutate(registry_id, |maybe_registry| -> Result<T::Hash, Error<T>> {
			let registry = maybe_registry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(
				Pallet::<T>::has_permission(registry_id, who, Permissions::ADMIN),
				Error::<T>::UnauthorizedOperation
			);
			ensure!(registry.status == Status::Active, Error::<T>::ArchivedRegistry);

			// Capture the *current* tx_hash before we overwrite status
			let old_hash = registry.tx_hash;
			registry.status = Status::Archived;
			Ok(old_hash)
		})?;

	Pallet::<T>::record_activity(registry_id, tx_hash, b"RegistryArchived")?;
	Ok(())
}

/// Restore an archived registry.
pub fn restore_registry<T: crate::Config>(
	registry_id: &RegistryIdentifierOf,
	who: &ProfileIdOf,
) -> DispatchResult {
	let tx_hash =
		Registries::<T>::try_mutate(registry_id, |maybe_registry| -> Result<T::Hash, Error<T>> {
			let registry = maybe_registry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(
				Pallet::<T>::has_permission(registry_id, who, Permissions::ADMIN),
				Error::<T>::UnauthorizedOperation
			);
			ensure!(registry.status == Status::Archived, Error::<T>::RegistryNotArchived);

			let old_hash = registry.tx_hash;
			registry.status = Status::Active;
			Ok(old_hash)
		})?;

	Pallet::<T>::record_activity(registry_id, tx_hash, b"RegistryRestored")?;
	Ok(())
}

/// Update the document author for a registry.
pub fn update_registry_author<T: crate::Config>(
	registry_id: &RegistryIdentifierOf,
	new_doc_author_profile_id: ProfileIdOf,
	who: ProfileIdOf,
) -> DispatchResult {
	ensure!(
		crate::pallet::Pallet::<T>::has_permission(registry_id, &who, Permissions::ADMIN),
		Error::<T>::UnauthorizedOperation
	);

	let mut registry = Registries::<T>::get(registry_id).ok_or(Error::<T>::RegistryNotFound)?;
	ensure!(registry.status == Status::Active, Error::<T>::ArchivedRegistry);

	let old_author = registry.doc_author_profile_id.take();
	registry.doc_author_profile_id = Some(new_doc_author_profile_id.clone());

	Pallet::<T>::record_activity(&registry_id, registry.tx_hash, b"RegistryAuthorUpdated")?;

	Registries::<T>::insert(registry_id, registry);

	// TODO:
	// Revisit if the permission level has to be ENTRY or all()
	Delegates::<T>::insert(registry_id, &new_doc_author_profile_id, Permissions::all());
	if let Some(old) = old_author {
		Delegates::<T>::remove(registry_id, &old);
	}
	Ok(())
}

/// Update registry creator
pub fn update_registry_creator<T: crate::Config>(
	registry_id: &RegistryIdentifierOf,
	new_profile_id: ProfileIdOf,
	who: &ProfileIdOf,
) -> DispatchResult {
	let tx_hash =
		Registries::<T>::try_mutate(registry_id, |maybe_registry| -> Result<T::Hash, Error<T>> {
			let registry = maybe_registry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(
				Pallet::<T>::has_permission(registry_id, who, Permissions::ADMIN),
				Error::<T>::UnauthorizedOperation
			);

			let old_hash = registry.tx_hash;
			registry.creator = new_profile_id.clone();
			Ok(old_hash)
		})?;

	Delegates::<T>::insert(registry_id, &new_profile_id, Permissions::all());
	Delegates::<T>::insert(registry_id, who, Permissions::ENTRY);

	Pallet::<T>::record_activity(registry_id, tx_hash, b"RegistryCreatorUpdated")?;
	Ok(())
}
