// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2021 BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

use codec::{Decode, Encode};
use cord_primitives::VersionMigratorTrait;
use sp_std::marker::PhantomData;

use crate::*;

mod v1;

/// Storage version of the DID pallet.
#[derive(Copy, Clone, Encode, Eq, Decode, Ord, PartialEq, PartialOrd)]
pub enum DidStorageVersion {
	V1,
	V2,
}

#[cfg(feature = "try-runtime")]
impl DidStorageVersion {
	/// The latest storage version.
	fn latest() -> Self {
		Self::V2
	}
}

// All nodes will default to this, which is not bad, as in case the "real"
// version is a later one (i.e. the node has been started with already the
// latest version), the migration will simply do nothing as there's nothing in
// the old storage entries to migrate from.
//
// It might get updated in the future when we know that no node is running this
// old version anymore.
impl Default for DidStorageVersion {
	fn default() -> Self {
		Self::V1
	}
}

impl<T: Config> VersionMigratorTrait<T> for DidStorageVersion {
	// It runs the right pre_migrate logic depending on the current storage version.
	#[cfg(feature = "try-runtime")]
	fn pre_migrate(&self) -> Result<(), &str> {
		match *self {
			Self::V1 => v1::pre_migrate::<T>(),
			Self::V2 => Err("Already latest v2 version."),
		}
	}

	// It runs the right migration logic depending on the current storage version.
	fn migrate(&self) -> Weight {
		match *self {
			Self::V1 => v1::migrate::<T>(),
			Self::V2 => 0u64,
		}
	}

	// It runs the right post_migrate logic depending on the current storage
	// version.
	#[cfg(feature = "try-runtime")]
	fn post_migrate(&self) -> Result<(), &str> {
		match *self {
			Self::V1 => v1::post_migrate::<T>(),
			Self::V2 => Err("Migration from v2 should have never happened in the first place."),
		}
	}
}

/// The DID pallet's storage migrator, which handles all version
/// migrations in a sequential fashion.
///
/// If a node has missed on more than one upgrade, the migrator will apply the
/// needed migrations one after the other. Otherwise, if no migration is needed,
/// the migrator will simply not do anything.
pub struct DidStorageMigrator<T>(PhantomData<T>);

impl<T: Config> DidStorageMigrator<T> {
	// Contains the migration sequence logic.
	fn get_next_storage_version(current: DidStorageVersion) -> Option<DidStorageVersion> {
		// If the version current deployed is at least v1, there is no more migrations
		// to run (other than the one from v1).
		match current {
			DidStorageVersion::V1 => None,
			DidStorageVersion::V2 => None,
		}
	}

	/// Checks whether the latest storage version deployed is lower than the
	/// latest possible.
	#[cfg(feature = "try-runtime")]
	pub(crate) fn pre_migrate() -> Result<(), &'static str> {
		ensure!(
			StorageVersion::<T>::get() < DidStorageVersion::latest(),
			"Already the latest storage version."
		);

		// Don't need to check for any other pre_migrate, as in try-runtime it is also
		// called in the migrate() function. Same applies for post_migrate checks for
		// each version migrator.

		Ok(())
	}

	/// Applies all the needed migrations from the currently deployed version to
	/// the latest possible, one after the other.
	///
	/// It returns the total weight consumed by ALL the migrations applied.
	pub(crate) fn migrate() -> Weight {
		let mut current_version: Option<DidStorageVersion> = Some(StorageVersion::<T>::get());
		// Weight for StorageVersion::get().
		let mut total_weight = T::DbWeight::get().reads(1);

		while let Some(ver) = current_version {
			// If any of the needed migrations pre-checks fail, the whole chain panics
			// (during tests).
			#[cfg(feature = "try-runtime")]
			if let Err(err) = <DidStorageVersion as VersionMigratorTrait<T>>::pre_migrate(&ver) {
				panic!("{:?}", err);
			}
			let consumed_weight = <DidStorageVersion as VersionMigratorTrait<T>>::migrate(&ver);
			total_weight = total_weight.saturating_add(consumed_weight);
			// If any of the needed migrations post-checks fail, the whole chain panics
			// (during tests).
			#[cfg(feature = "try-runtime")]
			if let Err(err) = <DidStorageVersion as VersionMigratorTrait<T>>::post_migrate(&ver) {
				panic!("{:?}", err);
			}
			// If more migrations should be applied, current_version will not be None.
			current_version = Self::get_next_storage_version(ver);
		}

		total_weight
	}

	/// Checks whether the storage version after all the needed migrations match
	/// the latest one.
	#[cfg(feature = "try-runtime")]
	pub(crate) fn post_migrate() -> Result<(), &'static str> {
		ensure!(
			StorageVersion::<T>::get() == DidStorageVersion::latest(),
			"Not updated to the latest version."
		);

		Ok(())
	}
}

// The storage migrator is the same as in the delegation pallet, so those test
// cases will suffice. We might want to refactor this into something generic
// over a type implementing the `VersionMigratorTrait` trait, and have it tested
// only once.
