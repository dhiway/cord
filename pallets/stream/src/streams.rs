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

/// An on-chain stream details mapper to an Identifier.
#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct StreamDetails<T: Config> {
	/// Stream hash.
	pub stream_id: IdOf<T>,
	/// Stream controller.
	pub creator: CordAccountOf<T>,
	/// Stream holder.
	pub holder: Option<CordAccountOf<T>>,
	/// \[OPTIONAL\] Schema Identifier
	pub schema: Option<IdOf<T>>,
	/// \[OPTIONAL\] Stream CID.
	pub cid: Option<CidOf>,
	/// \[OPTIONAL\] parent Stream ID.
	pub parent: Option<HashOf<T>>,
	/// \[OPTIONAL\] Stream Link
	pub link: Option<IdOf<T>>,
	/// The flag indicating the status of the stream.
	pub revoked: StatusOf,
}
