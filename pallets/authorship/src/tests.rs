use crate::{mock::*,ExtrinsicAuthors,Error};
use cord_primitives::AccountId;
use frame_support::{assert_ok,assert_err, error::BadOrigin};
use frame_system::RawOrigin;



#[test]
fn add() {
	ExtBuilder::default().build().execute_with(|| {
		let test_mock_authors: Vec<CordAccountOf<Test>> = vec![TEST_AUTHOR2,TEST_AUTHOR3];
		assert_ok!(Authorship::add(
			RawOrigin::Root.into(),
			test_mock_authors.clone()
		));
		
		assert_eq!(ExtrinsicAuthors::<Test>::get(TEST_AUTHOR2.clone()), Some(()));
		let events = System::events();
        assert_eq!(events.len(), 1);
	})
}

#[test]
fn remove() {
	ExtBuilder::default().build().execute_with(|| {
		let test_mock_authors: Vec<CordAccountOf<Test>> = vec![TEST_AUTHOR2];
		<ExtrinsicAuthors<Test>>::insert(TEST_AUTHOR2, ());
		assert_ok!(Authorship::remove(
			RawOrigin::Root.into(),
			test_mock_authors.clone()
		));
		assert_eq!(System::event_count(), 1);
	})
}

#[test]
	fn author_other_than_root(){
		ExtBuilder::default().build().execute_with(|| {
			let test_mock_authors: Vec<CordAccountOf<Test>> = vec![TEST_AUTHOR2];
			<ExtrinsicAuthors<Test>>::insert(TEST_AUTHOR2, ());
			assert_err!(Authorship::remove(
				RuntimeOrigin::signed(TEST_AUTHOR2.clone()),
				test_mock_authors.clone()
			),BadOrigin);
		})
	}

	#[test]
	fn max_author(){
		ExtBuilder::default().build().
		execute_with(|| {
			// let test_mock_authors: Vec<CordAccountOf<Test>> = vec![TEST_AUTHOR2];
			<ExtrinsicAuthors<Test>>::insert(TEST_AUTHOR2, ());
			assert_err!(Authorship::add(
				RawOrigin::Root.into(),
				vec![AccountId::new([2u8; 32]),AccountId::new([3u8; 32]),AccountId::new([4u8; 32]),AccountId::new([5u8; 32]),AccountId::new([6u8; 32]),AccountId::new([7u8; 32])],
			),Error::<Test>::TooManyAuthorityProposals);
		})
	}	