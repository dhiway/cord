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

/// An on-chain stream details mapper to an Identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct MetaParams<T: Config> {
	/// Identifier
	pub identifier: IdentifierOf,
	/// Hash.
	pub digest: HashOf<T>,
	/// Controller.
	pub controller: CordAccountOf<T>,
}

impl<T: Config> sp_std::fmt::Debug for MetaParams<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config> MetaParams<T> {
	pub fn add_to_identitifier(
		identifier: &IdentifierOf,
		requestor: CordAccountOf<T>,
		status: bool,
	) -> Result<(), Error<T>> {
		let ident = ss58identifier::from_known_identifier(identifier)
			.map_err(|_| Error::<T>::InvalidIdentifier)?;

		match ident {
			SPACE_INDEX => {
				pallet_space::SpaceDetails::<T>::set_space_metadata(identifier, requestor, status)
					.map_err(|_| Error::<T>::UnauthorizedOperation)?;
			},
			SCHEMA_PREFIX => {
				pallet_schema::SchemaDetails::<T>::set_schema_metadata(
					identifier, requestor, status,
				)
				.map_err(|_| Error::<T>::UnauthorizedOperation)?;
			},
			STREAM_PREFIX => {
				pallet_stream::StreamDetails::<T>::set_stream_metadata(
					identifier, requestor, status,
				)
				.map_err(|_| Error::<T>::UnauthorizedOperation)?;
			},
			_ => return Err(Error::<T>::InvalidIdentifier),
		}
		Ok(())
	}
}

#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct MetadataEntry<T: Config> {
	pub metadata: MetaDataOf,
	pub digest: HashOf<T>,
	pub controller: CordAccountOf<T>,
}

#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct MetaDeposit<T: Config> {
	pub author: CordAccountOf<T>,
	pub deposit: BalanceOf<T>,
}
