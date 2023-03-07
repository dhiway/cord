// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 Dhiway Networks Pvt. Ltd.
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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
use sp_std::vec::Vec;

pub mod types;
pub mod weights;

pub use crate::{types::*, weights::WeightInfo};
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{ss58identifier, IdentifierOf, StatusOf, SPACE_PREFIX};
	use cord_utilities::traits::CallSources;
	use frame_support::{pallet_prelude::*, sp_runtime::traits::Hash};
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Registry Identifier
	pub type RegistryIdOf = IdentifierOf;
	/// Schema Identifier
	pub type SchemaIdOf = IdentifierOf;
	/// Hash of the registry.
	pub type RegistryHashOf<T> = <T as frame_system::Config>::Hash;

	/// Type of a CORD account.
	pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of a registry creator.
	pub type ProposerOf<T> = <T as Config>::ProposerId;
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	pub type InputRegistryOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedRegistryLength>;

	pub type RegistryEntryOf<T> =
		RegistryEntry<InputRegistryOf<T>, RegistryHashOf<T>, SchemaIdOf, ProposerOf<T>, StatusOf>;

	pub type RegistryAuthorizationOf<T> =
		RegistryAuthorization<ProposerOf<T>, SchemaIdOf, Permissions>;

	pub type RegistryCommitOf<T> =
		RegistryCommit<RegistryCommitActionOf, ProposerOf<T>, BlockNumberFor<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, ProposerOf<Self>>;
		type ProposerId: Parameter + MaxEncodedLen;

		#[pallet::constant]
		type MaxEncodedRegistryLength: Get<u32>;

		#[pallet::constant]
		type MaxRegistryAuthorities: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// registry information stored on chain.
	/// It maps from an identifier to its details.
	#[pallet::storage]
	#[pallet::getter(fn registries)]
	pub type Registries<T> =
		StorageMap<_, Blake2_128Concat, RegistryIdOf, RegistryEntryOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn authorizations)]
	pub type Authorizations<T> = StorageMap<
		_,
		Blake2_128Concat,
		RegistryIdOf,
		BoundedVec<RegistryAuthorizationOf<T>, <T as Config>::MaxRegistryAuthorities>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn registry_commits)]
	pub type RegistryCommits<T> =
		StorageMap<_, Blake2_128Concat, RegistryIdOf, RegistryCommitOf<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new registry delegate has been added.
		/// \[registry identifier,  authority\]
		AddAuthorities { registry: RegistryIdOf, authority: ProposerOf<T> },
		/// A registry delegate has been removed.
		/// \[registry identifier,  authority\]
		RemoveAuthorities { registry: RegistryIdOf, authority: ProposerOf<T> },
		/// A new space has been created.
		/// \[registry identifier, creator\]
		Create { registry: RegistryIdOf, creator: ProposerOf<T> },
		/// A registry has been archived.
		/// \[registry identifier,  authority\]
		Archive { registry: RegistryIdOf, authority: ProposerOf<T> },
		/// A registry has been restored.
		/// \[registry identifier,  authority\]
		Restore { registry: RegistryIdOf, authority: ProposerOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Registry identifier is not unique
		RegistryAlreadyAnchored,
		/// Space identifier not found
		RegistryNotFound,
		/// Only when the author is not the controller or delegate.
		UnauthorizedOperation,
		// Invalid Identifier
		InvalidIdentifier,
		// Invalid Identifier Length
		InvalidIdentifierLength,
		// Invalid Identifier Prefix
		InvalidIdentifierPrefix,
		// Archived Registry
		ArchivedRegistry,
		// Registry not Archived
		RegistryNotArchived,
		/// Registry entries exceeded for an identifier
		TooManyRegistryEntries,
		/// Schema limit exceeds the permitted size.
		MaxEncodedRegistryLimitExceeded,
		/// Registry entries exceeded for an identifier
		RegistryAuthoritiesLimitExceeded,
		/// Empty transaction.
		EmptyTransaction,
		/// Invalid Schema.
		InvalidSchema,
		/// Schema not found
		SchemaNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller
		/// or delegates.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_authorities())]
		pub fn add_authorities(
			origin: OriginFor<T>,
			registry: RegistryIdOf,
			authorities: Vec<ProposerOf<T>>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_details.archive, Error::<T>::ArchivedRegistry);
			if registry_details.creator != creator {
				Self::is_an_authority(&registry, creator.clone()).map_err(Error::<T>::from)?;
			}

			Authorizations::<T>::try_mutate(registry.clone(), |ref mut a| {
				ensure!(
					a.len() + authorities.len() <= T::MaxRegistryAuthorities::get() as usize,
					Error::<T>::RegistryAuthoritiesLimitExceeded
				);
				for authority in authorities {
					a.try_push(RegistryAuthorizationOf::<T> {
						delegate: authority,
						schema: registry_details.schema.clone(),
						permissions: Permissions::all(),
					})
					.expect("authorities length is less than T::MaxRegistryAuthorities; qed");
				}

				<RegistryCommits<T>>::insert(
					&registry,
					RegistryCommitOf::<T> {
						commit: RegistryCommitActionOf::Authorization,
						delegate: creator.clone(),
						created_at: Self::timepoint(),
					},
				);
				Self::deposit_event(Event::AddAuthorities { registry, authority: creator });

				Ok(())
			})
		}

		/// Add space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller
		/// or delegates.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_delegates())]
		pub fn add_delegates(
			origin: OriginFor<T>,
			registry: RegistryIdOf,
			authorities: Vec<ProposerOf<T>>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_details.archive, Error::<T>::ArchivedRegistry);
			if registry_details.creator != creator {
				Self::is_an_authority(&registry, creator.clone()).map_err(Error::<T>::from)?;
			}

			Authorizations::<T>::try_mutate(registry.clone(), |ref mut a| {
				ensure!(
					a.len() + authorities.len() <= T::MaxRegistryAuthorities::get() as usize,
					Error::<T>::RegistryAuthoritiesLimitExceeded
				);
				for authority in authorities {
					a.try_push(RegistryAuthorizationOf::<T> {
						delegate: authority,
						schema: registry_details.schema.clone(),
						permissions: Permissions::default(),
					})
					.expect("authorities length is less than T::MaxRegistryAuthorities; qed");
				}

				<RegistryCommits<T>>::insert(
					&registry,
					RegistryCommitOf::<T> {
						commit: RegistryCommitActionOf::Authorization,
						delegate: creator.clone(),
						created_at: Self::timepoint(),
					},
				);

				Self::deposit_event(Event::AddAuthorities { registry, authority: creator });

				Ok(())
			})
		}

		/// Remove space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller
		/// or delegated authorities.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::deauthorize())]
		pub fn deauthorize(
			origin: OriginFor<T>,
			registry: IdentifierOf,
			authorities: Vec<ProposerOf<T>>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			if registry_details.creator != creator {
				Self::is_an_authority(&registry, creator.clone()).map_err(Error::<T>::from)?;
			}

			Authorizations::<T>::try_mutate(registry.clone(), |ref mut a| {
				for authority in authorities {
					a.retain(|x| x.delegate != authority);
				}

				<RegistryCommits<T>>::insert(
					&registry,
					RegistryCommitOf::<T> {
						commit: RegistryCommitActionOf::Deauthorize,
						delegate: creator.clone(),
						created_at: Self::timepoint(),
					},
				);

				Self::deposit_event(Event::RemoveAuthorities { registry, authority: creator });

				Ok(())
			})
		}
		/// Create a new space and associates with its identifier.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			tx_registry: InputRegistryOf<T>,
			tx_schema: SchemaIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			ensure!(tx_registry.len() > 0, Error::<T>::EmptyTransaction);
			ensure!(
				tx_registry.len() <= T::MaxEncodedRegistryLength::get() as usize,
				Error::<T>::MaxEncodedRegistryLimitExceeded
			);

			let digest = <T as frame_system::Config>::Hashing::hash(&tx_registry[..]);

			let identifier = IdentifierOf::try_from(
				ss58identifier::generate(&(digest).encode()[..], SPACE_PREFIX).into_bytes(),
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Registries<T>>::contains_key(&identifier),
				Error::<T>::RegistryAlreadyAnchored
			);

			ensure!(
				<pallet_schema::Schemas<T>>::contains_key(&tx_schema),
				Error::<T>::SchemaNotFound
			);

			<Registries<T>>::insert(
				&identifier,
				RegistryEntryOf::<T> {
					registry: tx_registry,
					digest,
					schema: tx_schema,
					creator: creator.clone(),
					archive: false,
				},
			);

			<RegistryCommits<T>>::insert(
				&identifier,
				RegistryCommitOf::<T> {
					commit: RegistryCommitActionOf::Genesis,
					delegate: creator.clone(),
					created_at: Self::timepoint(),
				},
			);

			Self::deposit_event(Event::Create { registry: identifier, creator });

			Ok(())
		}
		/// Archive a space
		///
		///This transaction can only be performed by the space controller
		/// or delegated authorities
		#[pallet::call_index(4)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::archive())]
		pub fn archive(origin: OriginFor<T>, registry: RegistryIdOf) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(registry_details.archive, Error::<T>::ArchivedRegistry);

			if registry_details.creator != creator {
				Self::is_an_authority(&registry, creator.clone()).map_err(Error::<T>::from)?;
			}

			<Registries<T>>::insert(
				&registry,
				RegistryEntryOf::<T> { archive: true, ..registry_details },
			);

			<RegistryCommits<T>>::insert(
				&registry,
				RegistryCommitOf::<T> {
					commit: RegistryCommitActionOf::Archive,
					delegate: creator.clone(),
					created_at: Self::timepoint(),
				},
			);

			Self::deposit_event(Event::Archive { registry, authority: creator });

			Ok(())
		}
		/// Restore an archived space
		///
		/// This transaction can only be performed by the space controller
		/// or delegated authorities
		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::restore())]
		pub fn restore(origin: OriginFor<T>, registry: RegistryIdOf) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_details.archive, Error::<T>::RegistryNotArchived);

			if registry_details.creator != creator {
				Self::is_an_authority(&registry, creator.clone()).map_err(Error::<T>::from)?;
			}

			<Registries<T>>::insert(
				&registry,
				RegistryEntryOf::<T> { archive: true, ..registry_details },
			);
			<RegistryCommits<T>>::insert(
				&registry,
				RegistryCommitOf::<T> {
					commit: RegistryCommitActionOf::Restore,
					delegate: creator.clone(),
					created_at: Self::timepoint(),
				},
			);

			Self::deposit_event(Event::Restore { registry, authority: creator });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn is_an_authority(
		tx_registry: &RegistryIdOf,
		proposer: ProposerOf<T>,
	) -> Result<(), Error<T>> {
		let authorities = <Authorizations<T>>::get(tx_registry);
		for authority in authorities.iter() {
			if authority.delegate == proposer {
				ensure!(
					(authority.permissions & Permissions::ADMIN) == Permissions::ADMIN,
					Error::<T>::UnauthorizedOperation
				);
			}
		}
		Ok(())
	}
	pub fn is_a_delegate(
		tx_registry: &RegistryIdOf,
		tx_schema: &SchemaIdOf,
		proposer: ProposerOf<T>,
	) -> Result<(), Error<T>> {
		let authorities = <Authorizations<T>>::get(tx_registry);
		for authority in authorities.iter() {
			if authority.delegate == proposer {
				ensure!(
					(authority.permissions & Permissions::ASSERT) == Permissions::ASSERT,
					Error::<T>::UnauthorizedOperation
				);
				ensure!(authority.schema == *tx_schema, Error::<T>::InvalidSchema);
			}
		}
		Ok(())
	}
	/// The current `Timepoint`.
	pub fn timepoint() -> Timepoint<T::BlockNumber> {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
