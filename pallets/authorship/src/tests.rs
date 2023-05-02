use crate::{mock::*, Error, ExtrinsicAuthors};
use cord_primitives::AccountId;
use frame_support::{assert_err, assert_ok, error::BadOrigin};
use frame_system::RawOrigin;

#[test]
fn add_author() {
	new_test_ext().execute_with(|| {
		// Adding Author if author doesnot exist in the account
		assert_ok!(Authorship::add(
			RawOrigin::Root.into(),
			vec![AccountId::new([11u8; 32]), AccountId::new([12u8; 32])]
		));

		//This ensures that the account was successfully added to the ExtrinsicAuthors
		assert_eq!(ExtrinsicAuthors::<Test>::get(AccountId::new([11u8; 32])), Some(()));

		//This code can confirm that the previous operation completed as expected.
		let events = System::events();
		assert_eq!(events.len(), 1);

		//if author already exist in account it should throw error
		// 'AuthorAccountAlreadyExists'
		assert_err!(
			Authorship::add(
				RawOrigin::Root.into(),
				vec![AccountId::new([10u8; 32]), AccountId::new([3u8; 32]),],
			),
			Error::<Test>::AuthorAccountAlreadyExists
		);
	});
}

#[test]
fn remove_auhtor() {
	new_test_ext().execute_with(|| {

		//Check if author does not exist and we try to remove author
		assert_err!(
			Authorship::remove(RawOrigin::Root.into(), vec![AccountId::new([100u8; 32])],),
			Error::<Test>::AuthorAccountNotFound
		);

		//first adding the account. Inserting the Author for removing it in Extrinsic
		// author
		assert_ok!(Authorship::add(
			RawOrigin::Root.into(),
			vec![AccountId::new([11u8; 32]), AccountId::new([12u8; 32])]
		));


		//Now Removing the previously inserted above account id
		assert_ok!(Authorship::remove(RawOrigin::Root.into(), vec![AccountId::new([12u8; 32])]));
	});
}

#[test]
fn author_other_than_root() {
	//Check if any other author trying to remove the account apart from root
	new_test_ext().execute_with(|| {
		<ExtrinsicAuthors<Test>>::insert(AccountId::new([12u8; 32]), ());
		assert_err!(
			Authorship::remove(
				RuntimeOrigin::signed(AccountId::new([12u8; 32])),
				vec![AccountId::new([12u8; 32])]
			),
			BadOrigin
		);
	})
}

#[test]
fn max_authority_proposal() {

	//Check for max authority proposal by defalt in test is set to 5 if we try to add more than 5 we should get errro
	//Too many authority proposals
	new_test_ext().execute_with(|| {
		assert_err!(
			Authorship::add(
				RawOrigin::Root.into(),
				vec![
					AccountId::new([2u8; 32]),
					AccountId::new([3u8; 32]),
					AccountId::new([4u8; 32]),
					AccountId::new([5u8; 32]),
					AccountId::new([6u8; 32]),
					AccountId::new([7u8; 32])
				],
			),
			Error::<Test>::TooManyAuthorityProposals
		);
	})
}
