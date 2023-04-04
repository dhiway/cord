use crate::{mock::*,Error,ExtrinsicAuthors};
use frame_support::{assert_ok, assert_noop};



#[test]
fn add() {
	ExtBuilder::default().build().execute_with(|| {
		let test_mock_authors: Vec<CordAccountOf<Test>> = vec![TEST_AUTHOR2,TEST_AUTHOR3];
		assert_ok!(Authorship::add(
			RuntimeOrigin::signed(TEST_AUTHOR2),
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
			RuntimeOrigin::signed(TEST_AUTHOR2),
			test_mock_authors.clone()
		));
		assert_eq!(System::event_count(), 1);
	})
}