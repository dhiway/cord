// CORD Blockchain – https://dhiway.network
// Copyright (C) 2019-2021 Dhiway
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

use codec::{Decode, Encode};
use cord_primitives::StatusOf;
use frame_support::{
	dispatch::DispatchInfo,
	traits::{Currency, ExistenceRequirement, Len, OnUnbalanced, WithdrawReasons},
	weights::Weight,
};
pub use pallet::*;

pub use pallet::*;
use sp_runtime::{
	traits::{DispatchInfoOf, Dispatchable, One, SignedExtension, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionLongevity, TransactionValidity, TransactionValidityError,
		ValidTransaction,
	},
	Saturating,
};
use sp_std::{marker::PhantomData, prelude::*};

pub use weights::WeightInfo;

pub mod types;
pub use crate::types::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	pub(crate) type CordProposerAccountOf<T> = <T as frame_system::Config>::AccountId;
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	pub type AnchorBlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	pub(crate) type ProposalInfoOf<T> =
		ProposalInfo<CordAccountOf<T>, CordProposerAccountOf<T>, BlockNumberOf<T>, BalanceOf<T>>;

	pub type ProposalEntryOf<T> = ProposalEntry<
		CordAccountOf<T>,
		CordProposerAccountOf<T>,
		BlockNumberOf<T>,
		BalanceOf<T>,
		AnchorBlockNumberOf<T>,
		StatusOf,
	>;

	pub(crate) type BalanceOf<T> = <<T as Config>::Currency as Currency<CordAccountOf<T>>>::Balance;

	type NegativeImbalanceOf<T> =
		<<T as Config>::Currency as Currency<CordAccountOf<T>>>::NegativeImbalance;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type ApproveOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Currency: Currency<CordAccountOf<Self>>;
		type CreditCollector: OnUnbalanced<NegativeImbalanceOf<Self>>;
		#[pallet::constant]
		type AuthorshipDuration: Get<Self::BlockNumber>;
		#[pallet::constant]
		type DelegationBlockLimit: Get<Self::BlockNumber>;
		#[pallet::constant]
		type MaxBlockProposals: Get<u32>;
		#[pallet::constant]
		type MaxRegistryBlockEntries: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// The pallet's runtime authors.
	#[pallet::storage]
	pub(super) type Authors<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ()>;

	// The pallet's runtime author block.
	#[pallet::storage]
	pub(super) type AuthorBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, T::BlockNumber, OptionQuery>;

	// Intermediates
	#[pallet::storage]
	pub(super) type AuthorProposals<T: Config> =
		StorageValue<_, BoundedVec<ProposalInfoOf<T>, T::MaxBlockProposals>, ValueQuery>;

	/// Collection of authorship metadata by block number.
	#[pallet::storage]
	#[pallet::getter(fn transaction_roots)]
	pub(super) type AuthorshipRegistry<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::BlockNumber,
		BoundedVec<ProposalEntryOf<T>, T::MaxRegistryBlockEntries>,
		ValueQuery,
	>;

	#[pallet::event]
	// #[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// When a new author proposal is added.
		AuthorAdditionProposal { author: CordAccountOf<T> },
		// When an authorship is expired.
		AuthorshipExpired { authors: Vec<CordAccountOf<T>> },
		// When a new author is added.
		AuthorsAdded { authors: Vec<CordAccountOf<T>> },
		// When an author is removed.
		AuthorsRemoved { author: CordAccountOf<T> },
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
		TooManyProposals,
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
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
			let total_weight: Weight = T::WeightInfo::on_initialize(0);

			let period = T::AuthorshipDuration::get();
			let expired = n.saturating_sub(period.saturating_add(One::one()));

			if expired > Zero::zero() {
				let mut expired_authors: Vec<CordAccountOf<T>> = Vec::new();
				let authors = <AuthorshipRegistry<T>>::get(expired);
				if authors.len() > Zero::zero() {
					authors.clone().into_iter().for_each(|r| {
						if !r.retired {
							<Authors<T>>::remove(&r.author);
							<AuthorBlock<T>>::remove(&r.author);
							Self::withdraw_credits(&r.author);
							expired_authors.push(r.author);
						}
					});
					Self::deposit_event(Event::AuthorshipExpired { authors: expired_authors });
				}
				total_weight.saturating_add(
					T::DbWeight::get().reads_writes(2, (authors.len() * 3).try_into().unwrap()),
				);
			}

			let proposals = <AuthorProposals<T>>::take();
			if proposals.len() > Zero::zero() {
				total_weight.saturating_add(
					T::DbWeight::get()
						.reads_writes(1, ((proposals.len() * 2) + 1).try_into().unwrap()),
				);
			}
			total_weight
		}

		fn on_finalize(n: T::BlockNumber) {
			// Insert new authors
			let proposals = <AuthorProposals<T>>::take();

			if proposals.len() > Zero::zero() {
				let mut authors_added: Vec<CordAccountOf<T>> = Vec::new();

				proposals.clone().into_iter().for_each(|p| {
					if p.pblock == Zero::zero() {
						<AuthorBlock<T>>::insert(&p.author, n);
						<Authors<T>>::insert(&p.author, ());
						<AuthorshipRegistry<T>>::mutate(n, |authorisations| {
							authorisations
								.try_push(ProposalEntryOf::<T> {
									author: p.author.clone(),
									credits: p.credits,
									parent: p.parent,
									pblock: n,
									ablock: n,
									retired: false,
								})
								.expect("authorities length is less than T::MaxBlockProposals; qed")
						});
						authors_added.push(p.author);
					} else {
						<AuthorshipRegistry<T>>::mutate(p.pblock, |authorisations| {
							authorisations
								.try_push(ProposalEntryOf::<T> {
									author: p.author.clone(),
									credits: p.credits,
									parent: p.parent,
									pblock: p.pblock,
									ablock: n,
									retired: false,
								})
								.expect("authorities length is less than T::MaxBlockProposals; qed")
						});
						<AuthorBlock<T>>::insert(&p.author, p.pblock);
						<Authors<T>>::insert(&p.author, ());
						authors_added.push(p.author);
					}
				});
				Self::deposit_event(Event::AuthorsAdded { authors: authors_added });
			}
		}
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub phantom: PhantomData<T>,
		pub authors: Vec<(T::AccountId, ())>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { phantom: Default::default(), authors: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let authors = &self.authors;
			if !authors.is_empty() {
				for (account, extrinsics) in authors.iter() {
					<Authors<T>>::insert(account, extrinsics);
				}
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a new author proposal.
		#[pallet::weight(182_886_000)]
		pub fn add(
			origin: OriginFor<T>,
			author: T::AccountId,
			credits: BalanceOf<T>,
		) -> DispatchResult {
			let proposer = ensure_signed(origin)?;

			ensure!(<Authors<T>>::contains_key(&proposer), Error::<T>::ProposerNotAuthorised);
			ensure!(!<Authors<T>>::contains_key(&author), Error::<T>::AuthorAccountAlreadyExists);

			let balance = <T::Currency as Currency<CordAccountOf<T>>>::free_balance(&proposer);
			<T::Currency as Currency<CordAccountOf<T>>>::ensure_can_withdraw(
				&proposer,
				credits,
				WithdrawReasons::TRANSFER,
				balance.saturating_sub(credits),
			)
			.map_err(|_| Error::<T>::UnableToWithdrawCredits)?;

			let period = T::AuthorshipDuration::get();
			let parent_block = <AuthorBlock<T>>::get(&proposer).map_or(Zero::zero(), |b| b);
			let block_number = frame_system::Pallet::<T>::block_number();
			let check_duration =
				parent_block.saturating_add(period.saturating_sub(T::DelegationBlockLimit::get()));
			ensure!(check_duration > block_number, Error::<T>::AuthorshipExpiringSoon);

			<AuthorProposals<T>>::mutate(|proposals| {
				if proposals.len() + 1 > T::MaxBlockProposals::get() as usize {
					return Err(Error::<T>::TooManyProposals)
				}
				proposals
					.try_push(ProposalInfo {
						author: author.clone(),
						credits,
						parent: proposer.clone(),
						pblock: parent_block,
					})
					.map_err(|_| Error::<T>::TooManyProposals)
			})?;
			<T::Currency as Currency<CordAccountOf<T>>>::transfer(
				&proposer,
				&author,
				credits,
				ExistenceRequirement::AllowDeath,
			)
			.map_err(|_| Error::<T>::UnableToTransferCredits)?;

			Self::deposit_event(Event::AuthorAdditionProposal { author });

			Ok(())
		}

		/// Remove an author. Only root or council orgin can perform this
		/// action.
		#[pallet::weight(182_886_000)]
		pub fn remove(origin: OriginFor<T>, remove_author: CordAccountOf<T>) -> DispatchResult {
			T::ApproveOrigin::ensure_origin(origin)?;
			ensure!(<Authors<T>>::contains_key(&remove_author), Error::<T>::AuthorAccountNotFound);

			let author_block = <AuthorBlock<T>>::get(&remove_author)
				.ok_or(Error::<T>::AuthorBlockDetailsNotFound)?;

			let registry_entries = AuthorshipRegistry::<T>::get(&author_block);

			registry_entries.into_iter().for_each(|entry| {
				if entry.author == remove_author.clone() {
					AuthorshipRegistry::<T>::mutate(author_block, |authors| {
						authors.retain(|x| x.author != remove_author);
						authors
							.try_push(ProposalEntryOf::<T> { retired: true, ..entry })
							.expect("authors length is less than T::MaxRegistryBlockEntries; qed")
					});
				}
			});
			<Authors<T>>::remove(&remove_author);
			<AuthorBlock<T>>::remove(&remove_author);

			Self::deposit_event(Event::AuthorsRemoved { author: remove_author });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn withdraw_credits(author: &T::AccountId) -> Weight {
		let total_weight: Weight = T::WeightInfo::on_initialize(0);
		let author_balance = <T::Currency as Currency<CordAccountOf<T>>>::total_balance(author);
		let imbalance = <T::Currency as Currency<CordAccountOf<T>>>::withdraw(
			&author,
			author_balance,
			WithdrawReasons::TRANSFER,
			ExistenceRequirement::AllowDeath,
		)
		.unwrap_or_default();
		T::CreditCollector::on_unbalanced(imbalance);

		total_weight
	}
}

/// The `CheckAuthorRegistry` struct.
#[derive(Encode, Decode, Clone, Eq, PartialEq, Default, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CheckAuthorRegistry<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for CheckAuthorRegistry<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "CheckAuthorRegistry")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> CheckAuthorRegistry<T> {
	/// Create new `SignedExtension` to check author permission.
	pub fn new() -> Self {
		Self(sp_std::marker::PhantomData)
	}
}

/// Implementation of the `SignedExtension` trait for the
/// `CheckAuthorRegistry` struct.
impl<T: Config + Send + Sync> SignedExtension for CheckAuthorRegistry<T>
where
	T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
{
	type AccountId = T::AccountId;
	type Call = T::RuntimeCall;
	type AdditionalSigned = ();
	type Pre = ();
	const IDENTIFIER: &'static str = "CheckAuthorRegistry";

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
		if <Authors<T>>::contains_key(who) {
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
