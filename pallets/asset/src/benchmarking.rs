#![cfg(feature = "runtime-benchmarks")]

use super::*;
use codec::Encode;
use cord_utilities::traits::GenerateBenchmarkOrigin;
use frame_benchmarking::{account, benchmarks};
use frame_support::sp_runtime::traits::Hash;
use frame_system::RawOrigin;

use identifier::{IdentifierType, Ss58Identifier};
use pallet_chain_space::{SpaceCodeOf, SpaceIdOf};
use sp_runtime::BoundedVec;

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

pub fn generate_asset_id<T: Config>(digest: &SpaceCodeOf<T>) -> AssetIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Asset).unwrap()
}

pub fn generate_asset_instance_id<T: Config>(digest: &SpaceCodeOf<T>) -> AssetInstanceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::AssetInstance)
		.unwrap()
}

const SEED: u32 = 0;

benchmarks! {
		where_clause {
			where
			<T as Config>::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::SpaceCreatorId>,
		}

		create {
			let caller: T::AccountId = account("caller", 0, SEED);
			let did: T::SpaceCreatorId = account("did", 0, SEED);

			let asset_desc = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_tag = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_meta = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_qty = 10;
			let asset_value = 10;
			let asset_type = AssetTypeOf::MF;

			let raw_space = [56u8; 256].to_vec();
			let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
			let space_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			);

			let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_id_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]]
					.concat()[..],
			);

			let auth_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			);
			let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

			let entry = AssetInputEntryOf::<T> {
				asset_desc,
				asset_qty,
				asset_type,
				asset_value,
				asset_tag,
				asset_meta,
			};

			let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());
			let capacity = 5u64;

			let digest = <T as frame_system::Config>::Hashing::hash(
				&[&entry.encode()[..]].concat()[..],
			);

			let create_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
			);

			let asset_id: Ss58Identifier = generate_asset_id::<T>(&create_id_digest);

			let chain_space_origin = RawOrigin::Root.into();

			pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
			pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity).expect("Approval should not fail.");

		}: _<T::RuntimeOrigin>(origin, entry, digest, authorization_id)
		verify {
			assert_last_event::<T>(Event::Create { identifier: asset_id, issuer: did.clone() }.into());
		}

		issue {
			let caller: T::AccountId = account("caller", 0, SEED);
			let did: T::SpaceCreatorId = account("did", 0, SEED);

			let asset_desc = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_tag = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_meta = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_qty = 10;
			let asset_value = 10;
			let asset_type = AssetTypeOf::MF;

			let raw_space = [56u8; 256].to_vec();
			let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
			let space_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			);

			let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_id_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]]
					.concat()[..],
			);

			let auth_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			);
			let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

			let entry = AssetInputEntryOf::<T> {
				asset_desc,
				asset_qty,
				asset_type,
				asset_value,
				asset_tag,
				asset_meta,
			};

			let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());
			let capacity = 5u64;

			let digest = <T as frame_system::Config>::Hashing::hash(
				&[&entry.encode()[..]].concat()[..],
			);

			let create_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
			);

			let asset_id: Ss58Identifier = generate_asset_id::<T>(&create_id_digest);

			let chain_space_origin = RawOrigin::Root.into();

			let issue_entry = AssetIssuanceEntryOf::<T> {
				asset_id: asset_id.clone(),
				asset_owner: did.clone(),
				asset_issuance_qty: Some(10),
			};

			let issue_entry_digest = <T as frame_system::Config>::Hashing::hash(
				&[&issue_entry.encode()[..]].concat()[..],
			);

			let issue_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&asset_id.encode()[..], &did.encode()[..], &space_id.encode()[..], &did.encode()[..], &issue_entry_digest.encode()[..]].concat()[..],
			);

			let instance_id = generate_asset_instance_id::<T>(&issue_id_digest);

			pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
			pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity).expect("Approval should not fail.");
			Pallet::<T>::create(origin.clone(), entry, digest, authorization_id.clone())?;

		}: _<T::RuntimeOrigin>(origin, issue_entry, issue_entry_digest, authorization_id)
		verify {
			assert_last_event::<T>(Event::Issue { identifier: asset_id, instance: instance_id }.into());
		}

		transfer {
			let caller: T::AccountId = account("caller", 0, SEED);
			let did: T::SpaceCreatorId = account("did", 0, SEED);

			let did_transfer: T::SpaceCreatorId = account("did", 1, SEED);

			let asset_desc = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_tag = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_meta = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_qty = 10;
			let asset_value = 10;
			let asset_type = AssetTypeOf::MF;

			let raw_space = [56u8; 256].to_vec();
			let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
			let space_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			);

			let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_id_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]]
					.concat()[..],
			);

			let auth_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			);
			let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

			let entry = AssetInputEntryOf::<T> {
				asset_desc,
				asset_qty,
				asset_type,
				asset_value,
				asset_tag,
				asset_meta,
			};

			let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());
			let capacity = 5u64;

			let digest = <T as frame_system::Config>::Hashing::hash(
				&[&entry.encode()[..]].concat()[..],
			);

			let create_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
			);

			let asset_id: Ss58Identifier = generate_asset_id::<T>(&create_id_digest);

			let chain_space_origin = RawOrigin::Root.into();

			let issue_entry = AssetIssuanceEntryOf::<T> {
				asset_id: asset_id.clone(),
				asset_owner: did.clone(),
				asset_issuance_qty: Some(10),
			};

			let issue_entry_digest = <T as frame_system::Config>::Hashing::hash(
				&[&issue_entry.encode()[..]].concat()[..],
			);

			let issue_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&asset_id.encode()[..], &did.encode()[..], &space_id.encode()[..], &did.encode()[..], &issue_entry_digest.encode()[..]].concat()[..],
			);

			let instance_id = generate_asset_instance_id::<T>(&issue_id_digest);

			let transfer_entry = AssetTransferEntryOf::<T> {
				asset_id: asset_id.clone(),
				asset_instance_id: instance_id.clone(),
				asset_owner: did.clone(),
				new_asset_owner: did_transfer.clone(),
			};

			let transfer_entry_digest = <T as frame_system::Config>::Hashing::hash(
				&[&transfer_entry.encode()[..]].concat()[..],
			);

			pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
			pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity).expect("Approval should not fail.");
			Pallet::<T>::create(origin.clone(), entry, digest, authorization_id.clone())?;
			Pallet::<T>::issue(origin.clone(), issue_entry, issue_entry_digest, authorization_id)?;

		}: _<T::RuntimeOrigin>(origin, transfer_entry, transfer_entry_digest)
		verify {
			assert_last_event::<T>(Event::Transfer { identifier: asset_id, instance: instance_id, from: did.clone(), to: did_transfer.clone() }.into());
		}

		status_change {
			let caller: T::AccountId = account("caller", 0, SEED);
			let did: T::SpaceCreatorId = account("did", 0, SEED);

			let asset_desc = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_tag = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_meta = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_qty = 10;
			let asset_value = 10;
			let asset_type = AssetTypeOf::MF;

			let raw_space = [56u8; 256].to_vec();
			let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
			let space_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
			);

			let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_id_digest.encode()[..], &space_id.encode()[..], &did.encode()[..]]
					.concat()[..],
			);

			let auth_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			);
			let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

			let entry = AssetInputEntryOf::<T> {
				asset_desc,
				asset_qty,
				asset_type,
				asset_value,
				asset_tag,
				asset_meta,
			};

			let origin =  <T as Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());
			let capacity = 5u64;

			let digest = <T as frame_system::Config>::Hashing::hash(
				&[&entry.encode()[..]].concat()[..],
			);

			let create_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
			);

			let asset_id: Ss58Identifier = generate_asset_id::<T>(&create_id_digest);

			let chain_space_origin = RawOrigin::Root.into();

			let issue_entry = AssetIssuanceEntryOf::<T> {
				asset_id: asset_id.clone(),
				asset_owner: did.clone(),
				asset_issuance_qty: Some(10),
			};

			let issue_entry_digest = <T as frame_system::Config>::Hashing::hash(
				&[&issue_entry.encode()[..]].concat()[..],
			);

			let issue_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&asset_id.encode()[..], &did.encode()[..], &space_id.encode()[..], &did.encode()[..], &issue_entry_digest.encode()[..]].concat()[..],
			);

			let instance_id = generate_asset_instance_id::<T>(&issue_id_digest);

			let new_status = AssetStatusOf::EXPIRED;

			pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
			pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity).expect("Approval should not fail.");
			Pallet::<T>::create(origin.clone(), entry, digest, authorization_id.clone())?;
			Pallet::<T>::issue(origin.clone(), issue_entry, issue_entry_digest, authorization_id)?;

		}: _<T::RuntimeOrigin>(origin, asset_id.clone(), Some(instance_id.clone()), new_status.clone())
		verify {
			assert_last_event::<T>(Event::StatusChange { identifier: asset_id.clone(), instance: Some(instance_id.clone()), status: new_status.clone() }.into());
		}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
