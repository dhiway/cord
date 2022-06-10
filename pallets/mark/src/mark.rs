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

#[derive(Clone, Debug, Encode, Decode, PartialEq, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]

pub struct Mark<T: Config> {
	// hash of the MTYPE used for this mark
	pub mtype_hash: HashOf<T>,
	// the account which executed the mark
	pub marker: CordAccountOf<T>,
	// id of the delegation node (if exist)
	pub delegation_id: Option<DelegationNodeIdOf<T>>,
	// revocation status
	pub revoked: bool,
}
