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

/// An on-chain statement entry mapped to an Identifier.
/// `StatementEntry` is a struct that contains a `StatementDigestOf`, a
/// `SchemaIdOf`, and a `RegistryIdOf`.
///
/// Properties:
///
/// * `digest`: The hash of the statement.
/// * `schema`: The schema identifier.
/// * `registry`: The registry that the statement is associated with.
#[derive(
	Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct StatementEntry<StatementDigestOf, SchemaIdOf, RegistryIdOf> {
	/// Statement hash.
	pub digest: StatementDigestOf,
	/// Schema Identifier
	pub schema: Option<SchemaIdOf>,
	/// Registry Identifier
	pub registry: RegistryIdOf,
}

/// An on-chain statement:digest entry mapped to an Identifier:Hash.
/// `AttestationDetails` is a struct that contains a `StatementCreatorIdOf`,
/// and a `StatusOf`.
///
/// Properties:
///
/// * `creator`: The account that has done the latest operation (creator on
///   create and update, and account revoking for revoke)
/// * `revoked`: This is a boolean flag that indicates whether the statement is
///   revoked or not.
#[derive(
	Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct AttestationDetails<StatementCreatorIdOf, StatusOf> {
	/// Attestation creator.
	pub creator: StatementCreatorIdOf,
	/// The flag indicating the status of the statement.
	pub revoked: StatusOf,
}

/// `StatementCommit` is a struct that contains a `StatementCommitAction`, a
/// `StatementDigest`, a `StatementCreatorId`, and a `Timepoint`.
///
/// Properties:
///
/// * `commit`: The type of commit.
/// * `digest`: The hash of the statement.
/// * `committed_by`: The account that committed the statement.
/// * `created_at`: The block number at which the statement was created.
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub struct StatementCommit<
	StatementCommitActionOf,
	StatementDigestOf,
	StatementCreatorIdOf,
	BlockNumber,
> {
	/// Statement commit type
	pub commit: StatementCommitActionOf,
	/// Statement hash.
	pub digest: StatementDigestOf,
	/// Registry delegate.
	pub committed_by: StatementCreatorIdOf,
	/// Statement block number
	pub created_at: Timepoint<BlockNumber>,
}

/// Defining the possible actions that can be taken on a statement.
#[derive(Clone, Copy, RuntimeDebug, Decode, Encode, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum StatementCommitActionOf {
	Genesis,
	Update,
	Revoke,
	Restore,
	Remove,
	Digest,
}
