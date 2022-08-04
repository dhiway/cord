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

/// An on-chain stream details mapper to an Identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
// #[codec(mel_bound())]
pub struct EntryType<T: Config> {
	/// Hash.
	pub digest: HashOf<T>,
	/// Controller.
	pub controller: CordAccountOf<T>,
	/// Space ID.
	pub space: IdentifierOf,
}

impl<T: Config> sp_std::fmt::Debug for EntryType<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

/// An on-chain stream details mapper to an Identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
// #[codec(mel_bound())]
pub struct EntryDetails<T: Config> {
	/// Stream hash.
	pub stream: EntryType<T>,
	/// The flag indicating the status of the stream.
	pub revoked: StatusOf,
}

impl<T: Config> sp_std::fmt::Debug for EntryDetails<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

/// An on-chain schema details mapped to an identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
// #[codec(mel_bound())]
pub struct EntryParams<T: Config> {
	/// Schema identifier
	pub identifier: IdentifierOf,
	/// Schema hash.
	pub stream: EntryType<T>,
}

impl<T: Config> sp_std::fmt::Debug for EntryParams<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}
