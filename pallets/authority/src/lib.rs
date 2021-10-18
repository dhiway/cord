//! # Authorities Pallet
//!
//! The Authorities Pallet allows addition and removal of authorities
//!
//! The pallet uses the Session pallet and implements related traits for session
//! management.

#![cfg_attr(not(feature = "std"), no_std)]

use pallet_session::Pallet as Session;
use sp_runtime::traits::{Convert, Zero};
use sp_std::prelude::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_session::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type AuthorityOrigin: EnsureOrigin<Self::Origin>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// The pallet's storage items.
	#[pallet::storage]
	#[pallet::getter(fn authorities)]
	pub type Authorities<T: Config> = StorageValue<_, Vec<T::AccountId>>;

	#[pallet::storage]
	#[pallet::getter(fn flag)]
	pub type Flag<T: Config> = StorageValue<_, bool>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// New Authority added.
		AuthorityAdded(T::AccountId),
		// Authority removed.
		AuthorityRemoved(T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		NoAuthorities,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub authorities: Vec<T::AccountId>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { authorities: Vec::new() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize_authoritiess(&self.authorities);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a new authority.
		///
		/// New authority's session keys should be set in session module before calling this.
		#[pallet::weight(0)]
		pub fn add_authority(origin: OriginFor<T>, authority_id: T::AccountId) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			let mut authorities: Vec<T::AccountId>;

			if <Authorities<T>>::get().is_none() {
				authorities = vec![authority_id.clone()];
			} else {
				authorities = <Authorities<T>>::get().unwrap();
				authorities.push(authority_id.clone());
			}

			<Authorities<T>>::put(authorities);

			// Calling rotate_session to queue the new session keys.
			Session::<T>::rotate_session();

			// Triggering rotate session again for the queued keys to take effect.
			Flag::<T>::put(true);

			Self::deposit_event(Event::AuthorityAdded(authority_id));
			Ok(())
		}

		/// Remove an authority.
		///
		#[pallet::weight(0)]
		pub fn remove_authority(
			origin: OriginFor<T>,
			authority_id: T::AccountId,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			let ref mut authorities = <Authorities<T>>::get().ok_or(Error::<T>::NoAuthorities)?;
			authorities.retain(|x| x != &authority_id);

			<Authorities<T>>::put(authorities);

			// Calling rotate_session to queue the new session keys.
			<pallet_session::Pallet<T>>::rotate_session();

			// Triggering rotate session again for the queued keys to take effect.
			Flag::<T>::put(true);

			Self::deposit_event(Event::AuthorityRemoved(authority_id));
			Ok(())
		}

		/// Force rotate session using elevated privileges.
		#[pallet::weight(0)]
		pub fn force_rotate_session(origin: OriginFor<T>) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			<pallet_session::Pallet<T>>::rotate_session();

			// Triggering rotate session again for any queued keys to take effect.
			// Not sure if double rotate is needed in this scenario. **TODO**
			Flag::<T>::put(true);
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn initialize_authoritiess(authorities: &[T::AccountId]) {
		if !authorities.is_empty() {
			assert!(<Authorities<T>>::get().is_none(), "Authorities are already initialized!");
			<Authorities<T>>::put(authorities);
		}
	}
}

/// Indicates to the session module if the session should be rotated.
/// We set this flag to true when we add/remove a validator.
impl<T: Config> pallet_session::ShouldEndSession<T::BlockNumber> for Pallet<T> {
	fn should_end_session(_now: T::BlockNumber) -> bool {
		Self::flag().unwrap()
	}
}

/// Provides the new set of validators to the session module when session is being rotated.
impl<T: Config> pallet_session::SessionManager<T::AccountId> for Pallet<T> {
	fn new_session(_new_index: u32) -> Option<Vec<T::AccountId>> {
		// Flag is set to false so that the session doesn't keep rotating.
		Flag::<T>::put(false);

		Self::authorities()
	}

	fn end_session(_end_index: u32) {}

	fn start_session(_start_index: u32) {}
}

impl<T: Config> frame_support::traits::EstimateNextSessionRotation<T::BlockNumber> for Pallet<T> {
	fn average_session_length() -> T::BlockNumber {
		Zero::zero()
	}

	fn estimate_current_session_progress(
		_now: T::BlockNumber,
	) -> (Option<sp_runtime::Permill>, frame_support::dispatch::Weight) {
		(None, Zero::zero())
	}

	fn estimate_next_session_rotation(
		_now: T::BlockNumber,
	) -> (Option<T::BlockNumber>, frame_support::dispatch::Weight) {
		(None, Zero::zero())
	}
}

/// Implementation of Convert trait for mapping AuthorityId with AccountId.
/// This is mainly used to map stash and controller keys.
/// In this module, for simplicity, we just return the same AccountId.
pub struct AuthorityOf<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Convert<T::AccountId, Option<T::AccountId>> for AuthorityOf<T> {
	fn convert(account: T::AccountId) -> Option<T::AccountId> {
		Some(account)
	}
}
