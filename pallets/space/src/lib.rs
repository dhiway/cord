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
	pub use cord_primitives::{ss58identifier, IdentifierOf, StatusOf, SPACE_INDEX};
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, ExistenceRequirement, OnUnbalanced, WithdrawReasons},
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::{
		traits::{IdentifyAccount, Saturating, Verify},
		SaturatedConversion,
	};
	use sp_std::{boxed::Box, vec::Vec};

	/// Hash of the space.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a cord signature.
	pub type SignatureOf<T> = <T as Config>::Signature;
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	pub type InputSpaceMetaOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedMetaLength>;

	pub type SpaceEntryOf<T> =
		SpaceEntry<HashOf<T>, CordAccountOf<T>, InputSpaceMetaOf<T>, StatusOf>;

	pub type SpaceRegistryEntryOf<T> =
		SpaceRegistryEntry<HashOf<T>, CordAccountOf<T>, SpaceCommitOf, BlockNumberFor<T>>;

	pub(crate) type BalanceOf<T> = <<T as Config>::Currency as Currency<CordAccountOf<T>>>::Balance;

	type NegativeImbalanceOf<T> =
		<<T as Config>::Currency as Currency<CordAccountOf<T>>>::NegativeImbalance;

	pub type SpaceInputOf<T> =
		SpaceInput<HashOf<T>, CordAccountOf<T>, SignatureOf<T>, InputSpaceMetaOf<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = CordAccountOf<Self>,
		>;
		type Currency: Currency<CordAccountOf<Self>>;
		type SpaceFee: Get<BalanceOf<Self>>;
		type BaseFee: Get<BalanceOf<Self>>;
		type FeeCollector: OnUnbalanced<NegativeImbalanceOf<Self>>;
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer>
			+ Parameter
			+ MaxEncodedLen
			+ TypeInfo;
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		#[pallet::constant]
		type MaxSpaceAuthorities: Get<u32>;
		#[pallet::constant]
		type MaxRegistryEntries: Get<u32>;
		#[pallet::constant]
		type MaxEncodedMetaLength: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// space information stored on chain.
	/// It maps from an identifier to its details.
	#[pallet::storage]
	#[pallet::getter(fn spaces)]
	pub type Spaces<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, SpaceEntryOf<T>, OptionQuery>;

	/// Space registry mapped to identifier.
	#[pallet::storage]
	#[pallet::getter(fn transaction_roots)]
	pub(super) type SpaceRegistry<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		BoundedVec<SpaceRegistryEntryOf<T>, T::MaxRegistryEntries>,
		ValueQuery,
	>;

	/// space authorities stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::getter(fn space_delegates)]
	pub(super) type SpaceAuthorities<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		BoundedVec<CordAccountOf<T>, T::MaxSpaceAuthorities>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Space delegates has been added.
		/// \[space identifier,  controller\]
		AddAuthorities { space: IdentifierOf, controller: CordAccountOf<T> },
		/// Space delegates has been removed.
		/// \[space identifier,  controller\]
		RemoveAuthorities { space: IdentifierOf, controller: CordAccountOf<T> },
		/// A new space has been created.
		/// \[space hash, space identifier, controller\]
		Create { space: IdentifierOf, digest: HashOf<T>, controller: CordAccountOf<T> },
		/// A space has been archived.
		/// \[space identifier\]
		Archive { space: IdentifierOf, controller: CordAccountOf<T> },
		/// A space has been restored.
		/// \[space identifier\]
		Restore { space: IdentifierOf, controller: CordAccountOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Space identifier is not unique
		SpaceAlreadyAnchored,
		/// Space identifier not found
		SpaceNotFound,
		/// Only when the author is not the controller.
		UnauthorizedOperation,
		// Maximum Number of delegates reached.
		TooManyAuthorities,
		// Invalid Identifier
		InvalidSpaceIdentifier,
		// Invalid Identifier Length
		InvalidIdentifierLength,
		// Invalid Identifier Prefix
		InvalidIdentifierPrefix,
		// Invalid creator signature
		InvalidControllerSignature,
		// Archived space
		ArchivedSpace,
		// Space not Archived
		SpaceNotArchived,
		// Invalid transaction hash
		InvalidTransactionHash,
		/// The paying account was unable to pay the fees for creating a space.
		UnableToPayFees,
		/// Registry entries exceeded for an identifier
		TooManyRegistryEntries,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller
		/// or delegates.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::delegate())]
		pub fn delegate(
			origin: OriginFor<T>,
			space: IdentifierOf,
			authorities: Vec<CordAccountOf<T>>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			// let content_hash = sp_io::hashing::blake2_256(
			// 	&[&authorities.encode()[..], &controller.encode()[..]].concat()[..],
			// );

			Self::from_space_identities(&space, controller.clone()).map_err(Error::<T>::from)?;

			SpaceAuthorities::<T>::try_mutate(space.clone(), |ref mut a| {
				ensure!(
					a.len() + authorities.len() <= T::MaxSpaceAuthorities::get() as usize,
					Error::<T>::TooManyAuthorities
				);
				for authority in authorities {
					a.try_push(authority)
						.expect("authorities length is less than T::MaxSpaceAuthorities; qed");
				}

				Self::deposit_event(Event::AddAuthorities { space, controller });

				Ok(())
			})
		}
		/// Remove space authorisations (delegation).
		///
		/// This transaction can only be performed by the space controller
		/// or delegated authorities.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::undelegate())]
		pub fn undelegate(
			origin: OriginFor<T>,
			space: IdentifierOf,
			authorities: Vec<CordAccountOf<T>>,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			Self::from_space_identities(&space, controller.clone()).map_err(Error::<T>::from)?;

			SpaceAuthorities::<T>::try_mutate(space.clone(), |ref mut a| {
				for authority in authorities {
					a.retain(|x| x != &authority);
				}

				Self::deposit_event(Event::RemoveAuthorities { space, controller });

				Ok(())
			})
		}

		/// Create a new space and associates with its identifier.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create(tx_space
				.meta
				.as_ref()
				.map(|ac| ac.len().saturated_into::<u32>())
				.unwrap_or(0)))]
		pub fn create(origin: OriginFor<T>, tx_space: Box<SpaceInputOf<T>>) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let SpaceInput { digest, controller, signature, meta } = *tx_space.clone();

			ensure!(
				signature.verify(&(&digest).encode()[..], &controller),
				Error::<T>::InvalidControllerSignature
			);

			// Check the free balance
			let balance = <T::Currency as Currency<CordAccountOf<T>>>::free_balance(&author);
			<T::Currency as Currency<CordAccountOf<T>>>::ensure_can_withdraw(
				&author,
				T::SpaceFee::get(),
				WithdrawReasons::FEE,
				balance.saturating_sub(T::SpaceFee::get()),
			)?;

			// let digest = <T as frame_system::Config>::Hashing::hash(&tx_space[..]);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				ss58identifier::generate(&(digest).encode()[..], SPACE_INDEX).into_bytes(),
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Spaces<T>>::contains_key(&identifier), Error::<T>::SpaceAlreadyAnchored);

			let imbalance = <T::Currency as Currency<CordAccountOf<T>>>::withdraw(
				&author,
				T::SpaceFee::get(),
				WithdrawReasons::FEE,
				ExistenceRequirement::AllowDeath,
			)
			.map_err(|_| Error::<T>::UnableToPayFees)?;
			T::FeeCollector::on_unbalanced(imbalance);
			let block_number = frame_system::Pallet::<T>::block_number();

			<Spaces<T>>::insert(
				&identifier,
				SpaceEntryOf::<T> { digest, controller: controller.clone(), meta, active: true },
			);

			SpaceRegistry::<T>::mutate(identifier.clone(), |a| {
				if a.len() + 1 > T::MaxRegistryEntries::get() as usize {
					return Err(Error::<T>::TooManyRegistryEntries)
				}
				a.try_push(SpaceRegistryEntry {
					digest,
					controller: controller.clone(),
					commit_type: SpaceCommitOf::Genesis,
					block_number,
				})
				.map_err(|_| Error::<T>::TooManyRegistryEntries)
			})?;

			Self::deposit_event(Event::Create { space: identifier, digest, controller });

			Ok(())
		}
		/// Archive a space
		///
		///This transaction can only be performed by the space controller
		/// or delegated authorities
		#[pallet::weight(<T as pallet::Config>::WeightInfo::archive())]
		pub fn archive(
			origin: OriginFor<T>,
			space: IdentifierOf,
			tx_space: Box<SpaceInputOf<T>>,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let SpaceInput { digest, controller, signature, meta: _ } = *tx_space.clone();

			ensure!(
				signature.verify(&(&digest).encode()[..], &controller),
				Error::<T>::InvalidControllerSignature
			);

			ss58identifier::from_known_format(&space, SPACE_INDEX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let space_details = <Spaces<T>>::get(&space).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(space_details.active, Error::<T>::ArchivedSpace);

			Self::from_space_delegates(&space, controller.clone()).map_err(<Error<T>>::from)?;

			let imbalance = <T::Currency as Currency<CordAccountOf<T>>>::withdraw(
				&author,
				T::BaseFee::get(),
				WithdrawReasons::FEE,
				ExistenceRequirement::AllowDeath,
			)
			.map_err(|_| Error::<T>::UnableToPayFees)?;
			T::FeeCollector::on_unbalanced(imbalance);
			let block_number = frame_system::Pallet::<T>::block_number();

			<Spaces<T>>::insert(
				&space,
				SpaceEntryOf::<T> {
					digest,
					controller: controller.clone(),
					active: false,
					..space_details
				},
			);

			SpaceRegistry::<T>::mutate(space.clone(), |a| {
				if a.len() + 1 > T::MaxRegistryEntries::get() as usize {
					return Err(Error::<T>::TooManyRegistryEntries)
				}
				a.try_push(SpaceRegistryEntry {
					digest,
					controller: controller.clone(),
					commit_type: SpaceCommitOf::Archive,
					block_number,
				})
				.map_err(|_| Error::<T>::TooManyRegistryEntries)
			})?;

			Self::deposit_event(Event::Archive { space, controller });

			Ok(())
		}
		/// Restore an archived space
		///
		/// This transaction can only be performed by the space controller
		/// or delegated authorities
		#[pallet::weight(<T as pallet::Config>::WeightInfo::restore())]
		pub fn restore(
			origin: OriginFor<T>,
			space: IdentifierOf,
			tx_space: Box<SpaceInputOf<T>>,
		) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			let SpaceInput { digest, controller, signature, meta: _ } = *tx_space.clone();

			ensure!(
				signature.verify(&(&digest).encode()[..], &controller),
				Error::<T>::InvalidControllerSignature
			);

			ss58identifier::from_known_format(&space, SPACE_INDEX)
				.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

			let space_details = <Spaces<T>>::get(&space).ok_or(Error::<T>::SpaceNotFound)?;
			ensure!(!space_details.active, Error::<T>::SpaceNotArchived);

			Self::from_space_delegates(&space, controller.clone()).map_err(<Error<T>>::from)?;

			let imbalance = <T::Currency as Currency<CordAccountOf<T>>>::withdraw(
				&author,
				T::BaseFee::get(),
				WithdrawReasons::FEE,
				ExistenceRequirement::AllowDeath,
			)
			.map_err(|_| Error::<T>::UnableToPayFees)?;
			T::FeeCollector::on_unbalanced(imbalance);
			let block_number = frame_system::Pallet::<T>::block_number();

			<Spaces<T>>::insert(
				&space,
				SpaceEntryOf::<T> {
					digest,
					controller: controller.clone(),
					active: true,
					..space_details
				},
			);
			SpaceRegistry::<T>::mutate(space.clone(), |a| {
				if a.len() + 1 > T::MaxRegistryEntries::get() as usize {
					return Err(Error::<T>::TooManyRegistryEntries)
				}
				a.try_push(SpaceRegistryEntry {
					digest,
					controller: controller.clone(),
					commit_type: SpaceCommitOf::Archive,
					block_number,
				})
				.map_err(|_| Error::<T>::TooManyRegistryEntries)
			})?;
			Self::deposit_event(Event::Archive { space, controller });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn from_space_identities(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		ss58identifier::from_known_format(tx_ident, SPACE_INDEX)
			.map_err(|_| Error::<T>::InvalidSpaceIdentifier)?;

		let space_details = <Spaces<T>>::get(&tx_ident).ok_or(Error::<T>::SpaceNotFound)?;
		ensure!(space_details.active, Error::<T>::ArchivedSpace);
		if space_details.controller != requestor {
			Self::from_space_delegates(tx_ident, requestor).map_err(Error::<T>::from)?;
		}
		Ok(())
	}
	pub fn from_space_delegates(
		tx_ident: &IdentifierOf,
		requestor: CordAccountOf<T>,
	) -> Result<(), Error<T>> {
		let authorities = <SpaceAuthorities<T>>::get(tx_ident);
		ensure!(
			(authorities.iter().find(|&authority| *authority == requestor) == Some(&requestor)),
			Error::<T>::UnauthorizedOperation
		);

		Ok(())
	}
}
