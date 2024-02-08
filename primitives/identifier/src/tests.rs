use super::*;
use crate::mock::*;
use codec::Encode;
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_err, assert_ok, error::BadOrigin};
use frame_system::RawOrigin;
use sp_runtime::{traits::Hash, AccountId32};
use sp_std::prelude::*;

//use identifier::{
//	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
//	EventEntryOf,
//};
//pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};

#[test]
fn creating_a_invalid_identifier_length_should_fail() {
	let space1 = [2u8; 1].to_vec();
	let space2 = [2u8; 50].to_vec();
	let space3 = [2u8; 40].to_vec();

	new_test_ext().execute_with(|| {
		assert_err!(
			Ss58Identifier::create_identifier(&(space1).encode()[..], IdentifierType::Space),
			IdentifierError::InvalidIdentifierLength
		);
		assert_err!(
			Ss58Identifier::create_identifier(&(space2).encode()[..], IdentifierType::Space),
			IdentifierError::InvalidIdentifierLength
		);
		assert_ok!(Ss58Identifier::create_identifier(
			&(space3).encode()[..],
			IdentifierType::Space
		));
	});
}
