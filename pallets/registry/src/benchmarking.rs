#![cfg(feature = "runtime-benchmarks")]

use super::*;
use codec::Encode;
use cord_primitives::curi::Ss58Identifier;
use cord_utilities::traits::GenerateBenchmarkOrigin;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{pallet_prelude::*, sp_runtime::traits::Hash};
use sp_std::{
	convert::{TryFrom, TryInto},
	vec::Vec,
};
const SEED: u32 = 0;
const MAX_REGISTRY_SIZE: u32 = 15 * 1024;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
	where_clause {
		where
		<T as Config>::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::RegistryCreatorId>,
		}
	create {
		let l in 1 .. MAX_REGISTRY_SIZE;

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::RegistryCreatorId = account("did", 0, SEED);

		let raw_registry: Vec<u8> = (0u8..u8::MAX).cycle().take(T::MaxEncodedRegistryLength::get().try_into().unwrap()).collect();
		let registry: InputRegistryOf::<T> = BoundedVec::try_from(raw_registry)
			.expect("Test Registry should fit into the expected input length of the test runtime");
		let digest = <T as frame_system::Config>::Hashing::hash(&registry[..]);
		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&registry.encode()[..], &did.encode()[..]].concat()[..],
		);
		let registry_id = Ss58Identifier::to_registry_id(&(id_digest).encode()[..]).unwrap();

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());

	}: _<T::RuntimeOrigin>(origin, registry, None)
	verify {
	  //checks whether the last event was a successful creation of the registry
		assert_last_event::<T>(Event::Create { registry: registry_id, creator:did }.into());
	}

	update {
		let l in 1 .. MAX_REGISTRY_SIZE;

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::RegistryCreatorId = account("did", 0, SEED);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());

		let raw_registry: Vec<u8> = (0u8..u8::MAX).cycle().take(T::MaxEncodedRegistryLength::get().try_into().unwrap()).collect();
		let registry: InputRegistryOf::<T> = BoundedVec::try_from(raw_registry)
			.expect("Test Registry should fit into the expected input length of the test runtime");
		let digest = <T as frame_system::Config>::Hashing::hash(&registry[..]);
		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&registry.encode()[..], &did.encode()[..]].concat()[..],
		);
		let registry_id = Ss58Identifier::to_registry_id(&(id_digest).encode()[..]).unwrap();

		//Pallet::<T>::create(origin.clone(), registry, None).expect("Should create a registry entry");
        Pallet::<T>::create(origin.clone(), registry, Option::None).expect("Should create a registry entry");

		let registry_update: Vec<u8> = (2u8..u8::MAX).cycle().take(T::MaxEncodedRegistryLength::get().try_into().unwrap()).collect();
		let utx_registry: InputRegistryOf::<T> = BoundedVec::try_from(registry_update)
			.expect("Update Registry should fit into the expected input length of the test runtime");


	}: _<T::RuntimeOrigin>(origin, utx_registry, registry_id.clone())
	verify {
		assert_last_event::<T>(Event::Update { registry: registry_id, authority: did }.into());
	}
 
archive {
        let l in 1 .. MAX_REGISTRY_SIZE;

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::RegistryCreatorId = account("did", 0, SEED);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());

		let raw_registry: Vec<u8> = (0u8..u8::MAX).cycle().take(T::MaxEncodedRegistryLength::get().try_into().unwrap()).collect();
		let registry: InputRegistryOf::<T> = BoundedVec::try_from(raw_registry)
			.expect("Test Registry should fit into the expected input length of the test runtime");
		let digest = <T as frame_system::Config>::Hashing::hash(&registry[..]);
		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&registry.encode()[..], &did.encode()[..]].concat()[..],
		);
		let registry_id = Ss58Identifier::to_registry_id(&(id_digest).encode()[..]).unwrap();

		Pallet::<T>::create(origin.clone(), registry, None).expect("Should create a registry entry");

		let registry_archive: Vec<u8> = (2u8..u8::MAX).cycle().take(T::MaxEncodedRegistryLength::get().try_into().unwrap()).collect();
		let utx_registry: InputRegistryOf::<T> = BoundedVec::try_from(registry_archive)
			.expect("Archived Registry should fit into the expected input length of the test runtime");

}: _<T::RuntimeOrigin>(origin, registry_id.clone())

verify {
    let archived_registry = <Registries<T>>::get(&registry_id).expect("Registry should exist");
    assert!(archived_registry.archive, "Registry should be archived");

    assert_last_event::<T>(Event::Archive { registry: registry_id, authority: did }.into());
}    

restore {
    let l in 1 .. MAX_REGISTRY_SIZE;

    let caller: T::AccountId = account("caller", 0, SEED);
    let did: T::RegistryCreatorId = account("did", 0, SEED);

    let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());

    let raw_registry: Vec<u8> = (0u8..u8::MAX).cycle().take(T::MaxEncodedRegistryLength::get().try_into().unwrap()).collect();
    let registry: InputRegistryOf::<T> = BoundedVec::try_from(raw_registry)
        .expect("Test Registry should fit into the expected input length of the test runtime");
    let digest = <T as frame_system::Config>::Hashing::hash(&registry[..]);
    let id_digest = <T as frame_system::Config>::Hashing::hash(
        &[&registry.encode()[..], &did.encode()[..]].concat()[..],
    );
    let registry_id = Ss58Identifier::to_registry_id(&(id_digest).encode()[..]).unwrap();

    Pallet::<T>::create(origin.clone(), registry, None).expect("Should create a registry entry");
    Pallet::<T>::archive(origin.clone(), registry_id.clone()).expect("Should archive the registry");

}: _<T::RuntimeOrigin>(origin, registry_id.clone())

