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
use codec::{Decode, Encode};
use scale_info::TypeInfo;

/// An on-chain stream details mapper to an Identifier.
#[derive(Clone, Debug, Encode, Decode, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct StreamDetails<T: Config> {
	/// Stream hash.
	pub stream_hash: HashOf<T>,
	/// Stream controller.
	pub controller: CordAccountOf<T>,
	/// Stream holder.
	pub holder: Option<CordAccountOf<T>>,
	/// \[OPTIONAL\] Schema Identifier
	pub schema: Option<IdentifierOf>,
	/// \[OPTIONAL\] Stream Link
	pub link: Option<IdentifierOf>,
	/// \[OPTIONAL\] Space ID.
	pub space: Option<IdentifierOf>,
	/// The flag indicating the status of the stream.
	pub revoked: StatusOf,
}
