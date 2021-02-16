// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.
// A module in charge of accounting reserves

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

use frame_support::{
    decl_event, decl_module, decl_storage,
    traits::{Currency, EnsureOrigin, ExistenceRequirement, Get, Imbalance, OnUnbalanced},
    weights::GetDispatchInfo,
    Parameter,
};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::{
    traits::{AccountIdConversion, Dispatchable},
    DispatchResult, ModuleId,
};
use sp_std::prelude::Box;

type BalanceOf<T, I> =
    <<T as Trait<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T, I> = <<T as Trait<I>>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

/// The module's configuration trait.
pub trait Trait<I: Instance = DefaultInstance>: frame_system::Config {
    type Event: From<Event<Self, I>> + Into<<Self as frame_system::Config>::Event>;
    type ExternalOrigin: EnsureOrigin<Self::Origin>;
    type Currency: Currency<Self::AccountId>;
    type Call: Parameter + Dispatchable<Origin = Self::Origin> + GetDispatchInfo;
    type ModuleId: Get<ModuleId>;
}

pub trait WithAccountId<AccountId> {
fn account_id() -> AccountId;
}

decl_storage! {
    trait Store for Module<T: Trait<I>, I: Instance = DefaultInstance> as Reserve {}
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
        /// Some amount was deposited (e.g. for transaction fees).
        Deposit(AccountId, Balance),
        /// Some funds were spent from the reserve.
        SpentFunds(AccountId, Balance),
        /// Someone tipped the company reserve
        TipReceived(AccountId, Balance),
        /// We executed a call coming from the company reserve account
        ReserveOp(DispatchResult),
    }
);

decl_module! {
    /// The module declaration.
    pub struct Module<T: Trait<I>, I: Instance = DefaultInstance> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        /// Spend `amount` funds from the reserve account to `to`.
        #[weight = 100_000_000]
        pub fn spend(origin, to: T::AccountId, amount: BalanceOf<T, I>) -> DispatchResult {
            T::ExternalOrigin::try_origin(origin)
                .map(|_| ())
                .or_else(ensure_root)?;

            let _ = T::Currency::transfer(&Self::account_id(), &to, amount, ExistenceRequirement::KeepAlive);

            Self::deposit_event(RawEvent::SpentFunds(to, amount));

            Ok(())
        }

        /// Deposit `amount` tokens in the treasure account
        #[weight = 50_000_000]
        pub fn tip(origin, amount: BalanceOf<T, I>) -> DispatchResult {
            let tipper = ensure_signed(origin)?;

            let _ = T::Currency::transfer(&tipper, &Self::account_id(), amount, ExistenceRequirement::AllowDeath);

            Self::deposit_event(RawEvent::TipReceived(tipper, amount));

            Ok(())
        }

        /// Dispatch a call as coming from the reserve account
        #[weight = (call.get_dispatch_info().weight + 10_000, call.get_dispatch_info().class)]
        pub fn apply_as(origin, call: Box<<T as Trait<I>>::Call>) {
            T::ExternalOrigin::try_origin(origin)
                .map(|_| ())
                .or_else(ensure_root)?;

            let res = call.dispatch(frame_system::RawOrigin::Root.into());

            Self::deposit_event(RawEvent::ReserveOp(res.map(|_| ()).map_err(|e| e.error)));
        }
    }
}

impl<T: Trait<I>, I: Instance> WithAccountId<T::AccountId> for Module<T, I> {
    fn account_id() -> T::AccountId {
        T::ModuleId::get().into_account()
    }
}

impl<T: Trait<I>, I: Instance> OnUnbalanced<NegativeImbalanceOf<T, I>> for Module<T, I> {
    fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<T, I>) {
    let numeric_amount = amount.peek();
    let reserve_id = Self::account_id();

    // Must resolve into existing but better to be safe.
    let _ = T::Currency::resolve_creating(&Self::account_id(), amount);

    Self::deposit_event(RawEvent::Deposit(reserve_id, numeric_amount));
}
}
 