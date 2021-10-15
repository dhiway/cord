// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2021 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::*;
use codec::{Decode, Encode};
use sp_runtime::DispatchResult;

/// An on-chain stream details mapper to an Identifier.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct StreamDetails<T: Config> {
	/// Stream hash.
	pub hash: HashOf<T>,
	/// \[OPTIONAL\] Stream CID.
	pub cid: Option<IdentifierOf>,
	/// \[OPTIONAL\] Stream previous CID.
	pub parent_cid: Option<IdentifierOf>,
	/// \[OPTIONAL\] Stream Schema
	pub schema: Option<IdOf<T>>,
	/// \[OPTIONAL\] Stream Link
	pub link: Option<IdOf<T>>,
	/// Stream controller.
	pub creator: CordAccountOf<T>,
	/// Stream block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the status of the stream.
	pub revoked: StatusOf,
}

/// An on-chain link details.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct StreamLink<T: Config> {
	/// Stream link.
	pub identifier: IdOf<T>,
	/// Link creator.
	pub creator: CordAccountOf<T>,
}

impl<T: Config> StreamLink<T> {
	pub fn link_tx(stream: &IdOf<T>, link: StreamLink<T>) -> DispatchResult {
		let mut links = <Links<T>>::get(stream).unwrap_or_default();
		links.push(link);
		<Links<T>>::insert(stream, links);
		Ok(())
	}
}

/// An on-chain commit details.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct StreamCommit<T: Config> {
	/// Stream hash.
	pub hash: HashOf<T>,
	/// \[OPTIONAL\] Stream CID
	pub cid: Option<IdentifierOf>,
	/// Stream block number
	pub block: BlockNumberOf<T>,
	/// Stream commit type
	pub commit: StreamCommitOf,
}

impl<T: Config> StreamCommit<T> {
	pub fn store_tx(identifier: &IdOf<T>, tx_commit: StreamCommit<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(identifier).unwrap_or_default();
		commit.push(tx_commit);
		<Commits<T>>::insert(identifier, commit);
		Ok(())
	}

	pub fn update_tx(identifier: &IdOf<T>, tx_commit: StreamCommit<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(identifier).unwrap();
		commit.push(tx_commit);
		<Commits<T>>::insert(identifier, commit);
		Ok(())
	}
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo)]
pub enum StreamCommitOf {
	Genesis,
	Update,
	StatusChange,
}
