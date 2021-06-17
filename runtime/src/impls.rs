// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Some configurable implementations as associated type for the CORD runtime.

use frame_support::traits::{Currency, Imbalance, OnUnbalanced};
// use crate::Call;
pub use cord_primitives::{AccountId, Balance};
// use frame_support::{
// 	traits::{Currency, Imbalance, InstanceFilter, OnUnbalanced},
// 	RuntimeDebug,
// };
// use frame_system::{self};

// use codec::{Decode, Encode};

pub type NegativeImbalance<T> =
	<pallet_balances::Pallet<T> as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

/// Logic for the block author to get a portion of fees.
pub struct ToAuthor<R>(sp_std::marker::PhantomData<R>);

impl<R> OnUnbalanced<NegativeImbalance<R>> for ToAuthor<R>
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

pub struct DealWithFees<R>(sp_std::marker::PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for DealWithFees<R>
where
	R: pallet_balances::Config + pallet_treasury::Config + pallet_authorship::Config,
	pallet_treasury::Module<R>: OnUnbalanced<NegativeImbalance<R>>,
	<R as frame_system::Config>::AccountId: From<cord_primitives::AccountId>,
	<R as frame_system::Config>::AccountId: Into<cord_primitives::AccountId>,
	<R as frame_system::Config>::Event: From<pallet_balances::Event<R>>,
{
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance<R>>) {
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

// /// The type used to represent the kinds of proxying allowed.
// #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug)]
// pub enum ProxyType {
// 	Any,
// 	NonTransfer,
// 	Governance,
// 	Staking,
// }

// impl Default for ProxyType {
// 	fn default() -> Self {
// 		Self::Any
// 	}
// }
// impl InstanceFilter<Call> for ProxyType {
// 	fn filter(&self, c: &Call) -> bool {
// 		match self {
// 			ProxyType::Any => true,
// 			ProxyType::NonTransfer => !matches!(
// 				c,
// 				Call::System(..) |
// 				Call::Scheduler(..) |
// 				Call::Babe(..) |
// 				Call::Timestamp(..) |
// 				Call::Indices(pallet_indices::Call::claim(..)) |
// 				Call::Indices(pallet_indices::Call::free(..)) |
// 				Call::Indices(pallet_indices::Call::freeze(..)) |
// 				// Specifically omitting Indices `transfer`, `force_transfer`
// 				// Specifically omitting the entire Balances pallet
// 				Call::Authorship(..) |
// 				Call::Staking(..) |
// 				Call::Offences(..) |
// 				Call::Session(..) |
// 				Call::Grandpa(..) |
// 				Call::ImOnline(..) |
// 				Call::AuthorityDiscovery(..) |
// 				Call::Democracy(..) |
// 				Call::Council(..) |
// 				Call::TechnicalCommittee(..) |
// 				Call::PhragmenElection(..) |
// 				Call::TechnicalMembership(..) |
// 				Call::Treasury(..) |
// 				Call::Bounties(..) |
// 				Call::Tips(..) |
// 				Call::Vesting(pallet_vesting::Call::vest(..)) |
// 				Call::Vesting(pallet_vesting::Call::vest_other(..)) |
// 				// Specifically omitting Vesting `vested_transfer`, and `force_vested_transfer`
// 				Call::Utility(..) |
// 				Call::Proxy(..) |
// 				Call::Multisig(..)
// 			),
// 			ProxyType::Governance => matches!(
// 				c,
// 				Call::Democracy(..)
// 					| Call::Council(..) | Call::TechnicalCommittee(..)
// 					| Call::PhragmenElection(..)
// 					| Call::Treasury(..) | Call::Bounties(..)
// 					| Call::Tips(..) | Call::Utility(..)
// 			),
// 			ProxyType::Staking => matches!(c, Call::Staking(..) | Call::Session(..) | Call::Utility(..)),
// 			ProxyType::IdentityJudgement => matches!(c, Call::Utility(..)),
// 			ProxyType::CancelProxy => matches!(c, Call::Proxy(pallet_proxy::Call::reject_announcement(..))),
// 		}
// 	}
// 	fn is_superset(&self, o: &Self) -> bool {
// 		match (self, o) {
// 			(x, y) if x == y => true,
// 			(ProxyType::Any, _) => true,
// 			(_, ProxyType::Any) => false,
// 			(ProxyType::NonTransfer, _) => true,
// 			_ => false,
// 		}
// 	}
// }
