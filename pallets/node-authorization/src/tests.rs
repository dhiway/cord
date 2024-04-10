// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Tests for node-authorization pallet.

use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::BadOrigin;

#[test]
fn check_genesis_well_known_nodes() {
	new_test_ext().execute_with(|| {
		let expected_nodes = vec![
			generate_peer(TEST_NODE_1),
			generate_peer(TEST_NODE_2),
			generate_peer(TEST_NODE_3),
		];
		let expected_set: BTreeSet<PeerId> = expected_nodes.into_iter().collect();

		assert_eq!(WellKnownNodes::<Test>::get(), expected_set);

		assert_eq!(
			Owners::<Test>::get(generate_peer(TEST_NODE_1)),
			Some(NodeInfo { id: test_node_id(TEST_NODE_1), owner: 10 })
		);
	});
}

#[test]
fn add_well_known_node_works() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			NodeAuthorization::add_well_known_node(
				RuntimeOrigin::signed(2),
				test_node(TEST_NODE_4),
				15
			),
			BadOrigin
		);
		assert_noop!(
			NodeAuthorization::add_well_known_node(
				RuntimeOrigin::signed(1),
				test_node(TEST_NODE_LEN),
				15
			),
			Error::<Test>::NodeIdTooLong
		);
		assert_noop!(
			NodeAuthorization::add_well_known_node(
				RuntimeOrigin::signed(1),
				test_node(TEST_NODE_7),
				15
			),
			Error::<Test>::InvalidNodeIdentifier
		);
		assert_ok!(NodeAuthorization::add_well_known_node(
			RuntimeOrigin::signed(1),
			test_node(TEST_NODE_4),
			20
		));
		assert_noop!(
			NodeAuthorization::add_well_known_node(
				RuntimeOrigin::signed(1),
				test_node(TEST_NODE_4),
				20
			),
			Error::<Test>::AlreadyJoined
		);

		assert_eq!(
			Owners::<Test>::get(generate_peer(TEST_NODE_4)),
			Some(NodeInfo { id: test_node_id(TEST_NODE_4), owner: 20 })
		);

		assert_noop!(
			NodeAuthorization::add_well_known_node(
				RuntimeOrigin::signed(1),
				test_node(TEST_NODE_5),
				25
			),
			Error::<Test>::TooManyNodes
		);
	});
}

#[test]
fn adding_already_claimed_well_known_node_should_fail() {
	new_test_ext().execute_with(|| {
		let node_id = test_node(TEST_NODE_6);

		let node = NodeAuthorization::generate_peer_id(&node_id).unwrap();
		let bounded_node_id: sp_runtime::BoundedVec<u8, _> =
			node_id.clone().try_into().expect("Node ID too long");

		<Owners<Test>>::insert(&node, NodeInfo { id: bounded_node_id, owner: 10 });

		assert_noop!(
			NodeAuthorization::add_well_known_node(RuntimeOrigin::signed(1), node_id.clone(), 20),
			Error::<Test>::AlreadyClaimed
		);
	});
}

#[test]
fn remove_well_known_node_works() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			NodeAuthorization::remove_well_known_node(
				RuntimeOrigin::signed(3),
				test_node(TEST_NODE_1)
			),
			BadOrigin
		);
		assert_noop!(
			NodeAuthorization::remove_well_known_node(
				RuntimeOrigin::signed(1),
				test_node(TEST_NODE_LEN)
			),
			Error::<Test>::NodeIdTooLong
		);
		assert_noop!(
			NodeAuthorization::remove_well_known_node(
				RuntimeOrigin::signed(1),
				test_node(TEST_NODE_7),
			),
			Error::<Test>::InvalidNodeIdentifier
		);
		assert_noop!(
			NodeAuthorization::remove_well_known_node(
				RuntimeOrigin::signed(1),
				test_node(TEST_NODE_5)
			),
			Error::<Test>::NotExist
		);

		AdditionalConnections::<Test>::insert(
			generate_peer(TEST_NODE_1),
			BTreeSet::from_iter(vec![generate_peer(TEST_NODE_4)]),
		);
		assert!(AdditionalConnections::<Test>::contains_key(generate_peer(TEST_NODE_1)));

		assert_ok!(NodeAuthorization::remove_well_known_node(
			RuntimeOrigin::signed(1),
			test_node(TEST_NODE_2)
		));

		let expected_nodes = vec![generate_peer(TEST_NODE_1), generate_peer(TEST_NODE_3)];
		let expected_set: BTreeSet<PeerId> = expected_nodes.into_iter().collect();

		assert_eq!(WellKnownNodes::<Test>::get(), expected_set);

		assert!(!Owners::<Test>::contains_key(generate_peer(TEST_NODE_2)));
		assert!(!AdditionalConnections::<Test>::contains_key(generate_peer(TEST_NODE_2)));
	});
}

