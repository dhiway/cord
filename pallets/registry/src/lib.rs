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
	/// Authorization Identifier
	pub type AuthorizationIdOf = IdentifierOf;
	/// Hash of the registry.
	pub type RegistryHashOf<T> = <T as frame_system::Config>::Hash;

	/// Type of a CORD account.
	pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of a registry creator.
	pub type ProposerIdOf<T> = <T as Config>::ProposerId;
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	pub type InputRegistryOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedRegistryLength>;

	pub type RegistryEntryOf<T> =
		RegistryEntry<InputRegistryOf<T>, RegistryHashOf<T>, SchemaIdOf, ProposerIdOf<T>, StatusOf>;

	pub type RegistryAuthorizationOf<T> =
		RegistryAuthorization<RegistryIdOf, ProposerIdOf<T>, SchemaIdOf, Permissions>;

	pub type RegistryCommitOf<T> =
		RegistryCommit<RegistryCommitActionOf, ProposerIdOf<T>, BlockNumberFor<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, ProposerIdOf<Self>>;
		type ProposerId: Parameter + MaxEncodedLen;

		#[pallet::constant]
		type MaxEncodedRegistryLength: Get<u32>;

		#[pallet::constant]
		type MaxRegistryAuthorities: Get<u32>;

		#[pallet::constant]
		type MaxRegistryCommitActions: Get<u32>;

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
	pub type Authorizations<T> =
		StorageMap<_, Blake2_128Concat, AuthorizationIdOf, RegistryAuthorizationOf<T>, OptionQuery>;

	/// space delegations stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::getter(fn authorities)]
	pub(super) type Authorities<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		RegistryIdOf,
		BoundedVec<ProposerIdOf<T>, T::MaxRegistryAuthorities>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub type Commits<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		RegistryIdOf,
		BoundedVec<RegistryCommitOf<T>, T::MaxRegistryCommitActions>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new registry delegate has been added.
		/// \[registry identifier,  authority\]
		AddAuthority { registry: RegistryIdOf, authority: ProposerIdOf<T> },
		/// A new registry delegate has been added.
		/// \[registry identifier,  authority\]
		AddAuthorization { registry: RegistryIdOf, delegate: ProposerIdOf<T> },
		/// A registry delegate has been removed.
		/// \[registry identifier,  authority\]
		RemoveAuthority { registry: RegistryIdOf, delegate: ProposerIdOf<T> },
		/// A registry delegate has been removed.
		/// \[registry identifier,  authority\]
		RemoveAuthorization { registry: RegistryIdOf, authorization_id: AuthorizationIdOf },
		/// A new space has been created.
		/// \[registry identifier, creator\]
		Create { registry: RegistryIdOf, creator: ProposerIdOf<T> },
		/// A registry has been archived.
		/// \[registry identifier,  authority\]
		Archive { registry: RegistryIdOf, authority: ProposerIdOf<T> },
		/// A registry has been restored.
		/// \[registry identifier,  authority\]
		Restore { registry: RegistryIdOf, authority: ProposerIdOf<T> },
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
		/// Registry commit entries exceeded
		MaxRegistryCommitsExceeded,
		/// Empty transaction.
		EmptyTransaction,
		/// Invalid Schema.
		InvalidSchema,
		/// Schema not found
		SchemaNotFound,
		/// Authority already added
		AuthorityAlreadyAdded,
		/// Authorization Id not found
		AuthorizationNotPresent,
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
			authority: ProposerIdOf<T>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_details.archive, Error::<T>::ArchivedRegistry);
			if registry_details.creator != creator {
				Self::is_an_authority(&registry, creator.clone()).map_err(Error::<T>::from)?;
			}

			let authority_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry.encode()[..], &authority.encode()[..]].concat()[..],
			);

			let identifier = IdentifierOf::try_from(
				ss58identifier::generate(&(authority_digest).encode()[..], SPACE_PREFIX)
					.into_bytes(),
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Authorizations<T>>::contains_key(&identifier),
				Error::<T>::AuthorityAlreadyAdded
			);

			let mut authorities = <Authorities<T>>::get(registry.clone());
			authorities
				.try_push(authority.clone())
				.map_err(|_| Error::<T>::RegistryAuthoritiesLimitExceeded)?;
			<Authorities<T>>::insert(&registry, authorities);

			<Authorizations<T>>::insert(
				&identifier,
				RegistryAuthorizationOf::<T> {
					registry: registry.clone(),
					delegate: authority.clone(),
					schema: registry_details.schema.clone(),
					permissions: Permissions::all(),
				},
			);

			Self::update_commit(&registry, creator.clone(), RegistryCommitActionOf::Authority)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::AddAuthority { registry, authority });

			Ok(())
		}

		/// Add space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller
		/// or delegates.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_delegates())]
		pub fn authorize(
			origin: OriginFor<T>,
			registry: RegistryIdOf,
			delegate: ProposerIdOf<T>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_details.archive, Error::<T>::ArchivedRegistry);
			if registry_details.creator != creator {
				Self::is_an_authority(&registry, creator.clone()).map_err(Error::<T>::from)?;
			}

			let delegate_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry.encode()[..], &delegate.encode()[..]].concat()[..],
			);

			let identifier = IdentifierOf::try_from(
				ss58identifier::generate(&(delegate_digest).encode()[..], SPACE_PREFIX)
					.into_bytes(),
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Authorizations<T>>::contains_key(&identifier),
				Error::<T>::AuthorityAlreadyAdded
			);

			<Authorizations<T>>::insert(
				&identifier,
				RegistryAuthorizationOf::<T> {
					registry: registry.clone(),
					delegate: delegate.clone(),
					schema: registry_details.schema.clone(),
					permissions: Permissions::default(),
				},
			);

			Self::update_commit(&registry, creator.clone(), RegistryCommitActionOf::Authorization)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::AddAuthorization { registry, delegate });

			Ok(())
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
			authorization_id: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			if registry_details.creator != creator {
				Self::is_an_authority(&registry, creator.clone()).map_err(Error::<T>::from)?;
			}

			ensure!(
				Authorizations::<T>::take(&authorization_id).is_some(),
				Error::<T>::AuthorizationNotPresent
			);

			Self::update_commit(&registry, creator.clone(), RegistryCommitActionOf::Deauthorize)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::RemoveAuthorization { registry, authorization_id });

			Ok(())
		}

		/// Remove space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller
		/// or delegated authorities.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::deauthorize())]
		pub fn remove_authorities(
			origin: OriginFor<T>,
			registry: IdentifierOf,
			delegate: ProposerIdOf<T>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry).ok_or(Error::<T>::RegistryNotFound)?;
			if registry_details.creator != creator {
				Self::is_an_authority(&registry, creator.clone()).map_err(Error::<T>::from)?;
			}

			Authorities::<T>::try_mutate(registry.clone(), |ref mut authorities| {
				authorities.retain(|x| x != &delegate);

				Self::update_commit(&registry, creator.clone(), RegistryCommitActionOf::Authority)
					.map_err(Error::<T>::from)?;

				Self::deposit_event(Event::RemoveAuthority { registry, delegate });

				Ok(())
			})
		}

		/// Create a new space and associates with its identifier.
		#[pallet::call_index(4)]
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
			Self::update_commit(&identifier, creator.clone(), RegistryCommitActionOf::Genesis)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Create { registry: identifier, creator });

			Ok(())
		}
		/// Archive a space
		///
		///This transaction can only be performed by the space controller
		/// or delegated authorities
		#[pallet::call_index(5)]
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

			Self::update_commit(&registry, creator.clone(), RegistryCommitActionOf::Archive)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Archive { registry, authority: creator });

			Ok(())
		}
		/// Restore an archived space
		///
		/// This transaction can only be performed by the space controller
		/// or delegated authorities
		#[pallet::call_index(6)]
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

			Self::update_commit(&registry, creator.clone(), RegistryCommitActionOf::Restore)
				.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Restore { registry, authority: creator });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn is_an_authority(
		tx_registry: &RegistryIdOf,
		proposer: ProposerIdOf<T>,
	) -> Result<(), Error<T>> {
		let authorities = <Authorities<T>>::get(tx_registry);
		ensure!(
			(authorities.iter().find(|&authority| *authority == proposer) == Some(&proposer)),
			Error::<T>::UnauthorizedOperation
		);

		Ok(())
	}

	pub fn update_commit(
		tx_registry: &RegistryIdOf,
		proposer: ProposerIdOf<T>,
		commit: RegistryCommitActionOf,
	) -> Result<(), Error<T>> {
		Commits::<T>::try_mutate(tx_registry, |commits| {
			commits
				.try_push(RegistryCommitOf::<T> {
					commit,
					committed_by: proposer,
					created_at: Self::timepoint(),
				})
				.map_err(|_| Error::<T>::MaxRegistryCommitsExceeded)?;

			Ok(())
		})
	}
	// pub fn can_assert(
	// 	authorization_id: &AuthorizationIdOf,
	// 	tx_schema: &SchemaIdOf,
	// 	proposer: ProposerIdOf<T>,
	// ) -> Result<(), Error<T>> {
	// 	let authorities = <Authorizations<T>>::get(tx_registry);
	// 	for authority in authorities.iter() {
	// 		if authority.delegate == proposer {
	// 			ensure!(
	// 				(authority.permissions & Permissions::ASSERT) == Permissions::ASSERT,
	// 				Error::<T>::UnauthorizedOperation
	// 			);
	// 			ensure!(authority.schema == *tx_schema, Error::<T>::InvalidSchema);
	// 		}
	// 	}
	// 	Ok(())
	// }

	// pub fn is_a_delegate(
	// 	tx_registry: &RegistryIdOf,
	// 	tx_schema: &SchemaIdOf,
	// 	proposer: ProposerIdOf<T>,
	// ) -> Result<(), Error<T>> {
	// 	let authorities = <Authorizations<T>>::get(tx_registry);
	// 	for authority in authorities.iter() {
	// 		if authority.delegate == proposer {
	// 			ensure!(
	// 				(authority.permissions & Permissions::ASSERT) == Permissions::ASSERT,
	// 				Error::<T>::UnauthorizedOperation
	// 			);
	// 			ensure!(authority.schema == *tx_schema, Error::<T>::InvalidSchema);
	// 		}
	// 	}
	// 	Ok(())
	// }
	/// The current `Timepoint`.
	pub fn timepoint() -> Timepoint<T::BlockNumber> {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
