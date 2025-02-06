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

use alloc::vec::Vec;
use codec::Encode;
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::*;
use sp_runtime::traits::Hash;

use crate::{
	pallet::Pallet, CordAccountOf, Delegates, Error, HashOf, Identifier, Permissions, Registries,
	RegistryDetails, RegistryIdentifierOf, Status,
};

pub fn push_option(buf: &mut Vec<u8>, field: Option<&[u8]>) {
	if let Some(bytes) = field {
		buf.push(1u8);
		buf.extend_from_slice(bytes);
	} else {
		buf.push(0u8);
	}
}

/// Create a new registry.
pub fn create_registry<T: crate::Config>(
	tx_hash: HashOf<T>,
	doc_id: Option<Vec<u8>>,
	doc_author_id: Option<CordAccountOf<T>>,
	doc_node_id: Option<Vec<u8>>,
	creator: CordAccountOf<T>,
) -> Result<RegistryIdentifierOf, sp_runtime::DispatchError> {
	let bounded_doc_id = doc_id
		.map(|v| v.try_into())
		.transpose()
		.map_err(|_| Error::<T>::InvalidIdentifierLength)?;
	let bounded_doc_node_id = doc_node_id
		.map(|v| v.try_into())
		.transpose()
		.map_err(|_| Error::<T>::InvalidIdentifierLength)?;
	let encoded_doc_author = doc_author_id.as_ref().map(|author| author.encode());

	let mut data = Vec::with_capacity(256);
	data.extend_from_slice(tx_hash.as_ref());
	push_option(
		&mut data,
		bounded_doc_id.as_ref().map(|v: &BoundedVec<u8, ConstU32<64>>| v.as_slice()),
	);
	push_option(&mut data, encoded_doc_author.as_ref().map(|v| v.as_slice()));
	push_option(
		&mut data,
		bounded_doc_node_id
			.as_ref()
			.map(|v: &BoundedVec<u8, ConstU32<64>>| v.as_slice()),
	);
	data.extend_from_slice(&creator.encode());

	let digest = T::Hashing::hash(&data);
	let pallet_name = <crate::pallet::Pallet<T> as frame_support::traits::PalletInfoAccess>::name();

	let registry_id =
		<cord_uri::Pallet<T> as Identifier>::build(&(digest).encode()[..], pallet_name)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

	ensure!(!Registries::<T>::contains_key(&registry_id), Error::<T>::RegistryAlreadyExists);

	let details = RegistryDetails {
		creator: creator.clone(),
		tx_hash,
		doc_id: bounded_doc_id,
		doc_author_id: doc_author_id.clone(),
		doc_node_id: bounded_doc_node_id,
		status: Status::Active,
	};

	Pallet::<T>::record_activity(&registry_id, b"RegistryCreated")?;
	Registries::<T>::insert(&registry_id, details);
	Delegates::<T>::insert(&registry_id, &creator, Permissions::all());
	if let Some(author) = doc_author_id {
		Delegates::<T>::insert(&registry_id, &author, Permissions::ENTRY);
	}
	Ok(registry_id)
}

/// Archive a registry.
pub fn archive_registry<T: crate::Config>(
	registry_id: &RegistryIdentifierOf,
	who: CordAccountOf<T>,
) -> DispatchResult {
	Registries::<T>::try_mutate(registry_id, |maybe_registry| -> DispatchResult {
		let registry = maybe_registry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
		ensure!(
			Pallet::<T>::has_permission(registry_id, &who, Permissions::ADMIN),
			Error::<T>::UnauthorizedOperation
		);
		ensure!(registry.status == Status::Active, Error::<T>::ArchivedRegistry);
		registry.status = Status::Archived;
		Ok(())
	})?;
	Pallet::<T>::record_activity(registry_id, b"RegistryArchived")?;
	Ok(())
}

/// Restore an archived registry.
pub fn restore_registry<T: crate::Config>(
	registry_id: &RegistryIdentifierOf,
	who: CordAccountOf<T>,
) -> DispatchResult {
	Registries::<T>::try_mutate(registry_id, |maybe_registry| -> DispatchResult {
		let registry = maybe_registry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
		ensure!(
			Pallet::<T>::has_permission(registry_id, &who, Permissions::ADMIN),
			Error::<T>::UnauthorizedOperation
		);
		ensure!(registry.status == Status::Archived, Error::<T>::RegistryNotArchived);
		registry.status = Status::Active;
		Ok(())
	})?;
	Pallet::<T>::record_activity(registry_id, b"RegistryRestored")?;
	Ok(())
}

/// Update the document author for a registry.
pub fn update_registry_author<T: crate::Config>(
	registry_id: &RegistryIdentifierOf,
	new_doc_author_id: CordAccountOf<T>,
	who: CordAccountOf<T>,
) -> DispatchResult {
	ensure!(
		crate::pallet::Pallet::<T>::has_permission(registry_id, &who, Permissions::ADMIN),
		Error::<T>::UnauthorizedOperation
	);

	let mut registry = Registries::<T>::get(registry_id).ok_or(Error::<T>::RegistryNotFound)?;
	ensure!(registry.status == Status::Active, Error::<T>::ArchivedRegistry);

	let old_author = registry.doc_author_id.take();
	registry.doc_author_id = Some(new_doc_author_id.clone());

	Pallet::<T>::record_activity(&registry_id, b"RegistryUpdated")?;

	Registries::<T>::insert(registry_id, registry);
	Delegates::<T>::insert(registry_id, &new_doc_author_id, Permissions::all());
	if let Some(old) = old_author {
		Delegates::<T>::remove(registry_id, &old);
	}
	Ok(())
}

/// Update registry creator
pub fn update_registry_creator<T: crate::Config>(
	registry_id: &RegistryIdentifierOf,
	new_creator: CordAccountOf<T>,
	who: CordAccountOf<T>,
) -> DispatchResult {
	Registries::<T>::try_mutate(registry_id, |maybe_registry| -> DispatchResult {
		let registry = maybe_registry.as_mut().ok_or(Error::<T>::RegistryNotFound)?;
		ensure!(
			crate::pallet::Pallet::<T>::has_permission(registry_id, &who, Permissions::ADMIN),
			Error::<T>::UnauthorizedOperation
		);
		registry.creator = new_creator;
		Ok(())
	})?;

	Pallet::<T>::record_activity(&registry_id, b"RegistryCreatorUpdated")?;
	Ok(())
}
