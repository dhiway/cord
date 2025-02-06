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

use crate::{
	pallet::Pallet, CordAccountOf, Delegates, Error, PermissionVariant, Permissions, Ss58Identifier,
};
use frame_support::pallet_prelude::*;

/// Adds a delegate with the given permissions, after verifying that
/// the caller (`who`) has ADMIN or DELEGATE permissions.
pub fn add_delegate<T: crate::Config>(
	identifier: &Ss58Identifier,
	who: &CordAccountOf<T>,
	delegate: &CordAccountOf<T>,
	roles: Vec<PermissionVariant>,
) -> DispatchResult {
	ensure!(
		Pallet::<T>::has_permission(identifier, who, Permissions::ADMIN | Permissions::DELEGATE),
		Error::<T>::UnauthorizedOperation
	);

	if Delegates::<T>::contains_key(identifier, delegate) {
		Err(Error::<T>::DelegateAlreadyExists.into())
	} else {
		let permissions = Permissions::from_variants(&roles);
		Pallet::<T>::record_activity(&identifier, b"DelegateAdded")?;
		Delegates::<T>::insert(identifier, delegate, permissions);
		Ok(())
	}
}

/// Removes a delegate after verifying that the caller (`who`)
/// has ADMIN permission.
pub fn remove_delegate<T: crate::Config>(
	identifier: &Ss58Identifier,
	who: &CordAccountOf<T>,
	delegate: &CordAccountOf<T>,
) -> DispatchResult {
	ensure!(
		Pallet::<T>::has_permission(identifier, who, Permissions::ADMIN),
		Error::<T>::UnauthorizedOperation
	);

	if Delegates::<T>::contains_key(identifier, delegate) {
		Pallet::<T>::record_activity(&identifier, b"DelegateRemoved")?;
		Delegates::<T>::remove(identifier, delegate);
		Ok(())
	} else {
		Err(Error::<T>::DelegateNotFound.into())
	}
}
