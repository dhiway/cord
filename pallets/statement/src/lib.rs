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

//! # Statement Pallet
//!
//! The Statement pallet is used to anchor identifiers representing off-chain
//! documents. The pallet provides means of creating, updating, revoking and
//! removing identifier data on-chain and delegated controls.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(test)]
pub mod tests;

use cord_primitives::{curi::Ss58Identifier, StatusOf};
use frame_support::{ensure, storage::types::StorageMap};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::{prelude::Clone, str};
pub mod types;
pub mod weights;
pub use crate::{pallet::*, types::*, weights::WeightInfo};
use frame_system::pallet_prelude::BlockNumberFor;
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use sp_std::{vec, vec::Vec};

#[frame_support::pallet]
pub mod pallet {

	use super::*;
	use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::{OptionQuery, *};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Hash;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Registry Identifier
	pub type RegistryIdOf = Ss58Identifier;
	/// Statement Identifier
	pub type StatementIdOf = Ss58Identifier;
	/// Schema Identifier
	pub type SchemaIdOf = Ss58Identifier;
	/// Authorization Identifier
	pub type AuthorizationIdOf = Ss58Identifier;
	/// Type of a creator identifier.
	pub type StatementCreatorIdOf<T> = pallet_registry::RegistryCreatorIdOf<T>;
	/// Hash of the statement.
	pub type StatementDigestOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for the statement details
	pub type StatementDetailsOf<T> =
		StatementDetails<StatementDigestOf<T>, SchemaIdOf, RegistryIdOf>;
	/// Type for the statement entry details
	pub type StatementEntryStatusOf<T> = StatementEntryStatus<StatementCreatorIdOf<T>, StatusOf>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_registry::Config + identifier::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, StatementCreatorIdOf<Self>>;
		/// Maximum entires supported per batch call
		#[pallet::constant]
		type MaxDigestsPerBatch: Get<u16>;
		/// Maximum removals per call
		#[pallet::constant]
		type MaxRemoveEntries: Get<u16>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// statement identifiers stored on chain.
	/// It maps from an identifier to its details.
	/// Only stores the latest state.
	#[pallet::storage]
	#[pallet::getter(fn statements)]
	pub type Statements<T> =
		StorageMap<_, Blake2_128Concat, StatementIdOf, StatementDetailsOf<T>, OptionQuery>;

	/// statement uniques stored on chain.
	/// It maps from a statement identifier and hash to its details.
	#[pallet::storage]
	#[pallet::getter(fn entries)]
	pub type Entries<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		StatementIdOf,
		Blake2_128Concat,
		StatementDigestOf<T>,
		StatementCreatorIdOf<T>,
		OptionQuery,
	>;

	/// Revocation registry of statement entries stored on chain.
	/// It maps from a statement identifier and hash to its details.
	#[pallet::storage]
	#[pallet::getter(fn revocation_lookup)]
	pub type RevocationRegistry<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		StatementIdOf,
		Blake2_128Concat,
		StatementDigestOf<T>,
		StatementEntryStatusOf<T>,
		OptionQuery,
	>;

	/// Storage for Identifier lookup.
	/// It maps from a statement entry digest and registry id to an identifier.
	#[pallet::storage]
	#[pallet::getter(fn identifier_lookup)]
	pub type IdentifierLookup<T> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		StatementDigestOf<T>,
		Twox64Concat,
		RegistryIdOf,
		StatementIdOf,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new statement identifier has been created.
		/// \[statement identifier, statement digest, controller\]
		Created {
			identifier: StatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorIdOf<T>,
		},
		/// A statement identifier has been updated.
		/// \[statement identifier, digest, controller\]
		Updated {
			identifier: StatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorIdOf<T>,
		},
		/// A statement identifier status has been revoked.
		/// \[statement identifier, controller\]
		Revoked { identifier: StatementIdOf, author: StatementCreatorIdOf<T> },
		/// A statement identifier status has been restored.
		/// \[statement identifier, controller\]
		Restored { identifier: StatementIdOf, author: StatementCreatorIdOf<T> },
		/// A statement identifier has been removed.
		/// \[statement identifier,  controller\]
		Removed { identifier: StatementIdOf, author: StatementCreatorIdOf<T> },
		/// A statement identifier has been removed.
		/// \[statement identifier,  controller\]
		PartialRemoval { identifier: StatementIdOf, removed: u32, author: StatementCreatorIdOf<T> },

