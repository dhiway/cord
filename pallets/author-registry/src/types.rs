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
use sp_runtime::RuntimeDebug;

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct ProposalInfo<CordAccountOf, CordProposerAccountOf, BlockNumberOf, BalanceOf> {
	/// Author
	pub author: CordAccountOf,
	/// Author Credits transferred
	pub credits: BalanceOf,
	/// Author Proposer
	pub parent: CordProposerAccountOf,
	/// Proposer parent block
	pub pblock: BlockNumberOf,
}

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct ProposalEntry<
	CordAccountOf,
	CordProposerAccountOf,
	BlockNumberOf,
	BalanceOf,
	AnchorBlockNumberOf,
	StatusOf,
> {
	/// Author
	pub author: CordAccountOf,
	/// Author Credits transferred
	pub credits: BalanceOf,
	/// Author Proposer
	pub parent: CordProposerAccountOf,
	/// Proposer parent block
	pub pblock: BlockNumberOf,
	/// Author anchored block
	pub ablock: AnchorBlockNumberOf,
	/// Status of the author - only council can change this
	pub retired: StatusOf,
}
