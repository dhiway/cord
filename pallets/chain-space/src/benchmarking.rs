#![cfg(feature = "runtime-benchmarks")]

use super::*;
use codec::Encode;
use cord_utilities::traits::GenerateBenchmarkOrigin;
use frame_benchmarking::{account, benchmarks};
use frame_support::sp_runtime::traits::Hash;
use frame_system::RawOrigin;
use identifier::{IdentifierType, Ss58Identifier};

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

pub fn generate_space_id<T: Config>(digest: &SpaceCodeOf<T>) -> SpaceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Space).unwrap()
}

pub fn generate_authorization_id<T: Config>(digest: &SpaceCodeOf<T>) -> AuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Authorization)
		.unwrap()
}

const SEED: u32 = 0;

benchmarks! {
		where_clause {
			where
			<T as Config>::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::SpaceCreatorId>,
			// T::ChainSpaceOrigin: EnsureOrigin<T::RuntimeOrigin>,
		}
		add_delegate {
			let caller: T::AccountId = account("caller", 0, SEED);
			let did: T::SpaceCreatorId = account("did", 0, SEED);
			let delegate_did: T::SpaceCreatorId = account("did", 1, SEED);
			let space = [2u8; 256].to_vec();
			let capacity = 5u64;

			let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			);
			let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			);
			let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			let delegate_id_digest = T::Hashing::hash(
				&[&space_id.encode()[..], &delegate_did.encode()[..], &did.encode()[..]].concat()[..],
			);
			let delegate_authorization_id = generate_authorization_id::<T>(&delegate_id_digest);

			let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());
			let chain_space_origin = RawOrigin::Root.into();


			Pallet::<T>::create(origin, space_digest )?;
			Pallet::<T>::approve(chain_space_origin, space_id.clone(), capacity ).expect("Approval should not fail.");

			let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did);
		}: _<T::RuntimeOrigin>(origin, space_id.clone(), delegate_did.clone(), authorization_id  )
		verify {
			assert_last_event::<T>(Event::Authorization { space: space_id, authorization: delegate_authorization_id, delegate: delegate_did,  }.into());
		}

		add_admin_delegate {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let delegate_did: T::SpaceCreatorId = account("did", 1, SEED);
			 let space = [2u8; 256].to_vec();
			 let capacity = 5u64;

			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let delegate_id_digest = T::Hashing::hash(
				 &[&space_id.encode()[..], &delegate_did.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let delegate_authorization_id = generate_authorization_id::<T>(&delegate_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did);
			 let root_origin = RawOrigin::Root.into();

			 Pallet::<T>::create(origin.clone(), space_digest )?;
			 Pallet::<T>::approve(root_origin, space_id.clone(), capacity )?;

		}: _<T::RuntimeOrigin>(origin, space_id.clone(), delegate_did.clone(), authorization_id  )
		verify {
			 assert_last_event::<T>(Event::Authorization { space: space_id, authorization: delegate_authorization_id, delegate: delegate_did,  }.into());
		}

		add_delegator {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let delegate_did: T::SpaceCreatorId = account("did", 1, SEED);
			 let space = [2u8; 256].to_vec();
			 let capacity = 5u64;

			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let delegate_id_digest = T::Hashing::hash(
				 &[&space_id.encode()[..], &delegate_did.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let delegate_authorization_id = generate_authorization_id::<T>(&delegate_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did);
			 let root_origin = RawOrigin::Root.into();

			 Pallet::<T>::create(origin.clone(), space_digest )?;
			 Pallet::<T>::approve(root_origin, space_id.clone(), capacity )?;

		 }: _<T::RuntimeOrigin>(origin, space_id.clone(), delegate_did.clone(), authorization_id  )
		 verify {
			 assert_last_event::<T>(Event::Authorization { space: space_id, authorization: delegate_authorization_id, delegate: delegate_did,  }.into());
		 }

		remove_delegate {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let delegate_did: T::SpaceCreatorId = account("did", 1, SEED);
			 let capacity = 5u64;

			 let space = [2u8; 256].to_vec();
			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);

			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let delegate_id_digest = T::Hashing::hash(
				 &[&space_id.encode()[..], &delegate_did.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let delegate_authorization_id = generate_authorization_id::<T>(&delegate_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did);
			 let root_origin = RawOrigin::Root.into();

			 Pallet::<T>::create(origin.clone(), space_digest )?;
			 Pallet::<T>::approve(root_origin, space_id.clone(), capacity )?;
			 Pallet::<T>::add_delegate(origin.clone(), space_id.clone(), delegate_did, authorization_id.clone() )?;

		}: _<T::RuntimeOrigin>(origin, space_id.clone(), delegate_authorization_id.clone(), authorization_id)
		verify {
			 assert_last_event::<T>(Event::Deauthorization { space: space_id, authorization: delegate_authorization_id }.into());
		}

		 create {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);

			 let space = [2u8; 256].to_vec();
			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);

			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());

		 }: _<T::RuntimeOrigin>(origin, space_digest )
		 verify {
			 assert_last_event::<T>(Event::Create { space: space_id, creator: did, authorization: authorization_id }.into());
		 }

		 approve {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let space = [2u8; 256].to_vec();
			 let capacity = 5u64;

			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did);
			 let root_origin = RawOrigin::Root.into();

			 Pallet::<T>::create(origin, space_digest )?;

		 }: _<T::RuntimeOrigin>(root_origin, space_id.clone(), capacity)
		 verify {
			 assert_last_event::<T>(Event::Approve { space: space_id }.into());
		 }

		archive {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let space = [2u8; 256].to_vec();
			 let capacity = 5u64;

			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());

			 Pallet::<T>::create(origin.clone(), space_digest )?;
			 Pallet::<T>::approve(RawOrigin::Root.into(), space_id.clone(), capacity )?;

		 }: _<T::RuntimeOrigin>(origin, space_id.clone(), authorization_id )
		 verify {
			 assert_last_event::<T>(Event::Archive { space: space_id, authority: did, }.into());
		 }

		restore {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let space = [2u8; 256].to_vec();
			 let capacity = 5u64;

			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());

			 Pallet::<T>::create(origin.clone(), space_digest )?;
			 Pallet::<T>::approve(RawOrigin::Root.into(), space_id.clone(), capacity )?;
			 Pallet::<T>::archive(origin.clone(), space_id.clone(), authorization_id.clone() )?;

		 }: _<T::RuntimeOrigin>(origin, space_id.clone(), authorization_id )
		 verify {
			 assert_last_event::<T>(Event::Restore { space: space_id, authority: did, }.into());
		 }

		update_transaction_capacity {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let space = [2u8; 256].to_vec();
			 let capacity = 5u64;
			 let new_capacity = 10u64;

			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did);
			 let root_origin = RawOrigin::Root;

			 Pallet::<T>::create(origin, space_digest )?;
			 Pallet::<T>::approve(root_origin.clone().into(), space_id.clone(), capacity )?;

		 }: _<T::RuntimeOrigin>(root_origin.into(), space_id.clone(), new_capacity)
		 verify {
			 assert_last_event::<T>(Event::UpdateCapacity { space: space_id }.into());
		 }

		reset_transaction_count {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let space = [2u8; 256].to_vec();
			 let capacity = 5u64;

			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did);

			 Pallet::<T>::create(origin, space_digest )?;
			 Pallet::<T>::approve(RawOrigin::Root.into(), space_id.clone(), capacity )?;

		}: _<T::RuntimeOrigin>(RawOrigin::Root.into(), space_id.clone())
		verify {
			 assert_last_event::<T>(Event::ResetUsage { space: space_id }.into());
		}

		approval_revoke {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let space = [2u8; 256].to_vec();
			 let capacity = 5u64;

			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did);

			 Pallet::<T>::create(origin, space_digest )?;
			 Pallet::<T>::approve(RawOrigin::Root.into(), space_id.clone(), capacity )?;

		 }: _<T::RuntimeOrigin>(RawOrigin::Root.into(), space_id.clone())
		 verify {
			 assert_last_event::<T>(Event::ApprovalRevoke { space: space_id }.into());
		 }

		 approval_restore {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let space = [2u8; 256].to_vec();
			 let capacity = 5u64;

			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did);

			 Pallet::<T>::create(origin, space_digest )?;
			 Pallet::<T>::approve(RawOrigin::Root.into(), space_id.clone(), capacity )?;
			 Pallet::<T>::approval_revoke(RawOrigin::Root.into(), space_id.clone())?;

		 }: _<T::RuntimeOrigin>(RawOrigin::Root.into(), space_id.clone())
		 verify {
			 assert_last_event::<T>(Event::ApprovalRestore { space: space_id }.into());
		 }

		subspace_create {
			 let caller: T::AccountId = account("caller", 0, SEED);
			 let did: T::SpaceCreatorId = account("did", 0, SEED);
			 let space = [2u8; 256].to_vec();
			 let subspace = [5u8; 256].to_vec();
			 let capacity = 50u64;

			 let space_digest = <T as frame_system::Config>::Hashing::hash(&space.encode()[..]);
			 let subspace_digest = <T as frame_system::Config>::Hashing::hash(&subspace.encode()[..]);
			 let id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let space_id: SpaceIdOf = generate_space_id::<T>(&id_digest);

			 let sub_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&subspace_digest.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let subspace_id: SpaceIdOf = generate_space_id::<T>(&sub_id_digest);

			 let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				 &[&subspace_id.encode()[..], &did.encode()[..]].concat()[..],
			 );
			 let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			 let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());

			 Pallet::<T>::create(origin.clone(), space_digest )?;
			 Pallet::<T>::approve(RawOrigin::Root.into(), space_id.clone(), capacity )?;

		 }: _<T::RuntimeOrigin>(origin, subspace_digest, 10u64, space_id.clone())
		 verify {
			 assert_last_event::<T>(Event::Create { space: subspace_id, creator: did, authorization: authorization_id }.into());
		 }

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);

}
