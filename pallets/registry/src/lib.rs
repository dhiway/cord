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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

#[cfg(any(test, feature = "runtime-benchmarks"))]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::{ensure, storage::types::StorageMap, BoundedVec};

pub mod types;
pub mod weights;

pub use crate::{types::*, weights::WeightInfo};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{curi::Ss58Identifier, StatusOf};
	use cord_utilities::traits::CallSources;
	use frame_support::{pallet_prelude::*, sp_runtime::traits::Hash};
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Registry Identifier
	pub type RegistryIdOf = Ss58Identifier;
	/// Schema Identifier
	pub type SchemaIdOf = Ss58Identifier;
	/// Authorization Identifier
	pub type AuthorizationIdOf = Ss58Identifier;
	/// Hash of the registry.
	pub type RegistryHashOf<T> = <T as frame_system::Config>::Hash;

	/// Type of a CORD account.
	pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of a registry creator.
	pub type RegistryCreatorIdOf<T> = <T as Config>::RegistryCreatorId;
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	pub type InputRegistryOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedRegistryLength>;

	pub type RegistryEntryOf<T> = RegistryEntry<
		InputRegistryOf<T>,
		RegistryHashOf<T>,
		SchemaIdOf,
		RegistryCreatorIdOf<T>,
		StatusOf,
	>;

	pub type RegistryAuthorizationOf<T> =
		RegistryAuthorization<RegistryIdOf, RegistryCreatorIdOf<T>, SchemaIdOf, Permissions>;

	pub type RegistryCommitOf<T> = RegistryCommit<
		RegistryCommitActionOf,
		RegistryHashOf<T>,
		RegistryCreatorIdOf<T>,
		BlockNumberFor<T>,
	>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, RegistryCreatorIdOf<Self>>;
		type RegistryCreatorId: Parameter + MaxEncodedLen;

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

	/// registry authorities stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::getter(fn authorities)]
	pub(super) type Authorities<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		RegistryIdOf,
		BoundedVec<RegistryCreatorIdOf<T>, T::MaxRegistryAuthorities>,
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
		/// A new registry authorization has been added.
		/// \[registry identifier, authorization,  authority\]
		AddAuthorization {
			registry: RegistryIdOf,
			authorization: AuthorizationIdOf,
			delegate: RegistryCreatorIdOf<T>,
		},
		/// A registry authorization has been removed.
		/// \[registry identifier, authorization, ]
		RemoveAuthorization { registry: RegistryIdOf, authorization: AuthorizationIdOf },
		/// A new registry has been created.
		/// \[registry identifier, creator\]
		Create { registry: RegistryIdOf, creator: RegistryCreatorIdOf<T> },
		/// A registry has been updated.
		/// \[registry identifier, authority\]
		Update { registry: RegistryIdOf, authority: RegistryCreatorIdOf<T> },
		/// A registry has been archived.
		/// \[registry identifier,  authority\]
		Archive { registry: RegistryIdOf, authority: RegistryCreatorIdOf<T> },
		/// A registry has been restored.
		/// \[registry identifier,  authority\]
		Restore { registry: RegistryIdOf, authority: RegistryCreatorIdOf<T> },
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// Registry identifier is not unique
		RegistryAlreadyAnchored,
		/// Registry identifier not found
		RegistryNotFound,
		/// Only when the author is not the controller or delegate.
		UnauthorizedOperation,
		/// Invalid Identifier
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
		DelegateAlreadyAdded,
		/// Authorization Id not found
		AuthorizationNotFound,
		/// Registry schema mismatch
		RegistrySchemaMismatch,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Addsadds a delegate to the list of authorities for a registry.
		///
		/// Arguments:
		///
		/// * `origin`: OriginFor<T>
		/// * `registry_id`: The registry to which the delegate is being added.
		/// * `delegate`: The delegate to add to the registry.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_admin_delegate())]
		pub fn add_admin_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: RegistryCreatorIdOf<T>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;

			ensure!(!registry_details.archive, Error::<T>::ArchivedRegistry);

			if registry_details.creator != creator {
				Self::is_an_authority(&registry_id, creator.clone()).map_err(Error::<T>::from)?;
			}

			// Id Digest = concat (H(<scale_encoded_registry_identifier>,
			// <scale_encoded_creator_identifier>, <scale_encoded_delegate_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]]
					.concat()[..],
			);

			let authorization_id = Ss58Identifier::to_authorization_id(&id_digest.encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Authorizations<T>>::contains_key(&authorization_id),
				Error::<T>::DelegateAlreadyAdded
			);

			let mut authorities = <Authorities<T>>::get(registry_id.clone());
			authorities
				.try_push(delegate.clone())
				.map_err(|_| Error::<T>::RegistryAuthoritiesLimitExceeded)?;
			<Authorities<T>>::insert(&registry_id, authorities);

			<Authorizations<T>>::insert(
				&authorization_id,
				RegistryAuthorizationOf::<T> {
					registry_id: registry_id.clone(),
					delegate: delegate.clone(),
					schema: registry_details.schema,
					permissions: Permissions::all(),
				},
			);

			Self::update_commit(
				&registry_id,
				id_digest,
				creator,
				RegistryCommitActionOf::Authorization,
			)
			.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::AddAuthorization {
				registry: registry_id,
				authorization: authorization_id,
				delegate,
			});

			Ok(())
		}

		/// Adds a delegate to a registry.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `registry_id`: The registry to which the delegate is being added.
		/// * `delegate`: The delegate to add to the registry.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_delegate())]
		pub fn add_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			delegate: RegistryCreatorIdOf<T>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;

			ensure!(!registry_details.archive, Error::<T>::ArchivedRegistry);

			if registry_details.creator != creator {
				Self::is_an_authority(&registry_id, creator.clone()).map_err(Error::<T>::from)?;
			}

			// Id Digest = concat (H(<scale_encoded_registry_identifier>,
			// <scale_encoded_creator_identifier>, <scale_encoded_delegate_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]]
					.concat()[..],
			);

			let authorization_id = Ss58Identifier::to_authorization_id(&id_digest.encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Authorizations<T>>::contains_key(&authorization_id),
				Error::<T>::DelegateAlreadyAdded
			);

			<Authorizations<T>>::insert(
				&authorization_id,
				RegistryAuthorizationOf::<T> {
					registry_id: registry_id.clone(),
					delegate: delegate.clone(),
					schema: registry_details.schema,
					permissions: Permissions::default(),
				},
			);

			Self::update_commit(
				&registry_id,
				id_digest,
				creator,
				RegistryCommitActionOf::Authorization,
			)
			.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::AddAuthorization {
				registry: registry_id,
				authorization: authorization_id,
				delegate,
			});

			Ok(())
		}

		/// Removes a delegate from a registry.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `registry_id`: The registry_id of the registry you want to remove
		///   the delegate from.
		/// * `authorization_id`: The transaction authorization id .
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove_delegate())]
		pub fn remove_delegate(
			origin: OriginFor<T>,
			registry_id: RegistryIdOf,
			authorization_id: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;

			if registry_details.creator != creator {
				Self::is_an_authority(&registry_id, creator.clone()).map_err(Error::<T>::from)?;
			}

			let tx_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry_id.encode()[..], &authorization_id.encode()[..], &creator.encode()[..]]
					.concat()[..],
			);
			ensure!(
				Authorizations::<T>::take(&authorization_id).is_some(),
				Error::<T>::AuthorizationNotFound
			);

			Self::update_commit(
				&registry_id,
				tx_digest,
				creator,
				RegistryCommitActionOf::Deauthorization,
			)
			.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::RemoveAuthorization {
				registry: registry_id,
				authorization: authorization_id,
			});

			Ok(())
		}

		/// Create a new registry.
		///
		/// Arguments:
		///
		/// * `origin`: OriginFor<T>
		/// * `tx_registry`: The new registry detail
		/// * `tx_schema`: Optional schema identifier. Schema Identifier is used
		///   to restrict the registry
		/// * content to a specific schema type.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			tx_registry: InputRegistryOf<T>,
			tx_schema: Option<SchemaIdOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			ensure!(tx_registry.len() > 0, Error::<T>::EmptyTransaction);
			ensure!(
				tx_registry.len() <= T::MaxEncodedRegistryLength::get() as usize,
				Error::<T>::MaxEncodedRegistryLimitExceeded
			);

			// Id Digest = concat (H(<scale_encoded_registry_input>,
			// <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&tx_registry.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let identifier = Ss58Identifier::to_registry_id(&id_digest.encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Registries<T>>::contains_key(&identifier),
				Error::<T>::RegistryAlreadyAnchored
			);

			let digest = <T as frame_system::Config>::Hashing::hash(&tx_registry[..]);

			if let Some(ref schema) = tx_schema {
				ensure!(
					<pallet_schema::Schemas<T>>::contains_key(schema),
					Error::<T>::SchemaNotFound
				);
			}

			<Registries<T>>::insert(
				&identifier,
				RegistryEntryOf::<T> {
					details: tx_registry,
					digest,
					schema: tx_schema,
					creator: creator.clone(),
					archive: false,
				},
			);
			Self::update_commit(
				&identifier,
				digest,
				creator.clone(),
				RegistryCommitActionOf::Genesis,
			)
			.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Create { registry: identifier, creator });

			Ok(())
		}
		/// Allows the creator or an admin delegate of a registry to update the
		/// registry's details
		///
		/// Arguments:
		///
		/// * `origin`: OriginFor<T>
		/// * `tx_registry`: The updated registry details
		/// * `registry_id`: The registry ID of the registry to be updated.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(4)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::update())]
		pub fn update(
			origin: OriginFor<T>,
			tx_registry: InputRegistryOf<T>,
			registry_id: RegistryIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			ensure!(tx_registry.len() > 0, Error::<T>::EmptyTransaction);
			ensure!(
				tx_registry.len() <= T::MaxEncodedRegistryLength::get() as usize,
				Error::<T>::MaxEncodedRegistryLimitExceeded
			);

			let registry_details =
				<Registries<T>>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(registry_details.archive, Error::<T>::ArchivedRegistry);

			if registry_details.creator != updater {
				Self::is_an_authority(&registry_id, updater.clone()).map_err(Error::<T>::from)?;
			}

			let digest = <T as frame_system::Config>::Hashing::hash(&tx_registry[..]);

			<Registries<T>>::insert(
				&registry_id,
				RegistryEntryOf::<T> {
					details: tx_registry,
					digest,
					creator: updater.clone(),
					..registry_details
				},
			);
			Self::update_commit(
				&registry_id,
				digest,
				updater.clone(),
				RegistryCommitActionOf::Update,
			)
			.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Update { registry: registry_id, authority: updater });

			Ok(())
		}

		/// Archives a registry
		///
		/// Arguments:
		///
		/// * `origin`: OriginFor<T>
		/// * `registry_id`: The id of the registry to archive.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::archive())]
		pub fn archive(origin: OriginFor<T>, registry_id: RegistryIdOf) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(registry_details.archive, Error::<T>::ArchivedRegistry);

			if registry_details.creator != creator {
				Self::is_an_authority(&registry_id, creator.clone()).map_err(Error::<T>::from)?;
			}

			<Registries<T>>::insert(
				&registry_id,
				RegistryEntryOf::<T> { archive: true, ..registry_details },
			);

			Self::update_commit(
				&registry_id,
				registry_details.digest,
				creator.clone(),
				RegistryCommitActionOf::Archive,
			)
			.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Archive { registry: registry_id, authority: creator });

			Ok(())
		}

		/// Restores an archived registry
		///
		/// Arguments:
		///
		/// * `origin`: OriginFor<T>
		/// * `registry_id`: The id of the registry to be restored.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(6)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::restore())]
		pub fn restore(origin: OriginFor<T>, registry_id: RegistryIdOf) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_details =
				<Registries<T>>::get(&registry_id).ok_or(Error::<T>::RegistryNotFound)?;
			ensure!(!registry_details.archive, Error::<T>::RegistryNotArchived);

			if registry_details.creator != creator {
				Self::is_an_authority(&registry_id, creator.clone()).map_err(Error::<T>::from)?;
			}

			<Registries<T>>::insert(
				&registry_id,
				RegistryEntryOf::<T> { archive: true, ..registry_details },
			);

			Self::update_commit(
				&registry_id,
				registry_details.digest,
				creator.clone(),
				RegistryCommitActionOf::Restore,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Restore { registry: registry_id, authority: creator });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// checks if the DID identifier passed is an authority of the `registry`
	///
	/// Arguments:
	///
	/// * `tx_registry`: The registry that the transaction is being performed
	///   on.
	/// * `authority`: The DID identifier that is trying to perform the
	///   operation.
	///
	/// Returns:
	///
	/// A Result<(), Error<T>>
	pub fn is_an_authority(
		tx_registry: &RegistryIdOf,
		authority: RegistryCreatorIdOf<T>,
	) -> Result<(), Error<T>> {
		let authorities = <Authorities<T>>::get(tx_registry);
		ensure!(
			(authorities.iter().find(|&a| *a == authority) == Some(&authority)),
			Error::<T>::UnauthorizedOperation
		);

		Ok(())
	}

	/// Updates the Identifier commit history.
	///
	/// Arguments:
	///
	/// * `tx_registry`: The registry that the transaction is being committed
	///   to.
	/// * `tx_digest`: The hash of the transaction that was committed.
	/// * `proposer`: The account that is proposing the transaction.
	/// * `commit`: The action that was committed.
	///
	/// Returns:
	///
	/// A Result<(), Error<T>>
	pub fn update_commit(
		tx_registry: &RegistryIdOf,
		tx_digest: RegistryHashOf<T>,
		proposer: RegistryCreatorIdOf<T>,
		commit: RegistryCommitActionOf,
	) -> Result<(), Error<T>> {
		let block_number = frame_system::Pallet::<T>::block_number();
		Commits::<T>::try_mutate(tx_registry, |commits| {
			commits
				.try_push(RegistryCommitOf::<T> {
					commit,
					digest: tx_digest,
					committed_by: proposer,
					created_at: block_number,
				})
				.map_err(|_| Error::<T>::MaxRegistryCommitsExceeded)?;

			Ok(())
		})
	}

	/// Checks if the given `authorization_id` is a valid authorization for the
	/// given `delegate` and `schema` (if provided)
	///
	/// Arguments:
	///
	/// * `authorization_id`: The authorization id that is being checked.
	/// * `delegate`: The delegate account.
	/// * `schema`: The schema that the delegate is using to assert.
	///
	/// Returns:
	///
	/// A tuple of the registry id and the schema id.
	pub fn is_a_delegate(
		authorization_id: &AuthorizationIdOf,
		delegate: RegistryCreatorIdOf<T>,
		schema: Option<SchemaIdOf>,
	) -> Result<RegistryIdOf, Error<T>> {
		let delegate_details = <Authorizations<T>>::get(authorization_id);
		if let Some(d) = delegate_details {
			ensure!(d.delegate == delegate, Error::<T>::UnauthorizedOperation);
			ensure!(
				(d.permissions & Permissions::ASSERT) == Permissions::ASSERT,
				Error::<T>::UnauthorizedOperation
			);
			if let Some(s) = d.schema {
				if let Some(m) = schema {
					ensure!(s == m, Error::<T>::RegistrySchemaMismatch);
				} else {
					ensure!(false, Error::<T>::RegistrySchemaMismatch);
				}
			}
			Ok(d.registry_id)
		} else {
			Err(Error::<T>::AuthorizationNotFound)
		}
	}

	/// Checks if the given `authorization_id` is an admin of the given
	/// `registry_id`
	///
	/// Arguments:
	///
	/// * `authorization_id`: The authorization id of the delegate.
	/// * `delegate`: The delegate account.
	///
	/// Returns:
	///
	/// The registry id of the registry that the delegate is an admin of.
	pub fn is_a_registry_admin(
		authorization_id: &AuthorizationIdOf,
		delegate: RegistryCreatorIdOf<T>,
	) -> Result<RegistryIdOf, Error<T>> {
		let delegate_details = <Authorizations<T>>::get(authorization_id);
		if let Some(d) = delegate_details {
			ensure!(d.delegate == delegate, Error::<T>::UnauthorizedOperation);
			ensure!(
				(d.permissions & Permissions::ADMIN) == Permissions::ADMIN,
				Error::<T>::UnauthorizedOperation
			);
			Ok(d.registry_id)
		} else {
			Err(Error::<T>::AuthorizationNotFound)
		}
	}
}
