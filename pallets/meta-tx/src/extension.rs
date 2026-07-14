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

use super::*;
use sp_runtime::impl_tx_ext_default;

/// This type serves as a marker extension to differentiate meta-transactions from regular
/// transactions. It implements the `TransactionExtension` trait and carries constant implicit data
/// ("_meta_tx").
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo, DebugNoBound)]
#[scale_info(skip_type_params(T))]
pub struct MetaTxMarker<T> {
	_phantom: core::marker::PhantomData<T>,
}

impl<T> MetaTxMarker<T> {
	/// Creates new `TransactionExtension` with implicit meta tx marked.
	pub fn new() -> Self {
		Self { _phantom: Default::default() }
	}
}

impl<T: Config + Send + Sync> TransactionExtension<T::RuntimeCall> for MetaTxMarker<T> {
	const IDENTIFIER: &'static str = "MetaTxMarker";
	type Implicit = [u8; 8];
	type Val = ();
	type Pre = ();
	fn implicit(&self) -> Result<Self::Implicit, TransactionValidityError> {
		Ok(*b"_meta_tx")
	}
	fn weight(&self, _: &T::RuntimeCall) -> Weight {
		Weight::zero()
	}
	impl_tx_ext_default!(T::RuntimeCall; validate prepare);
}
