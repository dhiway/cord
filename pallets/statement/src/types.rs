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
/// - `digest`: A `StatementDigestOf` type that stores the hash of the statement's content. This
///   hash serves as a cryptographic proof that can be used to verify the integrity of the
///   statement's data, ensuring that the data has not been altered since the hash was generated.
///
/// - `space`: A `SpaceIdOf` type that represents the identifier for the space to which the
///   statement is associated. This identifier helps categorize the statement within a specific
///   context or domain within the system.
///
/// - `schema`: An optional `SchemaIdOf` type that, if provided, points to a schema defining the
///   structure or expected format of the statement's data. This can be used for data validation or
///   to aid in the interpretation of the statement's data.
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
/// - `creator`: The identifier of the account that has revoked the statement. This field is used to
///   track which account has performed the revocation action.
///
/// - `revoked`: A boolean value that indicates whether the statement has been revoked. A value of
///   `true` confirms that the statement is no longer considered active, while `false` would imply
///   that the statement has not been revoked.
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

/// Holds the details for a specific presentation of a statement.
///
/// This struct captures the unique identifiers and metadata for a presentation
/// view of a statement, such as a PDF, image, or other visual representation.
/// It is used to link the presentation directly to its corresponding statement
/// and the space within which it is relevant.
///
/// ## Fields
///
/// - `presentation_digest`: A `StatementPresentationDigestOf` type that stores the hash of the
///   presentation's content. This hash acts as a unique identifier for the presentation and ensures
///   its integrity by allowing verification that the presentation has not been altered. e to which
///   both the statement and its presentation are associated. This helps in categorizing and
///   retrieving the presentation within the context of its space.
///
/// - `digest`: A `StatementDigestOf` type that holds the hash of the original statement's content.
///   This serves as a reference back to the statement that the presentation represents, creating a
///   link between the presentation and the statement's actual content.
///
/// - `space`: A `SpaceIdOf` type that signifies the identifier for the space to which both the
///   statement and its presentation are associated. This helps in categorizing and retrieving the
///   presentation within the context of its space.
///
/// ## Usage
///
/// `StatementPresentationDetails` is primarily used when a new presentation is
/// created for a statement or when querying for presentations associated with a
/// particular statement. It allows users and systems to manage and access the
/// various presentations of statements efficiently, ensuring that
/// each presentation can be authenticated and traced back to its original
/// statement and space.
#[derive(
	Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct StatementPresentationDetails<
	StatementCreatorOf,
	PresentationTypeOf,
	StatementDigestOf,
	SpaceIdOf,
> {
	/// The DID identifier for the party responsible for creating the
	/// presentation.
	pub creator: StatementCreatorOf,
	// /// The hash of the presentation's content, serving as a unique identifier.
	// pub presentation_digest: StatementDigestOf,
	/// Type of the presentation media
	pub presentation_type: PresentationTypeOf,
	/// The hash of the statement's content, linking the presentation to its
	/// referenced state.
	pub digest: StatementDigestOf,
	/// The identifier for the space contextualizing the statement and its
	/// presentation.
	pub space: SpaceIdOf,
}

/// Enum representing various file types that could be associated with a
/// statement's presentation.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum PresentationTypeOf {
	/// Represents any other file type not explicitly listed.
	Other,
	/// Represents a Portable Document Format (PDF) file.
	PDF,
	/// Represents a Joint Photographic Experts Group (JPEG) image file.
	JPEG,
	/// Represents a Portable Network Graphics (PNG) image file.
	PNG,
	/// Represents a Graphics Interchange Format (GIF) image file.
	GIF,
	/// Represents a Plain Text (TXT) file.
	TXT,
	/// Represents a Scalable Vector Graphics (SVG) file.
	SVG,
	/// Represents a JavaScript Object Notation (JSON) file.
	JSON,
	/// Represents a Microsoft Word Document (DOCX) file.
	DOCX,
	/// Represents an Excel Spreadsheet (XLSX) file.
	XLSX,
	/// Represents a PowerPoint Presentation (PPTX) file.
	PPTX,
	/// Represents an Audio file (MP3).
	MP3,
	/// Represents a Video file (MP4).
	MP4,
	// Represents an XML file.
	XML,
}

impl PresentationTypeOf {
	/// Returns the typical file extension associated with the file type.
	pub fn extension(&self) -> &str {
		match self {
			PresentationTypeOf::Other => "unknown",
			PresentationTypeOf::PDF => "pdf",
			PresentationTypeOf::JPEG => "jpeg",
			PresentationTypeOf::PNG => "png",
			PresentationTypeOf::GIF => "gif",
			PresentationTypeOf::TXT => "txt",
			PresentationTypeOf::SVG => "svg",
			PresentationTypeOf::JSON => "json",
			PresentationTypeOf::DOCX => "docx",
			PresentationTypeOf::XLSX => "xlsx",
			PresentationTypeOf::PPTX => "pptx",
			PresentationTypeOf::MP3 => "mp3",
			PresentationTypeOf::MP4 => "mp4",
			PresentationTypeOf::XML => "xml",
		}
	}
}

impl MaxEncodedLen for PresentationTypeOf {
	fn max_encoded_len() -> usize {
		1 // Since all variants are unit variants, they encode to a single byte.
	}
}
