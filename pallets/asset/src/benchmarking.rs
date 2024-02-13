#![cfg(feature = "runtime-benchmarks")]

use super::*;
use codec::Encode;
use cord_utilities::traits::GenerateBenchmarkOrigin;
use frame_benchmarking::{account, benchmarks};
use frame_support::sp_runtime::traits::Hash;
use frame_system::RawOrigin;

use frame_system::pallet_prelude::BlockNumberFor;
use pallet_chain_space::{SpaceCodeOf, SpaceIdOf};
//use sp_runtime::traits::Zero;
use sp_runtime::{traits::Zero, BoundedVec};

//use frame_system::EnsureOrigin;
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

pub fn generate_asset_id<T: Config>(digest: &SpaceCodeOf<T>) -> AssetIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Asset).unwrap()
}

const SEED: u32 = 0;

benchmarks! {
		//where_clause {
		//	where
		//	<T as Config>::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::SpaceCreatorId>,
			// T::ChainSpaceOrigin: EnsureOrigin<T::RuntimeOrigin>,
		//}
		where_clause {
			where
			<T as Config>::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::SpaceCreatorId>,
		}

		create {
			let caller: T::AccountId = account("caller", 0, SEED);
			let did: T::SpaceCreatorId = account("did", 0, SEED);

			//pub type AssetInputEntryOf<T> = AssetInputEntry<AssetDescriptionOf<T>, AssetTypeOf, AssetTagOf<T>, AssetMetadataOf<T>>
			let asset_desc = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			//let asset_type = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_tag = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_meta = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
			let asset_qty = 10;
			let asset_value = 10;
			let asset_type = AssetTypeOf::MF;
			let asset_status = AssetStatusOf::ACTIVE;

			let block_number = BlockNumberFor::<T>::zero();

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
			let identifier = generate_asset_id::<T>(&id_digest);

			let auth_digest = <T as frame_system::Config>::Hashing::hash(
				&[&space_id.encode()[..], &did.encode()[..]].concat()[..],
			);
			let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

			/*
			let entry = AssetInputEntryOf::<T> {
				asset_description,
				asset_type,
				asset_status,
				space_id,
				asset_tag,
				asset_metadata,
				block_number,
			};
			*/

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

			let digest = <Test as frame_system::Config>::Hashing::hash(
				&[&entry.encode()[..]].concat()[..],
			);

			let chain_space_origin = RawOrigin::Root.into();

			pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
			pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity ).expect("Approval should not fail.");

		}: _<T::RuntimeOrigin>(origin, entry, digest, authorization_id)
		verify {
			assert_last_event::<T>(Event::Create { identifier, issuer: did.clone() }.into());
		}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);

}
