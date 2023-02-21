// use crate::{Config, Pallet};
// use codec::{Decode, Encode};
// use frame_support::dispatch::DispatchInfo;
// use scale_info::TypeInfo;
// use sp_runtime::{
// 	traits::{DispatchInfoOf, Dispatchable, SignedExtension},
// 	transaction_validity::{
// 		InvalidTransaction, TransactionLongevity, TransactionValidity, TransactionValidityError,
// 		ValidTransaction,
// 	},
// };
// use sp_std::{marker::PhantomData, prelude::*};

// /// The `CheckAuthorRegistry` struct.
// #[derive(Encode, Decode, Clone, Eq, PartialEq, Default, scale_info::TypeInfo)]
// #[scale_info(skip_type_params(T))]
// pub struct CheckExtrinsicAuthor<T: Config + Send + Sync>(PhantomData<T>);

// impl<T: Config + Send + Sync> sp_std::fmt::Debug for CheckExtrinsicAuthor<T> {
// 	#[cfg(feature = "std")]
// 	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
// 		write!(f, "CheckExtrinsicAuthor")
// 	}

// 	#[cfg(not(feature = "std"))]
// 	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
// 		Ok(())
// 	}
// }

// impl<T: Config + Send + Sync> CheckExtrinsicAuthor<T> {
// 	/// Create new `SignedExtension` to check author permission.
// 	pub fn new() -> Self {
// 		Self(sp_std::marker::PhantomData)
// 	}
// }

// /// Implementation of the `SignedExtension` trait for the
// /// `CheckAuthorRegistry` struct.
// impl<T: Config + Send + Sync> SignedExtension for CheckExtrinsicAuthor<T>
// where
// 	T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
// {
// 	type AccountId = T::AccountId;
// 	type Call = T::RuntimeCall;
// 	type AdditionalSigned = ();
// 	type Pre = ();
// 	const IDENTIFIER: &'static str = "CheckExtrinsicAuthor";

// 	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
// 		Ok(())
// 	}

// 	fn pre_dispatch(
// 		self,
// 		who: &Self::AccountId,
// 		call: &Self::Call,
// 		info: &DispatchInfoOf<Self::Call>,
// 		len: usize,
// 	) -> Result<Self::Pre, TransactionValidityError> {
// 		self.validate(who, call, info, len).map(|_| ())
// 	}

// 	fn validate(
// 		&self,
// 		who: &Self::AccountId,
// 		_call: &Self::Call,
// 		_info: &DispatchInfoOf<Self::Call>,
// 		_len: usize,
// 	) -> TransactionValidity {
// 		if <Pallet<T>::ExtrinsicAuthors<T>>::contains_key(who) {
// 			Ok(ValidTransaction {
// 				priority: 0,
// 				longevity: TransactionLongevity::max_value(),
// 				propagate: true,
// 				..Default::default()
// 			})
// 		} else {
// 			Err(InvalidTransaction::Call.into())
// 		}
// 	}
// }
