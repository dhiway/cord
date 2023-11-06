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

//! # Statement Types Module
//!
//! This module defines the core data types used within the statement system of
//! the blockchain. These types are utilized to manage and represent statements,
//! which are references to off-chain data, within the on-chain environment. The
//! module ensures that statements are created, updated, and queried in a
//! consistent and secure manner.
//!
//! ## Overview
//!
//! The `StatementDetails` type is used to encapsulate the essential identifiers
//! for a statement, including its content's hash, the space it belongs to, and
//! an optional schema for data structure.
//!
//! The `StatementEntryStatus` type records the revocation status of a
//! statement, indicating whether it has been revoked by a particular account.

use crate::*;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

/// `StatementDetails` holds the essential identifiers for a statement within
/// the system. It is used to associate a statement with its content's hash, the
/// space it belongs to, and optionally, a schema that defines the structure of
/// the statement's data.
///
/// ## Fields
///
/// - `digest`: A `StatementDigestOf` type that stores the hash of the
///   statement's content. This hash serves as a cryptographic proof that can be
///   used to verify the integrity of the statement's data, ensuring that the
///   data has not been altered since the hash was generated.
///
/// - `space`: A `SpaceIdOf` type that represents the identifier for the space
///   to which the statement is associated. This identifier helps categorize the
///   statement within a specific context or domain within the system.
///
/// - `schema`: An optional `SchemaIdOf` type that, if provided, points to a
///   schema defining the structure or expected format of the statement's data.
///   This can be used for data validation or to aid in the interpretation of
///   the statement's data.
///
/// ## Usage
///
/// This struct is typically used when creating or querying statements, allowing
/// users to obtain a comprehensive view of a statement's identifiers and to
/// perform operations like verification or categorization based on these
/// identifiers.
#[derive(
	Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct StatementDetails<StatementDigestOf, SchemaIdOf, SpaceIdOf> {
	/// The hash of the statement's content, serving as a unique identifier of
	/// the latest state.
	pub digest: StatementDigestOf,
	/// Identifier for the space associated with the statement.
	pub space: SpaceIdOf,
	/// Optional identifier for the schema describing the statement's data
	/// structure.
	pub schema: Option<SchemaIdOf>,
}

/// `StatementEntryStatus` records the revocation status of a statement. It is a
/// critical component for managing the lifecycle of statements, ensuring that
/// any changes to the revocation status are traceable and auditable.
///
/// ## Fields
///
/// - `creator`: The identifier of the account that has revoked the statement.
///   This field is used to track which account has performed the revocation
///   action.
///
/// - `revoked`: A boolean value that indicates whether the statement has been
///   revoked. A value of `true` confirms that the statement is no longer
///   considered active, while `false` would imply that the statement has not
///   been revoked.
///
/// ## Usage
///
/// This struct is used to enforce the integrity of the revocation process. By
/// keeping a record of the revocation status and the responsible party, the
/// system provides a clear audit trail for the actions taken on statements.
#[derive(
	Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct StatementEntryStatus<StatementCreatorOf, StatusOf> {
	/// The DID identifier for the party responsible for revoking the statement.
	pub creator: StatementCreatorOf,
	/// Indicates whether the statement has been revoked.
	pub revoked: StatusOf,
}
