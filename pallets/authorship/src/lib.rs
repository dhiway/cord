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

//! # Network Authorship Manager

#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;
mod mock;

use codec::{Decode, Encode};
use frame_support::dispatch::DispatchInfo;
pub use pallet::*;

use sp_runtime::{
	traits::{DispatchInfoOf, Dispatchable, SignedExtension},
	transaction_validity::{
		InvalidTransaction, TransactionLongevity, TransactionValidity, TransactionValidityError,
		ValidTransaction,
	},
	SaturatedConversion,
};
use sp_std::{marker::PhantomData, prelude::*};

pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type AuthorApproveOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		#[pallet::constant]
		type MaxAuthorityProposals: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	// The pallet's runtime authors.
	#[pallet::storage]
	pub(super) type ExtrinsicAuthors<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ()>;

	#[pallet::event]
	// #[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// When a new author is added.
		AuthorsAdded { authors_added: Vec<CordAccountOf<T>> },
		// When an author is removed.
		AuthorsRemoved { authors_removed: Vec<CordAccountOf<T>> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// There is no author with the given ID.
		AuthorAccountNotFound,
		/// The author entry already exists.
		AuthorAccountAlreadyExists,
		/// Proposer is not authorised
		ProposerNotAuthorised,
		/// Too many proposals within a block
		TooManyAuthorityProposals,
		/// Unable to transfer credits from proposer
		UnableToTransferCredits,
		/// Unable to ensure withdrawal of credits from proposer
		UnableToWithdrawCredits,
		/// Not able to find author block
		AuthorBlockDetailsNotFound,
		/// Authorship is ending soon
		AuthorshipExpiringSoon,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		// pub phantom: PhantomData<T>,
		pub authors: Vec<(T::AccountId, ())>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { authors: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let authors = &self.authors;
			if !authors.is_empty() {
				for (account, extrinsics) in authors.iter() {
					<ExtrinsicAuthors<T>>::insert(account, extrinsics);
				}
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a new author proposal.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add(authors.len().saturated_into()))]

		pub fn add(origin: OriginFor<T>, authors: Vec<CordAccountOf<T>>) -> DispatchResult {
			T::AuthorApproveOrigin::ensure_origin(origin)?;
			ensure!(
				authors.len() <= T::MaxAuthorityProposals::get() as usize,
				Error::<T>::TooManyAuthorityProposals
			);
			let mut authors_added: Vec<CordAccountOf<T>> = Vec::new();

			for author in authors {
				if !<ExtrinsicAuthors<T>>::contains_key(&author) {
					<ExtrinsicAuthors<T>>::insert(&author, ());
					authors_added.push(author);
				}
			}

			Self::deposit_event(Event::AuthorsAdded { authors_added });

			Ok(())
		}

		/// Remove an author. Only root or council orgin can perform this
		/// action.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove(authors.len().saturated_into()))]
		pub fn remove(origin: OriginFor<T>, authors: Vec<CordAccountOf<T>>) -> DispatchResult {
			T::AuthorApproveOrigin::ensure_origin(origin)?;
			let mut authors_removed: Vec<CordAccountOf<T>> = Vec::new();

			for author in authors {
				if <ExtrinsicAuthors<T>>::contains_key(&author) {
					<ExtrinsicAuthors<T>>::remove(&author);
					authors_removed.push(author);
				}
			}

			Self::deposit_event(Event::AuthorsRemoved { authors_removed });
			Ok(())
		}
	}
}

/// The `CheckAuthorRegistry` struct.
#[derive(Encode, Decode, Clone, Eq, PartialEq, Default, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CheckExtrinsicAuthor<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for CheckExtrinsicAuthor<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "CheckExtrinsicAuthor")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> CheckExtrinsicAuthor<T> {
	/// Create new `SignedExtension` to check author permission.
	pub fn new() -> Self {
		Self(sp_std::marker::PhantomData)
	}
}

/// Implementation of the `SignedExtension` trait for the
/// `CheckAuthorRegistry` struct.
impl<T: Config + Send + Sync> SignedExtension for CheckExtrinsicAuthor<T>
where
	T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
{
	type AccountId = T::AccountId;
	type Call = T::RuntimeCall;
	type AdditionalSigned = ();
	type Pre = ();
	const IDENTIFIER: &'static str = "CheckExtrinsicAuthor";

	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		call: &Self::Call,
		info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		self.validate(who, call, info, len).map(|_| ())
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		_call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		if <ExtrinsicAuthors<T>>::contains_key(who) {
			Ok(ValidTransaction {
				priority: 0,
				longevity: TransactionLongevity::max_value(),
				propagate: true,
				..Default::default()
			})
		} else {
			Err(InvalidTransaction::Call.into())
		}
	}
}
