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
// #[codec(mel_bound())]
pub struct StreamType<T: Config> {
	/// Stream hash.
	pub digest: HashOf<T>,
	/// Stream controller.
	pub author: CordAccountOf<T>,
	/// Stream holder.
	pub holder: Option<CordAccountOf<T>>,
	/// \[OPTIONAL\] Schema Identifier
	pub schema: Option<IdentifierOf>,
	/// \[OPTIONAL\] Stream Link
	pub link: Option<IdentifierOf>,
	/// \[OPTIONAL\] Space ID.
	pub space: Option<IdentifierOf>,
}

impl<T: Config> sp_std::fmt::Debug for StreamType<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

/// An on-chain stream details mapper to an Identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
// #[codec(mel_bound())]
pub struct StreamDetails<T: Config> {
	/// Stream hash.
	pub stream: StreamType<T>,
	/// The flag indicating the status of the stream.
	pub revoked: StatusOf,
	/// The flag indicating the status of the metadata.
	pub metadata: StatusOf,
}

impl<T: Config> sp_std::fmt::Debug for StreamDetails<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config> StreamDetails<T> {
	pub fn set_stream_metadata(
		tx_stream: &IdentifierOf,
		requestor: CordAccountOf<T>,
		status: bool,
	) -> Result<(), Error<T>> {
		let stream_details = <Streams<T>>::get(&tx_stream).ok_or(Error::<T>::StreamNotFound)?;
		ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

		if stream_details.stream.author != requestor {
			if let Some(ref space) = stream_details.stream.space {
				pallet_space::SpaceDetails::<T>::from_space_identities(space, requestor)
					.map_err(|_| Error::<T>::UnauthorizedOperation)?;
			}
		} else {
			ensure!(stream_details.stream.author == requestor, Error::<T>::UnauthorizedOperation);
		}

		<Streams<T>>::insert(&tx_stream, StreamDetails { metadata: status, ..stream_details });

		Ok(())
	}
}

/// An on-chain schema details mapped to an identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
// #[codec(mel_bound())]
pub struct StreamParams<T: Config> {
	/// Schema identifier
	pub identifier: IdentifierOf,
	/// Schema hash.
	pub stream: StreamType<T>,
}

impl<T: Config> sp_std::fmt::Debug for StreamParams<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}
