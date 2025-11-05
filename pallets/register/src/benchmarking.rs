#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::register::{AttributeFlags, AttributeSpec, LookupSpec, RegistryPermissions, RegistryStatus};
use cord_primitives::packet::ElementType;
use frame_benchmarking::{v2::*, BenchmarkError};
use frame_support::{ensure, traits::PalletInfoAccess};
use frame_system::RawOrigin;
use pallet_entity::Ss58OfActiveAccounts;
use sp_runtime::traits::Hash;

pub trait EntityBinder<T: Config> {
	fn bind_account(account: &T::AccountId, token: &Ss58Identifier);
}

impl<T> EntityBinder<T> for pallet_entity::Pallet<T>
where
	T: Config + pallet_entity::Config,
{
	fn bind_account(account: &T::AccountId, token: &Ss58Identifier) {
		Ss58OfActiveAccounts::<T>::insert(account, token);
	}
}

fn bind_entity<T: Config>(account: &T::AccountId, token: &Ss58Identifier)
where
	T::EntityLookup: EntityBinder<T>,
{
	<T::EntityLookup as EntityBinder<T>>::bind_account(account, token);
}

fn element_from_bytes<T: Config>(data: &[u8]) -> Element<T::MaxRawDataLength> {
	Element::Raw(data.to_vec().try_into().expect("bounded element"))
}

frame_benchmarking::benchmarks! {
	where_clause { where T::EntityLookup: EntityBinder<T> }

	create_registry {
		let caller: T::AccountId = whitelisted_caller();
		let hash = T::Hashing::hash(&caller.encode());
		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let token = <T as pallet::Config>::Token::build(hash.as_ref(), pallet_name)
			.map_err(|_| BenchmarkError::Stop("failed to build entity token"))?;
		bind_entity::<T>(&caller, &token);

		let mut attributes = AttributeSchemaListOf::<T>::default();
		attributes
			.try_push(AttributeSpec {
				key: Attribute::try_from(b"key".to_vec()).unwrap(),
				kind: ElementType::Raw,
				flags: AttributeFlags::empty(),
			})
			.expect("attribute push");

		let token_spec =
			LookupSpec::Single(Attribute::try_from(b"key".to_vec()).expect("within key bound"));
		let mut lookup_specs = LookupSpecListOf::<T>::default();
		lookup_specs.try_push(token_spec.clone()).expect("lookup push");
}: _<T::RuntimeOrigin>(
	RawOrigin::Signed(caller.clone()).into(),
	element_from_bytes::<T>(b"info"),
	RegistryKind::Raw,
	attributes,
	token_spec,
	lookup_specs
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
		bind_entity::<T>(&caller, &token);

		let mut attributes = AttributeSchemaListOf::<T>::default();
		attributes
			.try_push(AttributeSpec {
				key: Attribute::try_from(b"key".to_vec()).unwrap(),
				kind: ElementType::Raw,
				flags: AttributeFlags::empty(),
			})
			.expect("attribute push");

		let token_spec =
			LookupSpec::Single(Attribute::try_from(b"key".to_vec()).expect("within key bound"));
		let mut lookup_specs = LookupSpecListOf::<T>::default();
		lookup_specs.try_push(token_spec.clone()).expect("lookup push");

		Pallet::<T>::create_registry(
			RawOrigin::Signed(caller.clone()).into(),
			element_from_bytes::<T>(b"info"),
			RegistryKind::Raw,
			attributes,
			token_spec.clone(),
			lookup_specs.clone(),
		)?;

		let registry = Registries::<T>::iter_keys().next().ok_or(BenchmarkError::Stop("missing registry"))?;

		let delegate: T::AccountId = frame_benchmarking::v2::account("deleg", 0, 0);
		let delegate_hash = T::Hashing::hash(&delegate.encode());
		let delegate_token = <T as pallet::Config>::Token::build(delegate_hash.as_ref(), pallet_name)
			.map_err(|_| BenchmarkError::Stop("failed to build delegate token"))?;
		bind_entity::<T>(&delegate, &delegate_token);
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
		bind_entity::<T>(&caller, &token);

		let mut attributes = AttributeSchemaListOf::<T>::default();
		attributes
			.try_push(AttributeSpec {
				key: Attribute::try_from(b"key".to_vec()).unwrap(),
				kind: ElementType::Raw,
				flags: AttributeFlags::empty(),
			})
			.expect("attribute push");

		let token_spec =
			LookupSpec::Single(Attribute::try_from(b"key".to_vec()).expect("within key bound"));
		let mut lookup_specs = LookupSpecListOf::<T>::default();
		lookup_specs.try_push(token_spec.clone()).expect("lookup push");

		Pallet::<T>::create_registry(
			RawOrigin::Signed(caller.clone()).into(),
			element_from_bytes::<T>(b"info"),
			RegistryKind::Raw,
			attributes,
			token_spec.clone(),
			lookup_specs.clone(),
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

	revoke_registry {
		let caller: T::AccountId = whitelisted_caller();
		let hash = T::Hashing::hash(&caller.encode());
		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let token = <T as pallet::Config>::Token::build(hash.as_ref(), pallet_name)
			.map_err(|_| BenchmarkError::Stop("failed to build entity token"))?;
		bind_entity::<T>(&caller, &token);

		let mut attributes = AttributeSchemaListOf::<T>::default();
		attributes
			.try_push(AttributeSpec {
				key: Attribute::try_from(b"key".to_vec()).unwrap(),
				kind: ElementType::Raw,
				flags: AttributeFlags::empty(),
			})
			.expect("attribute push");

		let token_spec =
			LookupSpec::Single(Attribute::try_from(b"key".to_vec()).expect("within key bound"));
		let mut lookup_specs = LookupSpecListOf::<T>::default();
		lookup_specs.try_push(token_spec.clone()).expect("lookup push");

		Pallet::<T>::create_registry(
			RawOrigin::Signed(caller.clone()).into(),
			element_from_bytes::<T>(b"info"),
			RegistryKind::Raw,
			attributes,
			token_spec,
			lookup_specs,
		)?;

		let registry = Registries::<T>::iter_keys().next().ok_or(BenchmarkError::Stop("missing registry"))?;
	}: _<T::RuntimeOrigin>(RawOrigin::Signed(caller.clone()).into(), registry.clone())
	verify {
		let info = Registries::<T>::get(&registry).expect("registry exists");
		ensure!(info.status == RegistryStatus::Revoked, BenchmarkError::Stop("status not updated"));
	}

}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
