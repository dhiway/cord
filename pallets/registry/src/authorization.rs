// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 Dhiway Networks Pvt. Ltd.
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
use frame_support::ensure;
use scale_info::TypeInfo;
use sp_runtime::DispatchError;

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct AuthorityAc<T: Config> {
	pub(crate) tx_auth_id: AuthorizationIdOf<T>,
}

impl<T: Config>
	pallet_stream::StreamAuthorization<ProposerIdOf<T>, SchemaIdOf, AuthorizationIdOf<T>>
	for AuthorityAc<T>
{
	fn can_create(
		&self,
		who: &ProposerIdOf<T>,
		schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError> {
		let authorization_details = Authorizations::<T>::get(self.authorization_id())
			.ok_or(Error::<T>::AuthorizationNotFound)?;
		let registry_details = Registries::<T>::get(&authorization_details.registry)
			.ok_or(Error::<T>::RegistryNotFound)?;
		ensure!(!registry_details.archive, Error::<T>::ArchivedRegistry);
		ensure!(
			// has permission
			((authorization_details.permissions & Permissions::ASSERT) == Permissions::ASSERT)
				// is the delegate
				&& &authorization_details.delegate == who
				// authorization matches the schema
				&& &registry_details.schema == schema,
			Error::<T>::AccessDenied
		);

		Ok(authorization_details.registry)
	}

	fn can_update(
		&self,
		who: &ProposerIdOf<T>,
		schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError> {
		self.can_create(who, schema)
	}

	fn can_set_status(
		&self,
		who: &ProposerIdOf<T>,
		_schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError> {
		let authorization_details = Authorizations::<T>::get(self.authorization_id())
			.ok_or(Error::<T>::AuthorizationNotFound)?;
		let registry_details = Registries::<T>::get(&authorization_details.registry)
			.ok_or(Error::<T>::RegistryNotFound)?;
		ensure!(!registry_details.archive, Error::<T>::ArchivedRegistry);
		ensure!(
			// has permission
			((authorization_details.permissions & Permissions::ADMIN) == Permissions::ADMIN)
				// is the delegate
				&& &authorization_details.delegate == who,
			Error::<T>::AccessDenied
		);

		Ok(authorization_details.registry)
	}

	fn can_remove(
		&self,
		who: &ProposerIdOf<T>,
		schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError> {
		self.can_set_status(who, schema)
	}

	fn authorization_id(&self) -> AuthorizationIdOf<T> {
		self.tx_auth_id.clone()
	}
}