		/// A statement digest has been added.
		/// \[statement identifier, digest, controller\]
		Digest {
			identifier: StatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorIdOf<T>,
		},
		/// A statement batch has been processed.
		/// \[successful count, failed count, failed indices,
		/// controller]
		BatchCreate {
			successful: u32,
			failed: u32,
			indices: Vec<u16>,
			author: StatementCreatorIdOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Statement idenfier is not unique
		StatementAlreadyAnchored,
		/// Statement idenfier not found
		StatementNotFound,
		/// Statement entry not found
		StatementEntryNotFound,
		/// Statement entry marked inactive
		StatementRevoked,
		/// Statement idenfier not marked inactive
		StatementNotRevoked,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		/// Statement link does not exist
		StatementLinkNotFound,
		/// Statement Link is revoked
		StatementLinkRevoked,
		/// Invalid creator signature
		InvalidSignature,
		/// Statement hash is not unique
		HashAlreadyAnchored,
		/// Expired Tx Signature
		ExpiredSignature,
		/// Invalid Statement Identifier
		InvalidStatementIdentifier,
		/// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		/// Statement not part of space
		StatementSpaceMismatch,
		/// Statement digest is not unique
		DigestHashAlreadyAnchored,
		/// Invalid transaction hash
		InvalidTransactionHash,
		/// Metadata limit exceeded
		MetadataLimitExceeded,
		/// Metadata already set for the entry
		MetadataAlreadySet,
		/// Metadata not found for the entry
		MetadataNotFound,
		/// Maximum Number of delegates reached.
		TooManyDelegates,
		/// More than the maximum mumber of delegates.
		TooManyDelegatesToRemove,
		/// Authorization not found
		AuthorizationDetailsNotFound,
		/// Maximum number of activities exceeded
		MaxStatementActivitiesExceeded,
		/// Attestation is not found
		AttestationNotFound,
		//Max Digests in a call
		MaxDigestLimitExceeded,
		/// Bulk Transaction Failed
		BulkTransactionFailed,
		/// Associate digest already present
		AssociateDigestAlreadyAnchored,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new statement and associates it with its
		/// controller. The controller (issuer) is the owner of the identifier.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `statement_digests`: Array of the digest of the statements.
		/// * `authorization`: AuthorizationIdOf.
		/// * `schema_id`: The schema id of the statement.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn create(
			origin: OriginFor<T>,
			digest: StatementDigestOf<T>,
			authorization: AuthorizationIdOf,
			schema_id: Option<SchemaIdOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			// Validate registry_id based on schema_id presence.
			let registry_id = match schema_id.clone() {
				Some(id) =>
					pallet_registry::Pallet::<T>::is_a_delegate(&authorization, &creator, Some(id)),
				None => pallet_registry::Pallet::<T>::is_a_delegate(&authorization, &creator, None),
			}
			.map_err(<pallet_registry::Error<T>>::from)?;

			// Id Digest = concat (H(<scale_encoded_statement_digest>,
			// <scale_encoded_registry_identifier>, <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[
					&digest.encode()[..],
					&registry_id.clone().encode()[..],
					&creator.clone().encode()[..],
				]
				.concat()[..],
			);

			let identifier = Ss58Identifier::to_statement_id(&(id_digest).encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Statements<T>>::contains_key(&identifier),
				Error::<T>::StatementAlreadyAnchored
			);

			<Statements<T>>::insert(
				&identifier,
				StatementDetailsOf::<T> {
					digest,
					schema: schema_id.clone(),
					registry: registry_id.clone(),
				},
			);

			<Entries<T>>::insert(&identifier, digest, creator.clone());
			<IdentifierLookup<T>>::insert(digest, &registry_id, &identifier);

			Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Created { identifier, digest, author: creator });

			Ok(())
		}

		/// Updates the statement identifier with a new digest. The updated
		/// digest represents the changes a statement reference document might
		/// have undergone. Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `statement_id`: The identifier of the statement to be updated.
		/// * `statement_digest`: The hash of the statement reference document.
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn update(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			new_statement_digest: StatementDigestOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;

			// Check for revocation first to fail early if applicable.
			ensure!(
				!<RevocationRegistry<T>>::contains_key(&statement_id, &statement_details.digest),
				Error::<T>::StatementRevoked
			);

			// Check for digest uniqueness to fail early if the digest is the same.
			ensure!(
				statement_details.digest != new_statement_digest,
				Error::<T>::StatementAlreadyAnchored
			);

			// Check if the updater is the previous creator or a delegate with the proper
			// authorization.
			let is_updater_creator = Entries::<T>::get(&statement_id, &statement_details.digest)
				.map_or(false, |prev_creator| prev_creator == updater);

			if !is_updater_creator {
				let registry_id =
					pallet_registry::Pallet::<T>::is_a_delegate(&authorization, &updater, None)
						.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(
					statement_details.registry == registry_id,
					Error::<T>::UnauthorizedOperation
				);
			}

			<RevocationRegistry<T>>::insert(
				&statement_id,
				&statement_details.digest,
				StatementEntryStatusOf::<T> { creator: updater.clone(), revoked: true },
			);

			<Entries<T>>::insert(&statement_id, &new_statement_digest, updater.clone());

			<IdentifierLookup<T>>::insert(
				&new_statement_digest,
				&statement_details.registry.clone(),
				&statement_id,
			);

			<Statements<T>>::insert(
				&statement_id,
				StatementDetailsOf::<T> { digest: new_statement_digest, ..statement_details },
			);

			Self::update_activity(&statement_id, CallTypeOf::Update).map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Updated {
				identifier: statement_id,
				digest: new_statement_digest,
				author: updater,
			});

			Ok(())
		}
		/// Revokes a statement.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `statement_id`: The statement identifier.
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn revoke(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;

			// Check for revocation first to fail early if applicable.
			ensure!(
				!<RevocationRegistry<T>>::contains_key(&statement_id, &statement_details.digest),
				Error::<T>::StatementRevoked
			);

			// Check if the updater is the previous creator or a delegate with the proper
			// authorization.
			let is_updater_creator = Entries::<T>::get(&statement_id, &statement_details.digest)
				.map_or(false, |prev_creator| prev_creator == updater);

			if !is_updater_creator {
				let registry_id =
					pallet_registry::Pallet::<T>::is_a_delegate(&authorization, &updater, None)
						.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(
					statement_details.registry == registry_id,
					Error::<T>::UnauthorizedOperation
				);
			}

			<RevocationRegistry<T>>::insert(
				&statement_id,
				&statement_details.digest,
				StatementEntryStatusOf::<T> { creator: updater.clone(), revoked: true },
			);

			Self::update_activity(&statement_id, CallTypeOf::Revoke).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Revoked { identifier: statement_id, author: updater });

			Ok(())
		}

		/// Restore a previously revoked statement.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `statement_id`: The statement identifier.
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(3)]
		#[pallet::weight({0})]
		pub fn restore(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;

			// Check for revocation first to fail early if not revoked.
			ensure!(
				<RevocationRegistry<T>>::contains_key(&statement_id, &statement_details.digest),
				Error::<T>::StatementNotRevoked
			);

			// Check if the updater is the previous creator or a delegate with the proper
			// authorization.
			let is_updater_creator = Entries::<T>::get(&statement_id, &statement_details.digest)
				.map_or(false, |prev_creator| prev_creator == updater);

			if !is_updater_creator {
				let registry_id =
					pallet_registry::Pallet::<T>::is_a_delegate(&authorization, &updater, None)
						.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(
					statement_details.registry == registry_id,
					Error::<T>::UnauthorizedOperation
				);
			}

			<RevocationRegistry<T>>::remove(&statement_id, &statement_details.digest);

			Self::update_activity(&statement_id, CallTypeOf::Restore).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Restored { identifier: statement_id, author: updater });

			Ok(())
		}

		/// Removes a statement from the registry.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `statement_id`: The statement id of the statement to be removed.
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn remove(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;

			// Authorization check is moved up to fail early.
			let registry_id =
				pallet_registry::Pallet::<T>::is_a_delegate(&authorization, &updater, None)
					.map_err(<pallet_registry::Error<T>>::from)?;

			ensure!(statement_details.registry == registry_id, Error::<T>::UnauthorizedOperation);

			// Count the entries in `Entries`.
			let entries_count = <Entries<T>>::iter_prefix(&statement_id).count();
			let max_removals = T::MaxRemoveEntries::get() as usize;

			// Determine if a complete or partial removal is needed.
			let is_complete_removal = entries_count <= max_removals;

			// Start the removal process.
			let mut removed_count = 0;
			if is_complete_removal {
				// Perform a complete removal.
				for (digest, _) in <Entries<T>>::iter_prefix(&statement_id) {
					<IdentifierLookup<T>>::remove(&digest, &registry_id);
				}
				let _ = <RevocationRegistry<T>>::clear_prefix(
					&statement_id,
					entries_count as u32,
					None,
				);
				let _ = <Entries<T>>::clear_prefix(&statement_id, entries_count as u32, None);
				<Statements<T>>::remove(&statement_id);
			} else {
				// Perform a partial removal.
				for (digest, _) in <Entries<T>>::iter_prefix(&statement_id).take(max_removals) {
					<IdentifierLookup<T>>::remove(&digest, &registry_id);
					<RevocationRegistry<T>>::remove(&statement_id, &digest);
					<Entries<T>>::remove(&statement_id, &digest);
					removed_count += 1;
				}
			}

			// Update activity and emit the appropriate event.
			Self::update_activity(
				&statement_id,
				if is_complete_removal { CallTypeOf::Remove } else { CallTypeOf::PartialRemove },
			)
			.map_err(<Error<T>>::from)?;

			let event = if is_complete_removal {
				Event::Removed { identifier: statement_id, author: updater }
			} else {
				Event::PartialRemoval {
					identifier: statement_id,
					removed: removed_count as u32,
					author: updater,
				}
			};

			Self::deposit_event(event);

			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight({0})]
		pub fn batch_create(
			origin: OriginFor<T>,
			digests: Vec<StatementDigestOf<T>>,
			authorization: AuthorizationIdOf,
			schema_id: Option<SchemaIdOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			// Ensure the incoming digests are not too large.
			ensure!(
				digests.len() <= T::MaxDigestsPerBatch::get() as usize,
				Error::<T>::MaxDigestLimitExceeded
			);

			// Validate registry_id based on schema_id presence.
			let registry_id = match schema_id.clone() {
				Some(id) =>
					pallet_registry::Pallet::<T>::is_a_delegate(&authorization, &creator, Some(id)),
				None => pallet_registry::Pallet::<T>::is_a_delegate(&authorization, &creator, None),
			}
			.map_err(<pallet_registry::Error<T>>::from)?;

			let mut success = 0u32;
			let mut fail = 0u32;
			let mut indices: Vec<u16> = Vec::new();

			for (index, digest) in digests.iter().enumerate() {
				let id_digest = <T as frame_system::Config>::Hashing::hash(
					&[
						&digest.encode()[..],
						&registry_id.clone().encode()[..],
						&creator.clone().encode()[..],
					]
					.concat()[..],
				);

				let identifier_result = Ss58Identifier::to_statement_id(&id_digest.encode());

				match identifier_result {
					Ok(identifier) =>
						if <Statements<T>>::contains_key(&identifier) {
							fail += 1;
							indices.push(index as u16);
						} else {
							<Statements<T>>::insert(
								&identifier,
								StatementDetailsOf::<T> {
									digest: digest.clone(),
									schema: schema_id.clone(),
									registry: registry_id.clone(),
								},
							);

							<Entries<T>>::insert(&identifier, digest, creator.clone());
							<IdentifierLookup<T>>::insert(&digest, &registry_id, &identifier);

							if Self::update_activity(&identifier, CallTypeOf::Genesis).is_err() {
								fail += 1;
								indices.push(index as u16);
							} else {
								success += 1;
							}
						},
					Err(_) => {
						fail += 1;
						indices.push(index as u16);
					},
				}
			}

			ensure!(success > 0, Error::<T>::BulkTransactionFailed);

			Self::deposit_event(Event::BatchCreate {
				successful: success,
				failed: fail,
				indices,
				author: creator,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn update_activity(
		statement_id: &StatementIdOf,
		statement_action: CallTypeOf,
	) -> Result<(), Error<T>> {
		let statement_moment = Self::timepoint();

		let statement_entry = EventEntryOf { action: statement_action, location: statement_moment };
		let _ = identifier::Pallet::<T>::update_timeline(
			statement_id,
			IdentifierTypeOf::Statement,
			statement_entry,
		);
		Ok(())
	}

	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
