// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! # Meta Tx (Meta Transaction) Pallet
//!
//! This pallet enables the dispatch of transactions that are authorized by one party (the signer)
//! and executed by an untrusted third party (the relayer), who covers the transaction fees.
//!
//! ## Pallet API
//!
//! See the [`pallet`] module for more information about the interfaces this pallet exposes,
//! including its configuration trait, dispatchables, storage items, events and errors.
//!
//! ## Overview
//!
//! The pallet provides a client-level API, typically not meant for direct use by end users.
//! A meta transaction, constructed with the help of a wallet, contains a target call, necessary
//! extensions, and the signer's signature. This transaction is then broadcast, and any interested
//! relayer can pick it up and execute it. The relayer submits a regular transaction via the
//! [`dispatch`](`Pallet::dispatch`) function, passing the meta transaction as an argument to
//! execute the target call on behalf of the signer while covering the fees.
//!
//! ### Example
#![doc = docify::embed!("src/tests.rs", sign_and_execute_meta_tx)]
//!
//! ## Low-Level / Implementation Details
//!
//! The structure of a meta transaction is identical to the
//! [`General`](sp_runtime::generic::Preamble::General) transaction.
//! It contains the target call along with a configurable set of extensions and its associated
//! version. Typically, these extensions include type like
//! `pallet_verify_signature::VerifySignature`, which provides the signer address
//! and the signature of the payload, encompassing the call and the meta-transaction’s
//! configurations, such as its mortality.  The extensions follow the same [`TransactionExtension`]
//! contract, and common types such as [`frame_system::CheckGenesis`],
//! [`frame_system::CheckMortality`], [`frame_system::CheckNonce`], etc., are applicable in the
//! context of meta transactions. Check the `mock` setup for the example.

#![cfg_attr(not(feature = "std"), no_std)]

mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(all(test, not(feature = "runtime-benchmarks")))]
mod tests;
pub mod weights;
#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::types::WeightlessExtension;
pub use pallet::*;
pub use weights::WeightInfo;
mod extension;
pub use extension::MetaTxMarker;
extern crate alloc;
use alloc::boxed::Box;

use core::ops::Add;
use frame_support::{
	dispatch::{DispatchInfo, GetDispatchInfo, PostDispatchInfo},
	pallet_prelude::*,
};
use frame_system::{pallet_prelude::*, RawOrigin as SystemOrigin};
use sp_runtime::{
	generic::ExtensionVersion,
	traits::{
		AsTransactionAuthorizedOrigin, DispatchTransaction, Dispatchable, TransactionExtension,
	},
};

/// Meta Transaction type.
///
/// The data that is provided and signed by the signer and shared with the relayer.
#[derive(Encode, Decode, PartialEq, Eq, TypeInfo, Clone, Debug)]
pub struct MetaTx<Call, Extension> {
	/// The target call to be executed on behalf of the signer.
	call: Call,
	/// The extension version.
	extension_version: ExtensionVersion,
	/// The extension/s for the meta transaction.
	extension: Extension,
}

impl<Call, Extension> MetaTx<Call, Extension> {
	/// Create a new meta transaction.
	pub fn new(call: Call, extension_version: ExtensionVersion, extension: Extension) -> Self {
		Self { call, extension_version, extension }
	}
}

