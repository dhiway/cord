use crate::{mock::*, Error, Event, LabelOf, NameRecordOf, SaltOf, LABEL_POLICY_VERSION};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use sp_core::H256;

fn label(value: &[u8]) -> LabelOf<Test> {
	BoundedVec::try_from(value.to_vec()).expect("bounded test label")
}

fn salt(value: &[u8]) -> SaltOf<Test> {
	BoundedVec::try_from(value.to_vec()).expect("bounded test salt")
}

fn commit_and_register(owner: u64, parent: Option<H256>, raw: &[u8]) -> H256 {
	let name_label = label(raw);
	let salt = salt(b"secret");
	let commitment = Dotns::registration_commitment(&owner, parent, &name_label, &salt);
	assert_ok!(Dotns::commit(RuntimeOrigin::signed(owner), commitment));
	run_to(System::block_number() + 2);
	assert_ok!(Dotns::register(RuntimeOrigin::signed(owner), parent, name_label.clone(), salt));
	Dotns::derive_name_id(parent, &name_label)
}

#[test]
fn label_policy_is_ascii_lowercase_and_versioned() {
	new_test_ext().execute_with(|| {
		assert_eq!(Dotns::label_policy_version(), LABEL_POLICY_VERSION);
		assert_ok!(Dotns::validate_label(b"alice-42".to_vec()));
		for invalid in
			[b"Alice".as_slice(), b"-alice", b"alice-", b"alice_name", &[0xe2, 0x98, 0x83]]
		{
			assert!(matches!(
				Dotns::validate_label(invalid.to_vec()),
				Err(Error::<Test>::InvalidLabel)
			));
		}
	});
}

#[test]
fn commit_reveal_binds_owner_and_age() {
	new_test_ext().execute_with(|| {
		let label = label(b"alice");
		let salt = salt(b"secret");
		let commitment = Dotns::registration_commitment(&1, None, &label, &salt);
		assert_ok!(Dotns::commit(RuntimeOrigin::signed(1), commitment));
		assert_noop!(
			Dotns::register(RuntimeOrigin::signed(1), None, label.clone(), salt.clone()),
			Error::<Test>::CommitmentTooYoung
		);
		run_to(3);
		assert_noop!(
			Dotns::register(RuntimeOrigin::signed(2), None, label.clone(), salt.clone()),
			Error::<Test>::CommitmentNotFound
		);
		assert_ok!(Dotns::register(RuntimeOrigin::signed(1), None, label.clone(), salt));
		let name = Dotns::derive_name_id(None, &label);
		let record: NameRecordOf<Test> = Dotns::name_record(name).expect("registered");
		assert_eq!(record.owner, 1);
		assert_eq!(record.parent, None);
		assert_eq!(record.depth, 0);
		assert_eq!(record.expires_at, 103);
		assert_eq!(crate::CommitmentCount::<Test>::get(1), 0);
	});
}

#[test]
fn register_rejects_empty_salt_even_when_a_commitment_exists() {
	new_test_ext().execute_with(|| {
		let label = label(b"alice");
		let empty = salt(b"");
		let commitment = Dotns::registration_commitment(&1, None, &label, &empty);
		assert_ok!(Dotns::commit(RuntimeOrigin::signed(1), commitment));
		run_to(3);
		assert_noop!(
			Dotns::register(RuntimeOrigin::signed(1), None, label, empty),
			Error::<Test>::InvalidSalt
		);
	});
}

#[test]
fn subname_requires_parent_authority_and_is_expiry_bounded() {
	new_test_ext().execute_with(|| {
		let parent = commit_and_register(1, None, b"alice");
		let child_label = label(b"docs");
		let child_salt = salt(b"secret");
		let attacker_commitment =
			Dotns::registration_commitment(&2, Some(parent), &child_label, &child_salt);
		assert_ok!(Dotns::commit(RuntimeOrigin::signed(2), attacker_commitment));
		run_to(System::block_number() + 2);
		assert_noop!(
			Dotns::register(
				RuntimeOrigin::signed(2),
				Some(parent),
				child_label.clone(),
				child_salt
			),
			Error::<Test>::NotAuthorized
		);

		let child = commit_and_register(1, Some(parent), b"docs");
		let record = Dotns::name_record(child).expect("child registered");
		assert_eq!(record.depth, 1);
		assert!(record.expires_at <= Dotns::name_record(parent).unwrap().expires_at);
	});
}

#[test]
fn ownership_controllers_resolvers_and_primary_are_native() {
	new_test_ext().execute_with(|| {
		let name = commit_and_register(1, None, b"alice");
		assert_ok!(Dotns::add_controller(RuntimeOrigin::signed(1), name, 2));
		let address = BoundedVec::try_from(b"cord:account:alice".to_vec()).unwrap();
		assert_ok!(Dotns::set_address(RuntimeOrigin::signed(2), name, Some(address)));
		assert_ok!(Dotns::set_subject(RuntimeOrigin::signed(2), name, Some(KNOWN_SUBJECT)));
		assert_ok!(Dotns::set_attestation(RuntimeOrigin::signed(2), name, Some(LIVE_ATTESTATION)));
		assert_ok!(Dotns::set_content(RuntimeOrigin::signed(2), name, Some(H256::repeat_byte(7))));
		assert_ok!(Dotns::set_primary_name(RuntimeOrigin::signed(1), Some(name)));
		assert_eq!(Dotns::primary_name(&1), Some(name));
		assert_ok!(Dotns::transfer(RuntimeOrigin::signed(1), name, 3));
		assert_eq!(Dotns::primary_name(&1), None);
		assert!(Dotns::controllers(name).is_empty());
		assert_eq!(Dotns::name_record(name).unwrap().owner, 3);
	});
}

