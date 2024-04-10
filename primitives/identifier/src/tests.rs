use super::*;
use crate::mock::*;
use codec::Encode;
use frame_support::{assert_err, assert_ok};

#[test]
fn creating_a_invalid_identifier_length_should_fail() {
	let space1 = [2u8; 1].to_vec();
	let space2 = [2u8; 50].to_vec();
	let space3 = [2u8; 30].to_vec();

	new_test_ext().execute_with(|| {
		assert_err!(
			Pallet::<Test>::create_identifier(&(space1).encode()[..]),
			IdentifierError::InvalidIdentifierLength
		);
		assert_err!(
			Pallet::<Test>::create_identifier(&(space2).encode()[..]),
			IdentifierError::InvalidIdentifierLength
		);
		assert_ok!(Pallet::<Test>::create_identifier(&(space3).encode()[..]));
	});
}
