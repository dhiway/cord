// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

//! Runtime API definition for Registry Entry

use crate::{Decode, Encode};
use sp_api::decl_runtime_apis;

decl_runtime_apis! {
	pub trait RegistryEntryApi<Hash, Ss58Identifier>
	where
		Hash: Encode + Decode,
		Ss58Identifier: Encode + Decode,
	{
		fn verify_digest(digest: Hash, registry_id: Option<Ss58Identifier>) -> Option<Ss58Identifier>;
	}
}
