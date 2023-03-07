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
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;

/// A global index, formed as the extrinsic index within a block, together with that block's height.
#[derive(
	Copy, Clone, Eq, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
pub struct Timepoint<BlockNumber> {
	/// The height of the chain at the point in time.
	pub height: BlockNumber,
	/// The index of the extrinsic at the point in time.
	pub index: u32,
}

// pub struct StreamInput<StreamDigestOf, CreatorIdOf, SchemaIdOf> {
// 	/// Stream hash.
// 	pub digest: HashOf<T>,
// 	/// Stream controller.
// 	pub controller: CordAccountOf<T>,
// 	/// Stream holder.
// 	pub holder: CordAccountOf<T>,
// 	/// \[OPTIONAL\] Schema Identifier
// 	pub schema: Option<IdentifierOf>,
// 	/// \[OPTIONAL\] Stream Link
// 	pub linked: Option<IdentifierOf>,
// 	/// \[OPTIONAL\] Registry ID.
// 	pub space: Option<IdentifierOf>,
// }

/// An on-chain stream entry mapped to an Identifier.
#[derive(
	Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]

pub struct StreamEntry<StreamDigestOf, CreatorIdOf, SchemaIdOf, RegistryIdOf, StatusOf> {
	/// Stream hash.
	pub digest: StreamDigestOf,
	/// Stream creator.
	pub creator: CreatorIdOf,
	/// Schema Identifier
	pub schema: SchemaIdOf,
	/// Registry Identifier
	pub registry: RegistryIdOf,
	/// The flag indicating the status of the stream.
	pub revoked: StatusOf,
}

#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub struct StreamCommit<StreamCommitActionOf, StreamDigestOf, CreatorIdOf, BlockNumber> {
	/// Stream commit type
	pub commit: StreamCommitActionOf,
	/// Stream hash.
	pub digest: StreamDigestOf,
	/// Registry delegate.
	pub committed_by: CreatorIdOf,
	/// Stream block number
	pub created_at: Timepoint<BlockNumber>,
}

#[derive(Clone, Copy, RuntimeDebug, Decode, Encode, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum StreamCommitActionOf {
	Genesis,
	Update,
	Status,
}

// /// An on-chain stream details mapper to an Identifier.
// #[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
// #[scale_info(skip_type_params(T))]
// #[codec(mel_bound())]
// pub struct StreamType<T: Config> {
// 	/// Stream hash.
// 	pub digest: HashOf<T>,
// 	/// Stream controller.
// 	pub controller: CordAccountOf<T>,
// 	/// Stream holder.
// 	pub holder: CordAccountOf<T>,
// 	/// \[OPTIONAL\] Schema Identifier
// 	pub schema: Option<IdentifierOf>,
// 	/// \[OPTIONAL\] Stream Link
// 	pub link: Option<IdentifierOf>,
// 	/// \[OPTIONAL\] Registry ID.
// 	pub space: Option<IdentifierOf>,
// }

// impl<T: Config> sp_std::fmt::Debug for StreamType<T> {
// 	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
// 		Ok(())
// 	}
// }

// /// An on-chain stream details mapper to an Identifier.
// #[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
// #[scale_info(skip_type_params(T))]
// #[codec(mel_bound())]
// pub struct StreamDetails<T: Config> {
// 	/// Stream hash.
// 	pub stream: StreamType<T>,
// 	/// The flag indicating the status of the stream.
// 	pub revoked: StatusOf,
// 	/// The flag indicating the status of metadata.
// 	pub meta: StatusOf,
// 	/// The flag indicating the status of delegation.
// 	pub delegates: StatusOf,
// }

// impl<T: Config> sp_std::fmt::Debug for StreamDetails<T> {
// 	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
// 		Ok(())
// 	}
// }

// impl<T: Config> StreamDetails<T> {
// 	pub fn set_stream_metadata(
// 		tx_stream: &IdentifierOf,
// 		requestor: CordAccountOf<T>,
// 		status: bool,
// 	) -> Result<(), Error<T>> {
// 		let stream_details = <Streams<T>>::get(&tx_stream).ok_or(Error::<T>::StreamNotFound)?;
// 		ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

// 		if stream_details.stream.controller != requestor {
// 			if let Some(ref space) = stream_details.stream.space {
// 				pallet_space::SpaceDetails::<T>::from_space_identities(space, requestor)
// 					.map_err(|_| Error::<T>::UnauthorizedOperation)?;
// 			}
// 		} else {
// 			ensure!(
// 				stream_details.stream.controller == requestor,
// 				Error::<T>::UnauthorizedOperation
// 			);
// 		}

// 		<Streams<T>>::insert(&tx_stream, StreamDetails { meta: status, ..stream_details });

// 		Ok(())
// 	}
// }

// /// An on-chain schema details mapped to an identifier.
// #[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
// #[scale_info(skip_type_params(T))]
// #[codec(mel_bound())]
// pub struct StreamParams<T: Config> {
// 	/// Schema identifier
// 	pub identifier: IdentifierOf,
// 	/// Schema hash.
// 	pub stream: StreamType<T>,
// }

// impl<T: Config> sp_std::fmt::Debug for StreamParams<T> {
// 	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
// 		Ok(())
// 	}
// }

// /// An on-chain schema details mapped to an identifier.
// #[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
// #[scale_info(skip_type_params(T))]
// #[codec(mel_bound())]
// pub struct StreamDelegationParams<T: Config> {
// 	/// Stream identifier
// 	pub identifier: IdentifierOf,
// 	/// Stream hash.
// 	pub digest: HashOf<T>,
// 	/// Stream controller.
// 	pub delegator: CordAccountOf<T>,
// }

// impl<T: Config> sp_std::fmt::Debug for StreamDelegationParams<T> {
// 	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
// 		Ok(())
// 	}
// }