#[test]
fn swap_well_known_node_works() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			NodeAuthorization::swap_well_known_node(
				RuntimeOrigin::signed(1),
				test_node(TEST_NODE_1),
				test_node(TEST_NODE_5)
			),
			Error::<Test>::NotOwner
		);
		assert_noop!(
			NodeAuthorization::swap_well_known_node(
				RuntimeOrigin::signed(1),
				test_node(TEST_NODE_LEN),
				test_node(TEST_NODE_5)
			),
			Error::<Test>::NodeIdTooLong
		);
		assert_noop!(
			NodeAuthorization::swap_well_known_node(
				RuntimeOrigin::signed(1),
				test_node(TEST_NODE_7),
				test_node(TEST_NODE_6)
			),
			Error::<Test>::InvalidNodeIdentifier
		);
		assert_noop!(
			NodeAuthorization::swap_well_known_node(
				RuntimeOrigin::signed(3),
				test_node(TEST_NODE_1),
				test_node(TEST_NODE_LEN)
			),
			Error::<Test>::NodeIdTooLong
		);

		assert_ok!(NodeAuthorization::swap_well_known_node(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_4)
		));

		let expected_nodes = vec![
			generate_peer(TEST_NODE_4),
			generate_peer(TEST_NODE_2),
			generate_peer(TEST_NODE_3),
		];
		let expected_set: BTreeSet<PeerId> = expected_nodes.into_iter().collect();

		assert_eq!(WellKnownNodes::<Test>::get(), expected_set);

		assert_noop!(
			NodeAuthorization::swap_well_known_node(
				RuntimeOrigin::signed(3),
				test_node(TEST_NODE_5),
				test_node(TEST_NODE_1)
			),
			Error::<Test>::NotExist
		);
		assert_noop!(
			NodeAuthorization::swap_well_known_node(
				RuntimeOrigin::signed(20),
				test_node(TEST_NODE_2),
				test_node(TEST_NODE_3)
			),
			Error::<Test>::AlreadyJoined
		);

		AdditionalConnections::<Test>::insert(
			generate_peer(TEST_NODE_2),
			BTreeSet::from_iter(vec![generate_peer(TEST_NODE_4)]),
		);

		assert_ok!(NodeAuthorization::swap_well_known_node(
			RuntimeOrigin::signed(20),
			test_node(TEST_NODE_2),
			test_node(TEST_NODE_5)
		));

		let expected_nodes = vec![
			generate_peer(TEST_NODE_4),
			generate_peer(TEST_NODE_5),
			generate_peer(TEST_NODE_3),
		];
		let expected_set: BTreeSet<PeerId> = expected_nodes.into_iter().collect();

		assert_eq!(WellKnownNodes::<Test>::get(), expected_set);

		assert!(!Owners::<Test>::contains_key(generate_peer(TEST_NODE_2)));
		assert!(!AdditionalConnections::<Test>::contains_key(generate_peer(TEST_NODE_2)));

		assert_eq!(
			Owners::<Test>::get(generate_peer(TEST_NODE_5)),
			Some(NodeInfo { id: test_node_id(TEST_NODE_5), owner: 20 })
		);

		assert!(Owners::<Test>::contains_key(generate_peer(TEST_NODE_5)));
		assert!(AdditionalConnections::<Test>::contains_key(generate_peer(TEST_NODE_5)));

		AdditionalConnections::<Test>::insert(
			generate_peer(TEST_NODE_5),
			BTreeSet::from_iter(vec![generate_peer(TEST_NODE_4)]),
		);
	});
}

