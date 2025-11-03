#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::register::LookupSpec;
use cord_primitives::packet::ElementType;
use frame_benchmarking::{v2::*, BenchmarkError};
use frame_support::{ensure, traits::PalletInfoAccess};
use frame_system::RawOrigin;
use pallet_entity::Ss58OfActiveAccounts;
use sp_runtime::traits::Hash;

fn element_from_bytes<T: Config>(data: &[u8]) -> Element<T::MaxRawDataLength> {
	Element::Raw(data.to_vec().try_into().expect("bounded element"))
}

frame_benchmarking::benchmarks! {
	where_clause { where T: pallet_entity::Config }

	create_registry {
		let caller: T::AccountId = whitelisted_caller();
		let hash = T::Hashing::hash(&caller.encode());
		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let token = <T as pallet::Config>::Token::build(hash.as_ref(), pallet_name)
			.map_err(|_| BenchmarkError::Stop("failed to build entity token"))?;
		Ss58OfActiveAccounts::<T>::insert(caller.clone(), token.clone());

		let mut attributes = AttributeSchemaListOf::<T>::default();
		attributes
			.try_push((Attribute::try_from(b"key".to_vec()).unwrap(), ElementType::Raw))
			.expect("attribute push");

		let token_spec =
			LookupSpec::Single(Attribute::try_from(b"key".to_vec()).expect("within key bound"));
	}: _<T::RuntimeOrigin>(
		RawOrigin::Signed(caller.clone()).into(),
		element_from_bytes::<T>(b"info"),
		RegistryKind::Raw,
		true,
		attributes,
		token_spec,
		LookupSpecListOf::<T>::default()
	)
	verify {
		assert!(Registries::<T>::iter_keys().next().is_some());
	}

	set_registry_delegate {
		let caller: T::AccountId = whitelisted_caller();
		let hash = T::Hashing::hash(&caller.encode());
		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let token = <T as pallet::Config>::Token::build(hash.as_ref(), pallet_name)
			.map_err(|_| BenchmarkError::Stop("failed to build entity token"))?;
		Ss58OfActiveAccounts::<T>::insert(caller.clone(), token.clone());

		let mut attributes = AttributeSchemaListOf::<T>::default();
		attributes
			.try_push((Attribute::try_from(b"key".to_vec()).unwrap(), ElementType::Raw))
			.expect("attribute push");

		let token_spec =
			LookupSpec::Single(Attribute::try_from(b"key".to_vec()).expect("within key bound"));

		Pallet::<T>::create_registry(
			RawOrigin::Signed(caller.clone()).into(),
			element_from_bytes::<T>(b"info"),
			RegistryKind::Raw,
			true,
			attributes,
			token_spec.clone(),
			LookupSpecListOf::<T>::default(),
		)?;

		let registry = Registries::<T>::iter_keys().next().ok_or(BenchmarkError::Stop("missing registry"))?;

		let delegate: T::AccountId = frame_benchmarking::v2::account("deleg", 0, 0);
		let delegate_hash = T::Hashing::hash(&delegate.encode());
		let delegate_token = <T as pallet::Config>::Token::build(delegate_hash.as_ref(), pallet_name)
			.map_err(|_| BenchmarkError::Stop("failed to build delegate token"))?;
		Ss58OfActiveAccounts::<T>::insert(delegate.clone(), delegate_token.clone());
		let roles = vec![RegistryPermissions::ENTRY];
	}: _<T::RuntimeOrigin>(
		RawOrigin::Signed(caller.clone()).into(),
		registry.clone(),
		delegate.clone(),
		roles.clone()
	)
	verify {
		let delegate_token = T::EntityLookup::lookup_token_of(&delegate)
			.map_err(|_| BenchmarkError::Stop("delegate token missing"))?;
		ensure!(RegistryDelegates::<T>::get(registry, delegate_token).is_some(), BenchmarkError::Stop("delegate not recorded"));
	}

	update_registry_info {
		let caller: T::AccountId = whitelisted_caller();
		let hash = T::Hashing::hash(&caller.encode());
		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let token = <T as pallet::Config>::Token::build(hash.as_ref(), pallet_name)
			.map_err(|_| BenchmarkError::Stop("failed to build entity token"))?;
		Ss58OfActiveAccounts::<T>::insert(caller.clone(), token.clone());

		let mut attributes = AttributeSchemaListOf::<T>::default();
		attributes
			.try_push((Attribute::try_from(b"key".to_vec()).unwrap(), ElementType::Raw))
			.expect("attribute push");

		let token_spec =
			LookupSpec::Single(Attribute::try_from(b"key".to_vec()).expect("within key bound"));

		Pallet::<T>::create_registry(
			RawOrigin::Signed(caller.clone()).into(),
			element_from_bytes::<T>(b"info"),
			RegistryKind::Raw,
			true,
			attributes,
			token_spec.clone(),
			LookupSpecListOf::<T>::default(),
		)?;

		let registry = Registries::<T>::iter_keys().next().ok_or(BenchmarkError::Stop("missing registry"))?;
		let new_info = element_from_bytes::<T>(b"new-info");
	}: _<T::RuntimeOrigin>(
		RawOrigin::Signed(caller.clone()).into(),
		registry.clone(),
		new_info.clone()
	)
	verify {
		let stored = Registries::<T>::get(&registry).expect("registry exists");
		ensure!(stored.info == new_info, BenchmarkError::Stop("info not updated"));
	}

	set_registry_status {
		let caller: T::AccountId = whitelisted_caller();
		let hash = T::Hashing::hash(&caller.encode());
		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let token = <T as pallet::Config>::Token::build(hash.as_ref(), pallet_name)
			.map_err(|_| BenchmarkError::Stop("failed to build entity token"))?;
		Ss58OfActiveAccounts::<T>::insert(caller.clone(), token.clone());

		let mut attributes = AttributeSchemaListOf::<T>::default();
		attributes
			.try_push((Attribute::try_from(b"key".to_vec()).unwrap(), ElementType::Raw))
			.expect("attribute push");

		let token_spec =
			LookupSpec::Single(Attribute::try_from(b"key".to_vec()).expect("within key bound"));

		Pallet::<T>::create_registry(
			RawOrigin::Signed(caller.clone()).into(),
			element_from_bytes::<T>(b"info"),
			RegistryKind::Raw,
			true,
			attributes,
			token_spec,
			LookupSpecListOf::<T>::default(),
		)?;

		let registry = Registries::<T>::iter_keys().next().ok_or(BenchmarkError::Stop("missing registry"))?;
	}: _<T::RuntimeOrigin>(RawOrigin::Root.into(), registry.clone(), false)
	verify {
		ensure!(!Registries::<T>::get(&registry).unwrap().is_active, BenchmarkError::Stop("status not updated"));
	}

}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
