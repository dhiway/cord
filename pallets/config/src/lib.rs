// CORD Blockchain – https://dhiway.network
// Copyright (C) Dhiway Networks Pvt. Ltd.
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

//! # Network Membership Manager
#![warn(unused_extern_crates)]
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

// use frame_support::traits::Get;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, traits::StorageVersion};
	use sp_std::marker::PhantomData;
	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	// Stores the network configuration type
	#[pallet::storage]
	pub type NetworkPermission<T> = StorageValue<_, bool, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub permissioned: bool,
		pub _marker: PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			NetworkPermission::<T>::put(&self.permissioned);
		}
	}
}

impl<T: Config> Pallet<T> {
	/// check if the network is permissioned
	pub fn is_permissioned() -> bool {
		NetworkPermission::<T>::get()
	}
}

impl<T: Config> cord_primitives::IsPermissioned for Pallet<T> {
	fn is_permissioned() -> bool {
		Self::is_permissioned()
	}
}
