// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Some configurable implementations as associated type for the CORD runtime.

pub use cord_primitives::{
	AccountId, Balance,
};
use frame_support::traits::{OnUnbalanced, Imbalance, Currency};
pub use pallet_balances::{Call as Balances, NegativeImbalance};

// Logic for the block author to get a portion of fees.
pub struct ToAuthor<R>(sp_std::mark::PhantomData<R>);

impl<R> OnUnbalanced<NegativeImbalance<R>> for ToAuthor<R>
where
	R: pallet_balances::Config + pallet_authorship::Config,
	<R as frame_system::Config>::AccountId: From<AccountId>,
	<R as frame_system::Config>::AccountId: Into<AccountId>,
	<R as frame_system::Config>::Event: From<
		pallet_balances::RawEvent<
			<R as frame_system::Config>::AccountId,
			<R as pallet_balances::Config>::Balance,
			pallet_balances::DefaultInstance,
		>,
	>,
{
	fn on_nonzero_unbalanced(amount: NegativeImbalance<R>) {
		let numeric_amount = amount.peek();
		let author = <pallet_authorship::Module<R>>::author();
		<pallet_balances::Module<R>>::resolve_creating(&author, amount);
		<frame_system::Module<R>>::deposit_event(pallet_balances::RawEvent::Deposit(
			author,
			numeric_amount,
		));
	}
}

pub struct DealWithFees<R>(sp_std::mark::PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for DealWithFees<R>
where
	R: pallet_balances::Config + pallet_treasury::Config + pallet_authorship::Config,
	pallet_treasury::Module<R>: OnUnbalanced<NegativeImbalance<R>>,
	<R as frame_system::Config>::AccountId: From<AccountId>,
	<R as frame_system::Config>::AccountId: Into<AccountId>,
	<R as frame_system::Config>::Event: From<pallet_balances::RawEvent<
		<R as frame_system::Config>::AccountId,
		<R as pallet_balances::Config>::Balance,
		pallet_balances::DefaultInstance>
	>,
{
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item=NegativeImbalance<R>>) {
		if let Some(fees) = fees_then_tips.next() {
			// for fees, 80% to treasury, 20% to author
			let mut split = fees.ration(80, 20);
			if let Some(tips) = fees_then_tips.next() {
				// for tips, if any, 100% to author
				tips.merge_into(&mut split.1);
			}
			use pallet_treasury::Module as Treasury;
			<Treasury<R> as OnUnbalanced<_>>::on_unbalanced(split.0);
			<ToAuthor<R> as OnUnbalanced<_>>::on_unbalanced(split.1);
		}
	}
}