/// The [`MetaTx`] for the given config.
pub type MetaTxFor<T> = MetaTx<<T as frame_system::Config>::RuntimeCall, <T as Config>::Extension>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config:
		frame_system::Config<
		RuntimeCall: Dispatchable<
			Info = DispatchInfo,
			PostInfo = PostDispatchInfo,
			RuntimeOrigin = <Self as frame_system::Config>::RuntimeOrigin,
		>,
		RuntimeOrigin: AsTransactionAuthorizedOrigin + From<SystemOrigin<Self::AccountId>>,
	>
	{
		/// Weight information for calls in this pallet.
		type WeightInfo: WeightInfo;
		/// The overarching event type.
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Transaction extension/s for meta transactions.
		///
		/// The extensions that must be present in every meta transaction. This generally includes
		/// extensions like `pallet_verify_signature::VerifySignature`,
		/// [frame_system::CheckSpecVersion], [frame_system::CheckTxVersion],
		/// [frame_system::CheckGenesis], [frame_system::CheckMortality],
		/// [frame_system::CheckNonce], etc. Check the `mock` setup for the example.
		///
		/// The types implementing the [`TransactionExtension`] trait can be composed into a tuple
		/// type that will implement the same trait by piping invocations through each type.
		///
		/// In the `runtime-benchmarks` environment the type must implement [`Default`] trait.
		/// The extension must provide an origin and the extension's weight must be zero. Use
		/// `pallet_meta_tx::WeightlessExtension` type when the `runtime-benchmarks` feature
		/// enabled.
		type Extension: TransactionExtension<<Self as frame_system::Config>::RuntimeCall>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid proof (e.g. signature).
		BadProof,
		/// The meta transaction is not yet valid (e.g. nonce too high).
		Future,
		/// The meta transaction is outdated (e.g. nonce too low).
		Stale,
		/// The meta transactions's birth block is ancient.
		AncientBirthBlock,
		/// The transaction extension did not authorize any origin.
		UnknownOrigin,
		/// The meta transaction is invalid.
		Invalid,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A meta transaction has been dispatched.
		///
		/// Contains the dispatch result of the meta transaction along with post-dispatch
		/// information.
		Dispatched { result: DispatchResultWithPostInfo },
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Dispatch a given meta transaction.
		///
		/// - `_origin`: Can be any kind of origin.
		/// - `meta_tx`: Meta Transaction with a target call to be dispatched.
		#[pallet::call_index(0)]
		#[pallet::weight({
			let dispatch_info = meta_tx.call.get_dispatch_info();
			let extension_weight = meta_tx.extension.weight(&meta_tx.call);
			let bare_call_weight = T::WeightInfo::bare_dispatch();
			(
				dispatch_info.call_weight.add(extension_weight).add(bare_call_weight),
				dispatch_info.class,
			)
		})]
		pub fn dispatch(
			_origin: OriginFor<T>,
			meta_tx: Box<MetaTxFor<T>>,
		) -> DispatchResultWithPostInfo {
			let origin = SystemOrigin::None;
			let meta_tx_size = meta_tx.encoded_size();
			// `info` with worst-case call weight and extension weight.
			let info = {
				let mut info = meta_tx.call.get_dispatch_info();
				info.extension_weight = meta_tx.extension.weight(&meta_tx.call);
				info
			};

			// dispatch the meta transaction.
			let meta_dispatch_res = meta_tx
				.extension
				.dispatch_transaction(
					origin.into(),
					meta_tx.call,
					&info,
					meta_tx_size,
					meta_tx.extension_version,
				)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::Dispatched { result: meta_dispatch_res });

			// meta weight after possible refunds.
			let meta_weight = meta_dispatch_res
				.map_or_else(|err| err.post_info.actual_weight, |info| info.actual_weight)
				.unwrap_or(info.total_weight());

			Ok((Some(T::WeightInfo::bare_dispatch().saturating_add(meta_weight)), true.into())
				.into())
		}
	}

	/// Implements [`From<TransactionValidityError>`] for [`Error`] by mapping the relevant error
	/// variants.
	impl<T> From<TransactionValidityError> for Error<T> {
		fn from(err: TransactionValidityError) -> Self {
			use TransactionValidityError::*;
			match err {
				Unknown(_) => Error::<T>::Invalid,
				Invalid(err) => match err {
					InvalidTransaction::BadProof => Error::<T>::BadProof,
					InvalidTransaction::Future => Error::<T>::Future,
					InvalidTransaction::Stale => Error::<T>::Stale,
					InvalidTransaction::AncientBirthBlock => Error::<T>::AncientBirthBlock,
					InvalidTransaction::UnknownOrigin => Error::<T>::UnknownOrigin,
					_ => Error::<T>::Invalid,
				},
			}
		}
	}
}
