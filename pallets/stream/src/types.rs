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

use crate::*;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

/// A global index, formed as the extrinsic index within a block, together with
/// that block's height.
#[derive(
	Copy, Clone, Eq, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
pub struct Timepoint<BlockNumber> {
	/// The height of the chain at the point in time.
	pub height: BlockNumber,
	/// The index of the extrinsic at the point in time.
	pub index: u32,
}

/// An on-chain stream entry mapped to an Identifier.
/// `StreamEntry` is a struct that contains a `StreamDigestOf`, a
/// `StreamCreatorIdOf`, a `SchemaIdOf`, a `RegistryIdOf`, and a `StatusOf`.
///
/// Properties:
///
/// * `digest`: The hash of the stream.
/// * `creator`: The account that created the stream.
/// * `schema`: The schema identifier.
/// * `registry`: The registry that the stream is associated with.
/// * `revoked`: This is a boolean flag that indicates whether the stream is
///   revoked or not.
#[derive(
	Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct StreamEntry<StreamDigestOf, StreamCreatorIdOf, SchemaIdOf, RegistryIdOf, StatusOf> {
	/// Stream hash.
	pub digest: StreamDigestOf,
	/// Stream creator.
	pub creator: StreamCreatorIdOf,
	/// Schema Identifier
	pub schema: Option<SchemaIdOf>,
	/// Registry Identifier
	pub registry: RegistryIdOf,
	/// The flag indicating the status of the stream.
	pub revoked: StatusOf,
}

/// `StreamCommit` is a struct that contains a `StreamCommitAction`, a
/// `StreamDigest`, a `StreamCreatorId`, and a `Timepoint`.
///
/// Properties:
///
/// * `commit`: The type of commit.
/// * `digest`: The hash of the stream.
/// * `committed_by`: The account that committed the stream.
/// * `created_at`: The block number at which the stream was created.
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub struct StreamCommit<StreamCommitActionOf, StreamDigestOf, StreamCreatorIdOf, BlockNumber> {
	/// Stream commit type
	pub commit: StreamCommitActionOf,
	/// Stream hash.
	pub digest: StreamDigestOf,
	/// Registry delegate.
	pub committed_by: StreamCreatorIdOf,
	/// Stream block number
	pub created_at: Timepoint<BlockNumber>,
}

/// Defining the possible actions that can be taken on a stream.
#[derive(Clone, Copy, RuntimeDebug, Decode, Encode, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum StreamCommitActionOf {
	Genesis,
	Update,
	Revoke,
	Restore,
	Remove,
	Digest,
}
