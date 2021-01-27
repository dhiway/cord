// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! Error: Handles errors for all other runtime modules
#![cfg_attr(not(feature = "std"), no_std)]

use core::convert::From;
use frame_support::{debug, decl_event, decl_module, dispatch, Parameter};
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::traits::{Bounded, MaybeDisplay, MaybeSerialize, Member};

/// The error trait
pub trait Trait: frame_system::Config {
	type ErrorCode: BaseArithmetic
		+ Parameter
		+ Member
		+ MaybeSerialize
		+ MaybeDisplay
		+ Bounded
		+ From<u16>;
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

/// The error type is a tuple of error code and an error message
pub type ErrorType = (u16, &'static str);

decl_event!(
	/// Events for errors
	pub enum Event<T> where <T as Trait>::ErrorCode {
		// An error occurred
		ErrorOccurred(ErrorCode),
	}
);

decl_module! {
	/// The error runtime module. Since it is used by other modules to deposit events, it has no transaction functions.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		/// Deposit events
		fn deposit_event() = default;

	}
}

/// Implementation of further module functions for errors
impl<T: Trait> Module<T> {
	/// Create an error, it logs the error, deposits an error event and returns the error with its message
	pub fn error(error_type: ErrorType) -> dispatch::DispatchResult {
		Err(Self::deposit_err(error_type))
	}

	/// Create an error, it logs the error, deposits an error event and returns the error message
	pub fn deposit_err(error_type: ErrorType) -> dispatch::DispatchError {
		debug::print!("{}", error_type.1);
		Self::deposit_event(RawEvent::ErrorOccurred(error_type.0.into()));
		dispatch::DispatchError::Other(error_type.1)
	}

	pub fn ok_or_deposit_err<S>(
		opt: Option<S>,
		error_type: ErrorType,
	) -> Result<S, dispatch::DispatchError> {
		if let Some(s) = opt {
			Ok(s)
		} else {
			Err(Self::deposit_err(error_type))
		}
	}
}
