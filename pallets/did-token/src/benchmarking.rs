#![cfg(feature = "runtime-benchmarks")]

use super::*;
use codec::Encode;
use cord_utilities::traits::GenerateBenchmarkOrigin;
use frame_benchmarking::{account, benchmarks};
use frame_support::sp_runtime::traits::Hash;
use frame_system::RawOrigin;
use identifier::{IdentifierType, Ss58Identifier};
use pallet_chain_space::SpaceCodeOf;

const SEED: u32 = 0;
const MAX_PAYLOAD_BYTE_LENGTH: u32 = 5 * 1024;

/// Generates a statement ID from a statement digest.
pub fn generate_statement_id<T: Config>(digest: &StatementDigestOf<T>) -> StatementIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Statement).unwrap()
}

/// Generates a space ID from a digest.
pub fn generate_space_id<T: Config>(digest: &SpaceCodeOf<T>) -> SpaceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Space).unwrap()
}

/// Generates an authorization ID from a digest.
pub fn generate_authorization_id<T: Config>(digest: &SpaceCodeOf<T>) -> AuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Authorization)
		.unwrap()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
	where_clause {
		where
		<T as Config>::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::SpaceCreatorId>,
	}
	register {

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::SpaceCreatorId = account("did", 0, SEED);
		let capacity = 3u64;

		let statement = [77u8; 32].to_vec();

		let statement_digest = <T as frame_system::Config>::Hashing::hash(&statement[..]);

		let raw_space = [56u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
		);

		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);
		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&statement_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]]
				.concat()[..],
		);

		let identifier = generate_statement_id::<T>(&id_digest);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity ).expect("Approval should not fail.");

	}: _<T::RuntimeOrigin>(origin, statement_digest, authorization_id, None)
	verify {
		assert_last_event::<T>(Event::Register { identifier, digest: statement_digest, author: did}.into());
	}

	update {

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::SpaceCreatorId = account("did", 0, SEED);
		let capacity = 5u64;

		let raw_space = [56u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
		);

		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

		let statement = [77u8; 32].to_vec();

		let statement_digest = <T as frame_system::Config>::Hashing::hash(&statement[..]);

		let statement_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&statement_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
		);

		let identifier = generate_statement_id::<T>(&statement_id_digest);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

		let statement_update = [12u8; 32].to_vec();
		let update_digest = <T as frame_system::Config>::Hashing::hash(&statement_update[..]);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity ).expect("Approval should not fail.");

		/* register the entry before update */
		let _ = Pallet::<T>::register(origin.clone(), statement_digest, authorization_id.clone(), None);

	}: _<T::RuntimeOrigin>(origin, identifier.clone(), update_digest, authorization_id)
	verify {
		assert_last_event::<T>(Event::Update { identifier, digest: update_digest, author: did}.into());
	}

	revoke {

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::SpaceCreatorId = account("did", 0, SEED);
		let capacity = 5u64;

		let raw_space = [56u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
		);
		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

		let statement = [77u8; 32].to_vec();
		let statement_digest = <T as frame_system::Config>::Hashing::hash(&statement[..]);
		let statement_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&statement_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
		);
		let identifier = generate_statement_id::<T>(&statement_id_digest);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity ).expect("Approval should not fail.");

		/* register the entry before update */
		let _ = Pallet::<T>::register(origin.clone(), statement_digest, authorization_id.clone(), None);

	}: _<T::RuntimeOrigin>(origin, identifier.clone(), authorization_id)
	verify {
		assert_last_event::<T>(Event::Revoke { identifier, author: did}.into());
	}

	restore {

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::SpaceCreatorId = account("did", 0, SEED);
		let capacity = 6u64;

		let raw_space = [56u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
		);
		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

		let statement = [77u8; 32].to_vec();
		let statement_digest = <T as frame_system::Config>::Hashing::hash(&statement[..]);
		let statement_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&statement_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
		);

		let identifier = generate_statement_id::<T>(&statement_id_digest);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);
		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity ).expect("Approval should not fail.");

		/* register the entry before update */
		let _ = Pallet::<T>::register(origin.clone(), statement_digest, authorization_id.clone(), None);
		let _ = Pallet::<T>::revoke(origin.clone(), identifier.clone(), authorization_id.clone());

	}: _<T::RuntimeOrigin>(origin, identifier.clone(), authorization_id)
	verify {
		assert_last_event::<T>(Event::Restore {identifier, author: did}.into());
	}

	remove {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::SpaceCreatorId = account("did", 0, SEED);
		let capacity = 5u64;

		let raw_space = [56u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
		);
		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

		let statement = [77u8; 32].to_vec();
		let statement_digest = <T as frame_system::Config>::Hashing::hash(&statement[..]);
		let statement_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&statement_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
		);

		let identifier = generate_statement_id::<T>(&statement_id_digest);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity ).expect("Approval should not fail.");

		/* register the entry before update */
		let _ = Pallet::<T>::register(origin.clone(), statement_digest, authorization_id.clone(), None);

	}: _<T::RuntimeOrigin>(origin, identifier.clone(), authorization_id)
	verify {
		assert_last_event::<T>(Event::Remove { identifier, author: did}.into());
	}

	register_batch {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::SpaceCreatorId = account("did", 0, SEED);
		let capacity = 3u64;

		let raw_space = [56u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
		);
		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

		let statement0 = [77u8; 32].to_vec();
		let statement1 = [87u8; 32].to_vec();
		let statement2 = [97u8; 32].to_vec();

		let statement_digest0 = <T as frame_system::Config>::Hashing::hash(&statement0[..]);
		let statement_digest1 = <T as frame_system::Config>::Hashing::hash(&statement1[..]);
		let statement_digest2 = <T as frame_system::Config>::Hashing::hash(&statement2[..]);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);
		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity ).expect("Approval should not fail.");

	}: _<T::RuntimeOrigin>(origin, vec![statement_digest0, statement_digest1, statement_digest2], authorization_id, None)
	verify {
		assert_last_event::<T>(Event::RegisterBatch { successful: 3, failed: 0, indices: [].to_vec(), author: did}.into());
	}

	add_presentation {

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::SpaceCreatorId = account("did", 0, SEED);
		let capacity = 3u64;

		let raw_space = [56u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
		);
		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

		let statement = [77u8; 32].to_vec();
		let statement_digest = <T as frame_system::Config>::Hashing::hash(&statement[..]);
		let statement_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&statement_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
		);

		let identifier = generate_statement_id::<T>(&statement_id_digest);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity ).expect("Approval should not fail.");

		/* register the entry before update */
		let _ = Pallet::<T>::register(origin.clone(), statement_digest, authorization_id.clone(), None);

	}: _<T::RuntimeOrigin>(origin, identifier.clone(), statement_digest, PresentationTypeOf::PDF, authorization_id)
	verify {
		assert_last_event::<T>(Event::PresentationAdded { identifier, digest: statement_digest, author: did}.into());
	}

	remove_presentation {

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::SpaceCreatorId = account("did", 0, SEED);
		let capacity = 4u64;

		let raw_space = [56u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
		);
		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

		let statement = [77u8; 32].to_vec();
		let statement_digest = <T as frame_system::Config>::Hashing::hash(&statement[..]);
		let statement_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&statement_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
		);

		let identifier = generate_statement_id::<T>(&statement_id_digest);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity ).expect("Approval should not fail.");

		/* register the entry before update */
		let _ = Pallet::<T>::register(origin.clone(), statement_digest, authorization_id.clone(), None);
		let _ = Pallet::<T>::add_presentation(origin.clone(), identifier.clone(), statement_digest, PresentationTypeOf::PDF, authorization_id.clone());
	}: _<T::RuntimeOrigin>(origin, identifier.clone(), statement_digest, authorization_id)
	verify {
		assert_last_event::<T>(Event::PresentationRemoved { identifier, digest: statement_digest, author: did}.into());
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);

}