verify {
    assert_last_event::<T>(Event::Restore { registry: registry_id, authority: did }.into());
}

add_admin_delegate {
    let l in 1 .. MAX_REGISTRY_SIZE;

    let caller: T::AccountId = account("caller", 0, SEED);
    let did: T::RegistryCreatorId = account("did", 0, SEED);
    let delegate: T::RegistryCreatorId = account("delegate", 0, SEED);

    let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());

    let raw_registry: Vec<u8> = (0u8..u8::MAX).cycle().take(T::MaxEncodedRegistryLength::get().try_into().unwrap()).collect();
    let registry: InputRegistryOf::<T> = BoundedVec::try_from(raw_registry)
        .expect("Test Registry should fit into the expected input length of the test runtime");
    let id_digest = <T as frame_system::Config>::Hashing::hash(
        &[&registry.encode()[..], &did.encode()[..]].concat()[..],
    );
    let registry_id = Ss58Identifier::to_registry_id(&(id_digest).encode()[..]).unwrap();

    Pallet::<T>::create(origin.clone(), registry, None).expect("Should create a registry entry");

    let origin_creator =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());
   
}: _<T::RuntimeOrigin>(origin, registry_id.clone(), delegate.clone())
verify {
    // assert_eq!(authorization.registry_id, registry_id, "Registry ID should match");
    // assert_eq!(authorization.delegate, delegate, "Delegate should match");
     let authorization_id = Ss58Identifier::to_authorization_id(
        &(<T as frame_system::Config>::Hashing::hash(
            &[&registry_id.encode()[..], &delegate.encode()[..], &did.encode()[..]].concat()[..],
        )).encode()[..]
    ).expect("Authorization ID should be generated");

    let authorization = <Authorizations<T>>::get(&authorization_id).expect("Authorization should exist");

    assert_last_event::<T>(Event::AddAuthorization {
        registry: registry_id,
        authorization: authorization_id,
        delegate: delegate,
    }.into());
}

add_delegate {
    let l in 1 .. MAX_REGISTRY_SIZE;

    let caller: T::AccountId = account("caller", 0, SEED);
    let did: T::RegistryCreatorId = account("did", 0, SEED);
    let delegate: T::RegistryCreatorId = account("delegate", 0, SEED);

    let origin = <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());

    let raw_registry: Vec<u8> = (0u8..u8::MAX).cycle().take(T::MaxEncodedRegistryLength::get().try_into().unwrap()).collect();
    let registry: InputRegistryOf::<T> = BoundedVec::try_from(raw_registry)
        .expect("Test Registry should fit into the expected input length of the test runtime");
    let digest = <T as frame_system::Config>::Hashing::hash(&registry[..]);
    let id_digest = <T as frame_system::Config>::Hashing::hash(
        &[&registry.encode()[..], &did.encode()[..]].concat()[..],
    );
    let registry_id = Ss58Identifier::to_registry_id(&(id_digest).encode()[..]).unwrap();

    Pallet::<T>::create(origin.clone(), registry, None).expect("Should create a registry entry");
    let id_digest = <T as frame_system::Config>::Hashing::hash(
        &[&registry_id.encode()[..], &delegate.encode()[..], &did.encode()[..]].concat()[..],
    );
    let authorization_id = Ss58Identifier::to_authorization_id(&id_digest.encode()[..])
        .map_err(|_| Error::<T>::InvalidIdentifierLength).unwrap();

}: _<T::RuntimeOrigin>(origin, registry_id.clone(), delegate.clone())
verify {
    assert_last_event::<T>(Event::AddAuthorization {
        registry: registry_id,
        authorization: authorization_id,
        delegate,
    }.into());
}

remove_delegate {
    let l in 1 .. MAX_REGISTRY_SIZE;

    let caller: T::AccountId = account("caller", 0, SEED);
    let did: T::RegistryCreatorId = account("did", 0, SEED);
    let delegate: T::RegistryCreatorId = account("delegate", 0, SEED);

    let origin = <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());

    let raw_registry: Vec<u8> = (0u8..u8::MAX).cycle().take(T::MaxEncodedRegistryLength::get().try_into().unwrap()).collect();
    let registry: InputRegistryOf::<T> = BoundedVec::try_from(raw_registry)
        .expect("Test Registry should fit into the expected input length of the test runtime");
    let digest = <T as frame_system::Config>::Hashing::hash(&registry[..]);
    let id_digest = <T as frame_system::Config>::Hashing::hash(
        &[&registry.encode()[..], &did.encode()[..]].concat()[..],
    );
    let registry_id = Ss58Identifier::to_registry_id(&(id_digest).encode()[..]).unwrap();

    Pallet::<T>::create(origin.clone(), registry, None).expect("Should create a registry entry");
    Pallet::<T>::add_admin_delegate(origin.clone(), registry_id.clone(), delegate.clone()).expect("Should add admin delegate");

    let authorization_id = {
        let id_digest = <T as frame_system::Config>::Hashing::hash(
            &[&registry_id.encode()[..], &delegate.encode()[..], &did.encode()[..]].concat()[..],
        );
        Ss58Identifier::to_authorization_id(&id_digest.encode()[..]).unwrap()
    };

}: _<T::RuntimeOrigin>(origin, registry_id.clone(), authorization_id.clone())
verify {
    assert_last_event::<T>(Event::RemoveAuthorization { registry: registry_id, authorization: authorization_id }.into());
}

}


impl_benchmark_test_suite!(
    Pallet, 
    crate::mock::new_test_ext(),
    crate::mock::Test
);
