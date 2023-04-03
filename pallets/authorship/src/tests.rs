use crate::{mock::*,Error};
use cord_utilities::mock::mock_origin;
use frame_support::{assert_ok, assert_noop};


#[test]
fn add() {
	let test_mock_authors: Vec<CordAccountOf<Test>> = vec![TEST_AUTHOR2,TEST_AUTHOR3];
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(2);
		assert_ok!(Authorship::add(
			mock_origin::DoubleOrigin(TEST_AUTHOR2, DID_02).into(),
			test_mock_authors.clone()
		));
		 assert_eq!(System::event_count(), 1);
	})
}

#[test]
fn remove() {
	let test_mock_authors: Vec<CordAccountOf<Test>> = vec![TEST_AUTHOR2,TEST_AUTHOR3];
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(2);
		assert_ok!(Authorship::remove(
			mock_origin::DoubleOrigin(TEST_AUTHOR2, DID_02).into(),
			test_mock_authors.clone()
		));
		assert_eq!(System::event_count(), 1);
	})
}

#[test]
fn max_authority() {
	let test_mock_authors: Vec<CordAccountOf<Test>> = vec![TEST_AUTHOR2,TEST_AUTHOR3];
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(2);
		for i in 0..7 {
			assert_ok!(Authorship::add(
				mock_origin::DoubleOrigin(TEST_AUTHOR2, DID_02).into(),
				test_mock_authors.clone()
			));
		}
		assert_noop!(Authorship::add(
			mock_origin::DoubleOrigin(TEST_AUTHOR2, DID_02).into(),
			test_mock_authors.clone()
		),Error::<Test>::TooManyAuthorityProposals);
	})
}