#[test]
fn transfer_node_works() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			NodeAuthorization::transfer_node(
				RuntimeOrigin::signed(15),
				test_node(TEST_NODE_LEN),
				10
			),
			Error::<Test>::NodeIdTooLong
		);

		assert_noop!(
			NodeAuthorization::transfer_node(RuntimeOrigin::signed(15), test_node(TEST_NODE_2), 10),
			Error::<Test>::NotOwner
		);

		assert_ok!(NodeAuthorization::transfer_node(
			RuntimeOrigin::signed(20),
			test_node(TEST_NODE_2),
			15
		));
		assert_eq!(
			Owners::<Test>::get(generate_peer(TEST_NODE_2)),
			Some(NodeInfo { id: test_node_id(TEST_NODE_2), owner: 15 })
		);
	});
}

#[test]
fn add_connections_works() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			NodeAuthorization::add_connection(
				RuntimeOrigin::signed(15),
				test_node(TEST_NODE_LEN),
				test_node(TEST_NODE_5)
			),
			Error::<Test>::NodeIdTooLong
		);
		assert_noop!(
			NodeAuthorization::add_connection(
				RuntimeOrigin::signed(15),
				test_node(TEST_NODE_1),
				test_node(TEST_NODE_5)
			),
			Error::<Test>::NotOwner
		);

		assert_ok!(NodeAuthorization::add_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_4)
		));
		assert_ok!(NodeAuthorization::add_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_5)
		));
		assert_ok!(NodeAuthorization::add_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_6)
		));
		assert_eq!(
			AdditionalConnections::<Test>::get(generate_peer(TEST_NODE_1)),
			BTreeSet::from_iter(vec![
				generate_peer(TEST_NODE_4),
				generate_peer(TEST_NODE_5),
				generate_peer(TEST_NODE_6)
			])
		);
	});
}

#[test]
fn remove_connections_works() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			NodeAuthorization::remove_connection(
				RuntimeOrigin::signed(15),
				test_node(TEST_NODE_LEN),
				test_node(TEST_NODE_5)
			),
			Error::<Test>::NodeIdTooLong
		);

		assert_noop!(
			NodeAuthorization::remove_connection(
				RuntimeOrigin::signed(15),
				test_node(TEST_NODE_1),
				test_node(TEST_NODE_5)
			),
			Error::<Test>::NotOwner
		);
		assert_ok!(NodeAuthorization::add_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_4)
		));
		assert_ok!(NodeAuthorization::add_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_5)
		));
		assert_ok!(NodeAuthorization::add_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_6)
		));

		assert_ok!(NodeAuthorization::remove_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_5)
		));
		assert_eq!(
			AdditionalConnections::<Test>::get(generate_peer(TEST_NODE_1)),
			BTreeSet::from_iter(vec![generate_peer(TEST_NODE_4), generate_peer(TEST_NODE_6)])
		);
	});
}

#[test]
fn get_authorized_nodes_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(NodeAuthorization::add_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_4)
		));
		assert_ok!(NodeAuthorization::add_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_5)
		));
		assert_ok!(NodeAuthorization::add_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_6)
		));

		let authorized_nodes = Pallet::<Test>::get_authorized_nodes(&generate_peer(TEST_NODE_1));
		assert_eq!(
			authorized_nodes,
			vec![
				generate_peer(TEST_NODE_6),
				generate_peer(TEST_NODE_5),
				generate_peer(TEST_NODE_4),
				generate_peer(TEST_NODE_3),
				generate_peer(TEST_NODE_2),
			]
		);
	});
}

#[test]
fn adding_already_connected_connection_should_fail() {
	new_test_ext().execute_with(|| {
		assert_ok!(NodeAuthorization::add_connection(
			RuntimeOrigin::signed(10),
			test_node(TEST_NODE_1),
			test_node(TEST_NODE_2)
		));

		assert_noop!(
			NodeAuthorization::add_connection(
				RuntimeOrigin::signed(10),
				test_node(TEST_NODE_1),
				test_node(TEST_NODE_2)
			),
			Error::<Test>::AlreadyConnected
		);
	});
}
