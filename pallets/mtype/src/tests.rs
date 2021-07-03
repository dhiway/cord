// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

// derived from kilt project

//! #MARK Types: Handles #MARK Types,
//! testing #MARK Types.

use frame_support::{assert_noop, assert_ok};

use crate::{self as mtype, mock::*};

#[test]
fn check_successful_mtype_creation() {
	let owner = ALICE;

	let operation = generate_base_mtype_creation_details();

	let builder = ExtBuilder::default();

	let mut ext = builder.build(None);

	// Write MTYPE on chain
	ext.execute_with(|| {
		assert_ok!(Mtype::anchor(get_origin(owner.clone()), operation.hash));
	});

	// Verify the MTYPE has the right owner
	let stored_mtype_owner =
		ext.execute_with(|| Mtype::mtypes(&operation.hash).expect("MTYPE hash should be present on chain."));
	assert_eq!(stored_mtype_owner, owner);
}

#[test]
fn check_duplicate_mtype_creation() {
	let owner = ALICE;

	let operation = generate_base_mtype_creation_details();

	let builder = ExtBuilder::default().with_mtypes(vec![(operation.hash, owner.clone())]);

	let mut ext = builder.build(None);

	ext.execute_with(|| {
		assert_noop!(
			Mtype::anchor(get_origin(owner.clone()), operation.hash),
			mtype::Error::<Test>::MTypeAlreadyExists
		);
	});
}
