// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.
// A module in charge of accounting dhi-treasury

#![cfg_attr(not(feature = "std"), no_std)]
pub mod weights;

use sp_runtime::traits::{AccountIdConversion, Saturating, StaticLookup};

use frame_support::{
	traits::{Currency, ExistenceRequirement, Get, OnUnbalanced},
	PalletId,
};

pub use pallet::*;
pub use weights::WeightInfo;

pub type BalanceOf<T, I = ()> =
	<<T as Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type NegativeImbalanceOf<T, I = ()> = <<T as Config<I>>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// Origin from which approvals must come.
		type BuilderOrigin: EnsureOrigin<Self::Origin>;
		/// The pallet id, used for deriving its sovereign account ID.
		#[pallet::constant]
		type PalletId: Get<PalletId>;
		/// The balance.
		type Currency: Currency<Self::AccountId>;
		/// The overarching event type.
		type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;
		/// The weight for this pallet's extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig;

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self
		}
	}

	#[cfg(feature = "std")]
	impl GenesisConfig {
		pub fn assimilate_storage<T: Config<I>, I: 'static>(
			&self,
			storage: &mut sp_runtime::Storage,
		) -> Result<(), String> {
			<Self as GenesisBuild<T, I>>::assimilate_storage(self, storage)
		}
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig {
		fn build(&self) {
			// Create builder account
			let account_id = <Pallet<T, I>>::account_id();
			let min = T::Currency::minimum_balance();
			if T::Currency::free_balance(&account_id) < min {
				let _ = T::Currency::make_free_balance_be(&account_id, min);
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// Some funds have been transfered  \[recipient, amount\].
		Transfer(T::AccountId, BalanceOf<T, I>),
		/// Some funds have been deposited  \[deposit\]
		Deposit(BalanceOf<T, I>),
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Builder balance is too low.
		InsufficientBalance,
	}

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		#[pallet::weight(0)]
		pub fn transfer(
			origin: OriginFor<T>,
			beneficiary: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: BalanceOf<T, I>,
		) -> DispatchResult {
			T::BuilderOrigin::ensure_origin(origin)?;
			let beneficiary = T::Lookup::lookup(beneficiary)?;
			let balance = T::Currency::free_balance(&Self::account_id());
			ensure!(balance >= amount, Error::<T, I>::InsufficientBalance);

			let _ = T::Currency::transfer(
				&Self::account_id(),
				&beneficiary,
				amount,
				ExistenceRequirement::KeepAlive,
			);

			Self::deposit_event(Event::Transfer(beneficiary, amount));

			Ok(())
		}

		/// Deposit WAY  to builder account
		#[pallet::weight(0)]
		pub fn receive(origin: OriginFor<T>, amount: BalanceOf<T, I>) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let _ = T::Currency::transfer(
				&sender,
				&Self::account_id(),
				amount,
				ExistenceRequirement::AllowDeath,
			);

			Self::deposit_event(Event::Transfer(sender, amount));

			Ok(())
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		fn account_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}
		/// Return the amount of money in the treasury.
		pub fn balance() -> BalanceOf<T, I> {
			T::Currency::free_balance(&Self::account_id())
				// Must never be less than 0 but better be safe.
				.saturating_sub(T::Currency::minimum_balance())
		}
	}

	impl<T: Config<I>, I: 'static> OnUnbalanced<NegativeImbalanceOf<T, I>> for Pallet<T, I> {
		fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<T, I>) {
			// Must resolve into existing but better to be safe.
			let _ = T::Currency::resolve_creating(&Self::account_id(), amount);
		}
	}
}