#[test]
fn subject_and_attestation_references_are_validated_before_mutation() {
	new_test_ext().execute_with(|| {
		let name = commit_and_register(1, None, b"alice");
		let unknown_subject = H256::repeat_byte(43);
		let non_live_attestation = H256::repeat_byte(44);

		assert_noop!(
			Dotns::set_subject(RuntimeOrigin::signed(1), name, Some(unknown_subject)),
			Error::<Test>::InvalidSubjectReference
		);
		assert_eq!(Dotns::name_record(name).unwrap().subject, None);

		assert_ok!(Dotns::set_subject(RuntimeOrigin::signed(1), name, Some(KNOWN_SUBJECT)));
		assert_noop!(
			Dotns::set_attestation(RuntimeOrigin::signed(1), name, Some(non_live_attestation)),
			Error::<Test>::InvalidAttestationReference
		);
		let record = Dotns::name_record(name).unwrap();
		assert_eq!(record.subject, Some(KNOWN_SUBJECT));
		assert_eq!(record.attestation, None);

		assert_ok!(Dotns::set_attestation(RuntimeOrigin::signed(1), name, Some(LIVE_ATTESTATION)));
		assert_ok!(Dotns::set_subject(RuntimeOrigin::signed(1), name, None));
		assert_ok!(Dotns::set_attestation(RuntimeOrigin::signed(1), name, None));
		let record = Dotns::name_record(name).unwrap();
		assert_eq!(record.subject, None);
		assert_eq!(record.attestation, None);
	});
}

#[test]
fn reservations_protection_and_pause_fail_closed() {
	new_test_ext().execute_with(|| {
		let protected = label(b"admin");
		assert_ok!(Dotns::set_label_protection(RuntimeOrigin::root(), protected.clone(), true));
		let salt = salt(b"secret");
		let commitment = Dotns::registration_commitment(&1, None, &protected, &salt);
		assert_ok!(Dotns::commit(RuntimeOrigin::signed(1), commitment));
		run_to(3);
		assert_noop!(
			Dotns::register(RuntimeOrigin::signed(1), None, protected, salt),
			Error::<Test>::ProtectedLabel
		);
		assert_ok!(Dotns::set_paused(RuntimeOrigin::root(), true));
		assert_noop!(
			Dotns::commit(RuntimeOrigin::signed(1), H256::repeat_byte(1)),
			Error::<Test>::Paused
		);
		assert_ok!(Dotns::set_paused(RuntimeOrigin::root(), false));
		System::assert_last_event(Event::<Test>::PauseSet { paused: false }.into());
	});
}

#[test]
fn genesis_reservations_and_scoped_registrar_governance_are_enforced() {
	new_test_ext_with_dotns(vec![2], vec![(label(b"system"), Some(1))]).execute_with(|| {
		let reserved_label = label(b"system");
		let registration_salt = salt(b"secret");
		let attacker_commitment =
			Dotns::registration_commitment(&3, None, &reserved_label, &registration_salt);
		assert_ok!(Dotns::commit(RuntimeOrigin::signed(3), attacker_commitment));
		run_to(3);
		assert_noop!(
			Dotns::register(
				RuntimeOrigin::signed(3),
				None,
				reserved_label.clone(),
				registration_salt.clone()
			),
			Error::<Test>::ReservedName
		);

		let owner_commitment =
			Dotns::registration_commitment(&1, None, &reserved_label, &registration_salt);
		assert_ok!(Dotns::commit(RuntimeOrigin::signed(1), owner_commitment));
		run_to(5);
		assert_ok!(Dotns::register(
			RuntimeOrigin::signed(1),
			None,
			reserved_label,
			registration_salt,
		));

		assert_noop!(
			Dotns::reserve_name(RuntimeOrigin::signed(3), None, label(b"corp"), None, None),
			Error::<Test>::NotRegistrar
		);
		assert_ok!(Dotns::reserve_name(
			RuntimeOrigin::signed(2),
			None,
			label(b"corp"),
			Some(1),
			None,
		));
		assert_ok!(Dotns::set_registrar(RuntimeOrigin::root(), 2, false));
		assert_noop!(
			Dotns::set_label_protection(RuntimeOrigin::signed(2), label(b"blocked"), true),
			Error::<Test>::NotRegistrar
		);
	});
}

#[test]
fn genesis_rejects_invalid_or_duplicate_reservation_labels() {
	assert!(std::panic::catch_unwind(|| {
		new_test_ext_with_dotns(Vec::new(), vec![(label(b"Alice"), None)]);
	})
	.is_err());
	assert!(std::panic::catch_unwind(|| {
		new_test_ext_with_dotns(
			Vec::new(),
			vec![(label(b"system"), None), (label(b"system"), Some(1))],
		);
	})
	.is_err());
}
