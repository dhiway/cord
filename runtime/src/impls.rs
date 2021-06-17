// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Some configurable implementations as associated type for the CORD runtime.

pub use cord_primitives::{AccountId, Balance};
use frame_support::traits::{Currency, Imbalance, OnUnbalanced};

pub type NegativeImbalance<T> =
	<pallet_balances::Pallet<T> as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

/// Logic for the block author to get a portion of fees.
pub struct Author<R>(sp_std::marker::PhantomData<R>);

impl<R> OnUnbalanced<NegativeImbalance<R>> for Author<R>
where
	R: pallet_balances::Config + pallet_authorship::Config,
	<R as frame_system::Config>::AccountId: From<cord_primitives::AccountId>,
	<R as frame_system::Config>::AccountId: Into<cord_primitives::AccountId>,
	<R as frame_system::Config>::Event: From<pallet_balances::Event<R>>,
{
	fn on_nonzero_unbalanced(amount: NegativeImbalance<R>) {
		let numeric_amount = amount.peek();
		let author = <pallet_authorship::Pallet<R>>::author();
		<pallet_balances::Pallet<R>>::resolve_creating(&<pallet_authorship::Pallet<R>>::author(), amount);
		<frame_system::Pallet<R>>::deposit_event(pallet_balances::Event::Deposit(author, numeric_amount));
	}
}
