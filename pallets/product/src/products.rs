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

/// An on-chain product details mapper to an Identifier.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ProductDetails<T: Config> {
	/// Stream hash.
	pub tx_hash: HashOf<T>,
	/// \[OPTIONAL\] Product Information CID.
	pub cid: Option<IdentifierOf>,
	/// \[OPTIONAL\] Product previous CID.
	pub parent_cid: Option<IdentifierOf>,
	/// \[OPTIONAL\] Store Identifier.
	pub store_id: Option<IdOf<T>>,
	/// \[OPTIONAL\] Stream Schema
	pub schema: Option<IdOf<T>>,
	/// \[OPTIONAL\] Stream Link
	pub link: Option<IdOf<T>>,
	/// Stream controller.
	pub creator: CordAccountOf<T>,
	/// Product price in INR as currency basis
	// pub price: Option<f32>,
	/// The number of times a given product has been upvoted.
	pub rating: Option<u32>,
	/// Stream block number
	pub block: BlockNumberOf<T>,
	/// The flag indicating the availability status of the product.
	pub status: StatusOf,
}

/// An on-chain link details.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ProductLink<T: Config> {
	/// Stream link.
	pub identifier: IdOf<T>,
	/// Store link.
	pub store_id: IdOf<T>,
	/// Link creator.
	pub creator: CordAccountOf<T>,
}

impl<T: Config> ProductLink<T> {
	pub fn link_tx(product: &IdOf<T>, link: ProductLink<T>) -> DispatchResult {
		let mut links = <Links<T>>::get(product).unwrap_or_default();
		links.push(link);
		<Links<T>>::insert(product, links);
		Ok(())
	}
}

/// An on-chain commit details.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ProductCommit<T: Config> {
	/// Stream hash.
	pub tx_hash: HashOf<T>,
	/// \[OPTIONAL\] Stream CID
	pub cid: Option<IdentifierOf>,
	/// Stream block number
	pub block: BlockNumberOf<T>,
	/// Stream commit type
	pub commit: ProductCommitOf,
}

impl<T: Config> ProductCommit<T> {
	pub fn store_tx(identifier: &IdOf<T>, tx_commit: ProductCommit<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(identifier).unwrap_or_default();
		commit.push(tx_commit);
		<Commits<T>>::insert(identifier, commit);
		Ok(())
	}

	pub fn update_tx(identifier: &IdOf<T>, tx_commit: ProductCommit<T>) -> DispatchResult {
		let mut commit = <Commits<T>>::get(identifier).unwrap();
		commit.push(tx_commit);
		<Commits<T>>::insert(identifier, commit);
		Ok(())
	}
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo)]
pub enum ProductCommitOf {
	Create,
	List,
	Update,
	Order,
	Return,
	Rating,
	StatusChange,
}
