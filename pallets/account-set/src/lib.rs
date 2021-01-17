/*
 * This file is part of the CORD
 * Copyright (C) 2020 - 21  Dhiway
 * 
 * derived from account set pallet
 */

//! # Account Set Pallet
//!
//! The Account Set Pallet provides functionality to restrict extrinsic submission to a set of
//! allowed accounts. The filtering of accounts is done during the transaction queue validation.

#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::prelude::*;
use sp_runtime::codec::{Decode, Encode};
use sp_std::marker::PhantomData;
use sp_std::fmt::Debug;
use frame_support::{
    decl_event, decl_storage, decl_module,
    dispatch,
    weights::DispatchInfo,
};
use frame_system::ensure_root;
use sp_runtime::{
    transaction_validity::{
		ValidTransaction, TransactionValidityError,
        InvalidTransaction, TransactionValidity,
        TransactionPriority, TransactionLongevity,
	},
    traits::{SignedExtension, DispatchInfoOf, Dispatchable}
};

pub trait Trait: frame_system::Config {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as AccountSet {
        // The allow-list is a _set_ of accounts.
        // We map to (), and use the map for faster lookups.
        AllowedAccounts get(fn allowed_accounts) config(): map hasher(blake2_128_concat) T::AccountId => ();
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        fn deposit_event() = default;

        /// Add a new account to the allow-list.
        /// Can only be called by the root.
        #[weight = 0]
        pub fn add_account(origin, new_account: T::AccountId) -> dispatch::DispatchResult {
            ensure_root(origin)?;

            <AllowedAccounts<T>>::insert(&new_account, ());

            Self::deposit_event(RawEvent::AccountAllowed(new_account));

            Ok(())
        }

        /// Remove an account from the allow-list.
        /// Can only be called by the root.
        #[weight = 0]
        pub fn remove_account(origin, account_to_remove: T::AccountId) -> dispatch::DispatchResult {
            ensure_root(origin)?;

            <AllowedAccounts<T>>::remove(&account_to_remove);

            Self::deposit_event(RawEvent::AccountRemoved(account_to_remove));

            Ok(())
        }
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Config>::AccountId,
    {
        // When a new account is added to the allow-list.
        AccountAllowed(AccountId),
        // When an account is removed from the allow-list.
        AccountRemoved(AccountId),
    }
);

/// The following section implements the `SignedExtension` trait
/// for the `AllowAccount` type.
/// `SignedExtension` is being used here to filter out the not allowed accounts
/// when they try to send extrinsics to the runtime.
/// Inside the `validate` function of the `SignedExtension` trait,
/// we check if the sender (origin) of the extrinsic is part of the
/// allow-list or not.
/// The extrinsic will be rejected as invalid if the origin is not part
/// of the allow-list.
/// The validation happens at the transaction queue level,
///  and the extrinsics are filtered out before they hit the pallet logic.

/// The `AllowAccount` struct.
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct AllowAccount<T: Trait + Send + Sync>(PhantomData<T>);

/// Debug impl for the `AllowAccount` struct.
impl<T: Trait + Send + Sync> Debug for AllowAccount<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "AllowAccount")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

/// Implementation of the `SignedExtension` trait for the `AllowAccount` struct.
impl<T: Trait + Send + Sync> SignedExtension for AllowAccount<T> where 
    T::Call: Dispatchable<Info=DispatchInfo> {
    type AccountId = T::AccountId;
	type Call = T::Call;
	type AdditionalSigned = ();
	type Pre = ();
	const IDENTIFIER: &'static str = "AllowAccount";

    fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> { Ok(()) }

    // Filter out the not allowed keys.
    // If the key is in the allow-list, return a valid transaction,
    // else return a custom error.
    fn validate(
        &self,
		who: &Self::AccountId,
		_call: &Self::Call,
		info: &DispatchInfoOf<Self::Call>,
		_len: usize,
    ) -> TransactionValidity {
        if <AllowedAccounts<T>>::contains_key(who) {
            Ok(ValidTransaction {
                priority: info.weight as TransactionPriority,
                longevity: TransactionLongevity::max_value(),
                propagate: true,
                ..Default::default()
            })
        } else {
            Err(InvalidTransaction::Call.into())
        }
    }
}
