#![cfg_attr(not(feature = "std"), no_std)]

// #[cfg(test)]
// mod mock;
// #[cfg(test)]
// mod tests;

use frame_support::Parameter;
use sp_std::prelude::*;

use frame_support::{sp_runtime::BoundToRuntimeAppPublic, traits::OneSessionHandler};
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use cord_primitives::{SessionApiError, DEFAULT_SESSION_PERIOD};
	use frame_support::{
		pallet_prelude::*,
		sp_runtime::{traits::OpaqueKeys, RuntimeAppPublic},
		sp_std,
	};
	use frame_system::pallet_prelude::*;
	use pallet_session::{Pallet as Session, SessionManager};

	#[pallet::type_value]
	pub fn DefaultValidators<T: Config>() -> Option<Vec<T::AccountId>> {
		None
	}

	#[pallet::storage]
	#[pallet::getter(fn validators)]
	pub type Validators<T: Config> =
		StorageValue<_, Option<Vec<T::AccountId>>, ValueQuery, DefaultValidators<T>>;

	#[pallet::type_value]
	pub fn DefaultSessionForValidatorsChange<T: Config>() -> Option<u32> {
		None
	}

	#[pallet::storage]
	#[pallet::getter(fn session_for_validators_change)]
	pub type SessionForValidatorsChange<T: Config> =
		StorageValue<_, Option<u32>, ValueQuery, DefaultSessionForValidatorsChange<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_session::Config {
		type AuthorityId: Member
			+ Parameter
			+ RuntimeAppPublic
			+ Default
			+ MaybeSerializeDeserialize;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type AuthorityOrigin: EnsureOrigin<Self::Origin>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// New Authority added.
		AddAuthority(T::AccountId, u32),
		// Authority removed.
		RemoveAuthority(T::AccountId, u32),
		//Rotate Validators
		ChangeAuthorities(Vec<T::AccountId>, u32),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		NoAuthorities,
	}

	pub struct CordSessionManager<T>(sp_std::marker::PhantomData<T>);

	#[pallet::pallet]
	pub struct Pallet<T>(sp_std::marker::PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
		pub fn add_validator(origin: OriginFor<T>, validator: T::AccountId) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;
			let mut validators: Vec<T::AccountId>;
			if <Validators<T>>::get().is_none() {
				validators = vec![validator.clone()];
			} else {
				validators = <Validators<T>>::get().unwrap();
				validators.push(validator.clone());
			}

			let current_session = Session::<T>::current_index();

			Validators::<T>::put(Some(validators));
			SessionForValidatorsChange::<T>::put(Some(current_session + 2));

			Self::deposit_event(Event::AddAuthority(validator, current_session + 2));
			Ok(())
		}

		#[pallet::weight((T::BlockWeights::get().max_block, DispatchClass::Operational))]
		pub fn remove_validator(origin: OriginFor<T>, validator: T::AccountId) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;
			let ref mut validators = <Validators<T>>::get().ok_or(Error::<T>::NoAuthorities)?;
			validators.retain(|x| x != &validator);

			let current_session = Session::<T>::current_index();

			Validators::<T>::put(Some(validators));
			SessionForValidatorsChange::<T>::put(Some(current_session + 2));

			Self::deposit_event(Event::RemoveAuthority(validator, current_session + 2));
			Ok(())
		}
	}

	#[pallet::storage]
	#[pallet::getter(fn authorities)]
	pub(super) type Authorities<T: Config> = StorageValue<_, Vec<T::AuthorityId>, ValueQuery>;

	#[pallet::type_value]
	pub(super) fn DefaultForSessionPeriod() -> u32 {
		DEFAULT_SESSION_PERIOD
	}

	#[pallet::storage]
	#[pallet::getter(fn session_period)]
	pub(super) type SessionPeriod<T: Config> =
		StorageValue<_, u32, ValueQuery, DefaultForSessionPeriod>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub authorities: Vec<T::AuthorityId>,
		pub session_period: u32,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { authorities: Vec::new(), session_period: DEFAULT_SESSION_PERIOD }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize_authorities(&self.authorities);
			<SessionPeriod<T>>::put(&self.session_period);
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn initialize_authorities(authorities: &[T::AuthorityId]) {
			if !authorities.is_empty() {
				assert!(<Authorities<T>>::get().is_empty(), "Authorities are already initialized!");
				<Authorities<T>>::put(authorities);
			}
		}

		pub(crate) fn update_authorities(authorities: &[T::AuthorityId]) {
			<Authorities<T>>::put(authorities);
		}

		pub fn next_session_authorities() -> Result<Vec<T::AuthorityId>, SessionApiError> {
			Session::<T>::queued_keys()
				.iter()
				.map(|(_, key)| key.get(T::AuthorityId::ID).ok_or(SessionApiError::DecodeKey))
				.collect::<Result<Vec<T::AuthorityId>, SessionApiError>>()
		}
	}

	impl<T: Config> SessionManager<T::AccountId> for CordSessionManager<T> {
		fn new_session(session: u32) -> Option<Vec<T::AccountId>> {
			if let Some(session_for_validators_change) =
				Pallet::<T>::session_for_validators_change()
			{
				if session_for_validators_change == session {
					let validators = Pallet::<T>::validators().expect(
						"Validators also should be Some(), when session_for_validators_change is",
					);
					return Some(validators);
				}
			}
			None
		}

		fn start_session(_: u32) {}

		fn end_session(_: u32) {}
	}

	impl<T: Config> BoundToRuntimeAppPublic for Pallet<T> {
		type Public = T::AuthorityId;
	}

	impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
		type Key = T::AuthorityId;

		fn on_genesis_session<'a, I: 'a>(validators: I)
		where
			I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
			T::AccountId: 'a,
		{
			let authorities = validators.map(|(_, key)| key).collect::<Vec<_>>();
			Self::initialize_authorities(authorities.as_slice());
		}

		fn on_new_session<'a, I: 'a>(_changed: bool, validators: I, _queued_validators: I)
		where
			I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
			T::AccountId: 'a,
		{
			let authorities = validators.map(|(_, key)| key).collect::<Vec<_>>();
			Self::update_authorities(authorities.as_slice());
		}

		fn on_disabled(_validator_index: usize) {}
	}
}
