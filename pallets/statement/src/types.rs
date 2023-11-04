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

/// An on-chain statement entry mapped to details.
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
pub struct StatementDetails<StatementDigestOf, SchemaIdOf, RegistryIdOf> {
	/// Statement hash.
	pub digest: StatementDigestOf,
	/// Registry Identifier
	pub registry: RegistryIdOf,
	/// Schema Identifier
	pub schema: Option<SchemaIdOf>,
}

/// An on-chain statement:digest entry mapped to statement entry details.
/// `StatementEntryDetails` is a struct that contains a `StatementCreatorIdOf`,
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
pub struct StatementEntryStatus<StatementCreatorIdOf, StatusOf> {
	/// Statement status updater.
	pub creator: StatementCreatorIdOf,
	/// The flag indicating the status of the statement entry.
	pub revoked: StatusOf,
}
