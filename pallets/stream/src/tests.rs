use super::*;
use crate::mock::*;
use codec::Encode;
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_err, assert_noop, assert_ok, BoundedVec};
use pallet_registry::{InputRegistryOf, RegistryHashOf};
use pallet_schema::{InputSchemaOf, SchemaHashOf};
use sp_core::H256;
use sp_runtime::{traits::Hash, AccountId32};
use sp_std::prelude::*;

const DEFAULT_STREAM_HASH_SEED: u64 = 1u64;
const ALTERNATIVE_STREAM_HASH_SEED: u64 = 2u64;

pub fn get_stream_hash<T>(default: bool) -> StreamHashOf<T>
where
	T: Config,
	T::Hash: From<H256>,
{
	if default {
		H256::from_low_u64_be(DEFAULT_STREAM_HASH_SEED).into()
	} else {
		H256::from_low_u64_be(ALTERNATIVE_STREAM_HASH_SEED).into()
	}
}

//Function to generate stream ID

pub fn generate_stream_id<T: Config>(digest: &StreamHashOf<T>) -> StreamIdOf {
	Ss58Identifier::to_stream_id(&(digest).encode()[..]).unwrap()
}

//Function to generate Schema ID
pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
	Ss58Identifier::to_schema_id(&(digest).encode()[..]).unwrap()
}

//function to generate registry ID
pub fn generate_registry_id<T: Config>(digest: &RegistryHashOf<T>) -> RegistryIdOf {
	Ss58Identifier::to_registry_id(&(digest).encode()[..]).unwrap()
}


pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
pub(crate) const DID_01: SubjectId = SubjectId(AccountId32::new([2u8; 32]));
pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);
pub(crate) const ACCOUNT_01: AccountId = AccountId::new([2u8; 32]);

//Test function for creating the stream

#[test]
fn create_stream_should_succeed() {
    let creator = DID_00;
	let author = ACCOUNT_00;
    let delegate = DID_01;

    //Creation of Stream Digest
    let stream = vec![0u8;32];
	let stream_digest = <Test as frame_system::Config>::Hashing::hash(&stream[..]);
    //End of stream Digest 

    //creation of schema ID
    let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let schema_digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);
    //End of schema ID

    //creation of authorization ID
    let raw_registry = [9u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);


    let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

    //End of authorization ID

    //testing of function 
    new_test_ext().execute_with(|| {
		//Creating a registry
		assert_err!(Stream::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			stream_digest,
			authorization_id,
            Some(schema_id)
		),pallet_registry::Error::<Test>::AuthorizationNotFound);
	});


}

