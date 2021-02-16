// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Some configurable implementations as associated type for the CORD runtime.

pub use cord_primitives::{
	AccountId, Balance,
};
use frame_support::{
	traits::{OnUnbalanced, Imbalance, InstanceFilter, Currency},
	RuntimeDebug
	};
use frame_system::{self};
use crate::Call;

use codec::{Encode, Decode};

pub type NegativeImbalance<T> =
    <pallet_balances::Module<T> as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

/// Logic for the block author to get a portion of fees.
pub struct Author<R>(sp_std::marker::PhantomData<R>);

impl<R> OnUnbalanced<NegativeImbalance<R>> for Author<R>
where
	R: pallet_balances::Config + pallet_authorship::Config,
	<R as frame_system::Config>::AccountId: From<AccountId>,
	<R as frame_system::Config>::AccountId: Into<AccountId>,
	<R as frame_system::Config>::Event: From<pallet_balances::Event<R>>,
{
	fn on_nonzero_unbalanced(amount: NegativeImbalance<R>) {
		let numeric_amount = amount.peek();
		let author = <pallet_authorship::Module<R>>::author();
		<pallet_balances::Module<R>>::resolve_creating(&<pallet_authorship::Module<R>>::author(), amount);
		<frame_system::Module<R>>::deposit_event(pallet_balances::Event::Deposit(author, numeric_amount));
	}
}

/// The type used to represent the kinds of proxying allowed.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug)]
pub enum ProxyType {
	Any,
	NonTransfer,
	Governance,
	Staking,
}

impl Default for ProxyType { fn default() -> Self { Self::Any } }
impl InstanceFilter<Call> for ProxyType {
	fn filter(&self, c: &Call) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::NonTransfer => !matches!(c,
				Call::System(..) |
				Call::Scheduler(..) |
				Call::Timestamp(..) |
				Call::Indices(pallet_indices::Call::claim(..)) |
				Call::Indices(pallet_indices::Call::free(..)) |
				Call::Indices(pallet_indices::Call::freeze(..)) |
				// Specifically omitting Indices `transfer`, `force_transfer`
				// Specifically omitting the entire Balances pallet
				Call::Authorship(..) |
				Call::Staking(..) |
				Call::Offences(..) |
				Call::Session(..) |
				Call::Grandpa(..) |
				Call::ImOnline(..) |
				Call::AuthorityDiscovery(..) |
				Call::Council(..) |
				// Specifically omitting Vesting `vested_transfer`, and `force_vested_transfer`
				Call::Utility(..) |
				Call::Proxy(..) |
				Call::Multisig(..)
			),
			ProxyType::Governance => matches!(c,
				Call::Council(..) |
				Call::TechnicalCommittee(..) |
				Call::CordReserve(..) |
				Call::Utility(..)
			),
			ProxyType::Staking => matches!(c,
				Call::Staking(..) |
				Call::Session(..) |
				Call::Utility(..)
			),
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		match (self, o) {
			(x, y) if x == y => true,
			(ProxyType::Any, _) => true,
			(_, ProxyType::Any) => false,
			(ProxyType::NonTransfer, _) => true,
			_ => false,
		}
	}
}