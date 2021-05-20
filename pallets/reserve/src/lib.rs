// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.
// A module in charge of accounting reserves

#![cfg_attr(not(feature = "std"), no_std)]
pub mod benchmarking;
#[cfg(test)]
mod tests;
pub mod weights;

pub use cord_primitives::{AccountId, Balance};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{Currency, EnsureOrigin, ExistenceRequirement, Get, Imbalance, OnUnbalanced},
	weights::GetDispatchInfo,
	Parameter,
};
use frame_system::ensure_signed;
use sp_runtime::{
	traits::{AccountIdConversion, Dispatchable},
	DispatchResult, ModuleId,
};
use sp_std::prelude::Box;
pub use weights::WeightInfo;

type BalanceOf<T, I> = <<T as Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T, I> =
	<<T as Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

/// The module's configuration trait.
pub trait Config<I: Instance = DefaultInstance>: frame_system::Config + pallet_balances::Config {
	type Event: From<Event<Self, I>> + Into<<Self as frame_system::Config>::Event>;
	type ExternalOrigin: EnsureOrigin<Self::Origin>;
	type Currency: Currency<Self::AccountId>;
	type Call: Parameter + Dispatchable<Origin = Self::Origin> + GetDispatchInfo;
	type ModuleId: Get<ModuleId>;
	/// Weight information for extrinsics in this pallet.
	type WeightInfo: WeightInfo;
}

pub trait WithAccountId<AccountId> {
	fn account_id() -> AccountId;
}

decl_storage! {
	trait Store for Module<T: Config<I>, I: Instance = DefaultInstance> as Reserve {}
	add_extra_genesis {
		build(|_config| {
			let our_account = &<Module<T, I>>::account_id();

			if T::Currency::free_balance(our_account) < T::Currency::minimum_balance() {
				let _ = T::Currency::make_free_balance_be(
					our_account,
					T::Currency::minimum_balance(),
				);
			}
		});
	}
}

decl_event!(
	pub enum Event<T, I: Instance = DefaultInstance>
	where
		AccountId = <T as frame_system::Config>::AccountId,
		Balance = BalanceOf<T, I>,
	{
		/// Some transaction units got deposited to the reserve.
		Deposit(AccountId, Balance),
		/// Some CRD units were transfered from the reserve.
		Transfer(AccountId, Balance),
		/// Some CRD units got transfered to the  reserve
		Received(AccountId, Balance),
		// / We executed a call coming from the company reserve account
		ReserveOp(DispatchResult),
	}
);

decl_error! {
	/// Error for the reserve module.
	pub enum Error for Module<T: Config<I>, I: Instance > {
		/// Reserve balance is too low.
		InsufficientBalance,
	}
}

decl_module! {
	/// The module declaration.
	pub struct Module<T: Config<I>, I: Instance = DefaultInstance> for enum Call where origin: T::Origin {
		type Error = Error<T, I>;

		fn deposit_event() = default;

		/// Transfer CRD units from the reserve account.
		#[weight = <T as Config<I>>::WeightInfo::transfer()]
		pub fn transfer(origin, to: T::AccountId, amount: BalanceOf<T, I>) -> DispatchResult {
				T::ExternalOrigin::ensure_origin(origin)?;

			let balance = T::Currency::free_balance(&Self::account_id());
			ensure!(
				balance >= amount,
				Error::<T, I>::InsufficientBalance
			);

			let _ = T::Currency::transfer(&Self::account_id(), &to, amount, ExistenceRequirement::KeepAlive);

			Self::deposit_event(RawEvent::Transfer(to, amount));

			Ok(())
		}

		/// Deposit CRD units to the reserve account
		#[weight = <T as Config<I>>::WeightInfo::receive()]
		pub fn receive(origin, amount: BalanceOf<T, I>) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let _ = T::Currency::transfer(&sender, &Self::account_id(), amount, ExistenceRequirement::AllowDeath);

			Self::deposit_event(RawEvent::Received(sender, amount));

			Ok(())
		}

		/// Dispatch a call as coming from the reserve account
		#[weight = (call.get_dispatch_info().weight + 10_000, call.get_dispatch_info().class)]
		pub fn apply_as(origin, call: Box<<T as Config<I>>::Call>) {
			T::ExternalOrigin::ensure_origin(origin)?;

			let res = call.dispatch(frame_system::RawOrigin::Root.into());

			Self::deposit_event(RawEvent::ReserveOp(res.map(|_| ()).map_err(|e| e.error)));
		}
	}
}

impl<T: Config<I>, I: Instance> WithAccountId<T::AccountId> for Module<T, I> {
	fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}
}

impl<T: Config<I>, I: Instance> OnUnbalanced<NegativeImbalanceOf<T, I>> for Module<T, I> {
	fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<T, I>) {
		let numeric_amount = amount.peek();
		let reserve_id = Self::account_id();

		// Must resolve into existing but better to be safe.
		let _ = T::Currency::resolve_creating(&Self::account_id(), amount);
		Self::deposit_event(RawEvent::Deposit(reserve_id, numeric_amount));
	}
}
