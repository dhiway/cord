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
#[derive(Clone, Debug, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct SpaceDetails<T: Config> {
	/// Space hash.
	pub space_hash: HashOf<T>,
	/// Space creator.
	pub controller: CordAccountOf<T>,
	/// The flag indicating the status of the space.
	pub archived: StatusOf,
}

impl<T: Config> SpaceDetails<T> {
	pub fn from_space_identities(
		tx_space: &IdentifierOf,
		requestor: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		mark::from_known_format(tx_space, SPACE_IDENTIFIER_PREFIX)
			.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

		let space_details = <Spaces<T>>::get(&tx_space).ok_or(Error::<T>::SpaceNotFound)?;
		ensure!(!space_details.archived, Error::<T>::ArchivedSpace);

		if space_details.controller != requestor {
			let delegates = <Delegations<T>>::get(tx_space);
			ensure!(
				(delegates.iter().find(|&delegate| *delegate == requestor) == Some(&requestor)),
				Error::<T>::UnauthorizedOperation
			);
		}
		Ok(())
	}
}
