// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]

use super::*;
use crate::{
	mock::*,
	register::{AttributeFlags, LookupSpec, RegistryKind, RegistryPermissions, RegistryStatus},
	AttributePairsOf, LookupIndex, PacketPointer, PacketStatus, Packets, RegistryQueryCounts,
};
use alloc::format;
use cord_primitives::{
	packet::{Attribute, Element, ElementType},
	AccountId, Signature,
};
use core::{
	cmp::min,
	convert::{TryFrom, TryInto},
	sync::atomic::{AtomicU64, Ordering},
};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use sp_core::Pair;

static VIEW_AUTH_COUNTER: AtomicU64 = AtomicU64::new(0);

fn raw(data: &[u8]) -> Element<MaxRawDataLength> {
	Element::Raw(data.to_vec().try_into().expect("bounded element"))
}

fn attr_key(data: &[u8]) -> Attribute {
	Attribute::try_from(data.to_vec()).expect("key within bounds")
}

fn value_raw(data: &[u8]) -> Element<MaxRawDataLength> {
	Element::try_from(data).expect("value within bounds")
}

fn value_u64(value: u64) -> Element<MaxRawDataLength> {
	Element::from_u64(value)
}

fn attrs<'a>(
	pairs: impl IntoIterator<Item = (&'a [u8], ElementType, AttributeFlags)>,
) -> AttributeSchemaListOf<Test> {
	let mut list = AttributeSchemaListOf::<Test>::default();
	for (key, ty, flags) in pairs {
		let bounded_key = Attribute::try_from(key.to_vec()).expect("within key bound");
		list.try_push(AttributeSpec { key: bounded_key, kind: ty, flags })
			.expect("capacity");
	}
	list
}

fn token_spec(keys: &[&[u8]]) -> TokenSpecOf<Test> {
	match keys.len() {
		0 => LookupSpec::Combo(
			BoundedVec::<Attribute, <Test as Config>::MaxAdditionalAttributes>::default(),
		),
		1 => LookupSpec::Single(Attribute::try_from(keys[0].to_vec()).expect("within key bound")),
		_ => {
			let mut combo =
				BoundedVec::<Attribute, <Test as Config>::MaxAdditionalAttributes>::default();
			for key in keys {
				combo
					.try_push(Attribute::try_from(key.to_vec()).expect("within key bound"))
					.expect("combo capacity");
			}
			LookupSpec::Combo(combo)
		},
	}
}

fn lookup_specs(specs: &[&[&[u8]]]) -> LookupSpecListOf<Test> {
	let mut list = LookupSpecListOf::<Test>::default();
	for spec in specs {
		list.try_push(token_spec(spec)).expect("lookup spec capacity");
	}
	list
}

fn view_auth(account: AccountId) -> ViewAuthorizationOf<Test> {
	let pair = mock::ACCOUNT_KEYS
		.with(|keys| keys.borrow().get(&account).cloned())
		.expect("account key seeded");
	bind_account(account.clone());
	let id = VIEW_AUTH_COUNTER.fetch_add(1, Ordering::Relaxed);
	let payload_text = format!("view-auth-{id}");
	let payload_vec = payload_text.into_bytes();
	let payload: ViewAuthPayloadOf<Test> =
		payload_vec.clone().try_into().expect("payload within bounds");
	let signature = Signature::from(pair.sign(&payload_vec));
	ViewAuthorizationOf::<Test> { account, payload, signature }
}

fn default_auth() -> ViewAuthorizationOf<Test> {
	view_auth(account(0))
}

fn bind_delegate(registry: &Ss58Identifier, account: AccountId) -> Ss58Identifier {
	let token = bind_account(account.clone());
	RegistryDelegates::<Test>::insert(
		registry,
		&token,
		RegistryPermissions::ENTRY | RegistryPermissions::VIEW,
	);
	token
}

fn create_registry(
	maintainer: AccountId,
	attributes: AttributeSchemaListOf<Test>,
	token_spec_def: TokenSpecOf<Test>,
	lookups: LookupSpecListOf<Test>,
) -> (Ss58Identifier, Ss58Identifier) {
	let maintainer_token = bind_account(maintainer.clone());
	assert_ok!(Pallet::<Test>::create_registry(
		RuntimeOrigin::signed(maintainer.clone()),
		raw(b"Test Registry"),
		RegistryKind::Raw,
		attributes,
		token_spec_def,
		lookups,
	));
	(first_registry(), maintainer_token)
}

fn first_registry() -> Ss58Identifier {
	Registries::<Test>::iter_keys().next().expect("registry present")
}

#[test]
fn registry_creation_defaults_to_active() {
	new_test_ext().execute_with(|| {
		let attributes = attrs([(b"id".as_ref(), ElementType::U64, AttributeFlags::empty())]);
		let maintainer = account(10);
		let (registry, maintainer_token) = create_registry(
			maintainer.clone(),
			attributes.clone(),
			token_spec(&[b"id"]),
			lookup_specs(&[&[b"id"]]),
		);

		let info = Pallet::<Test>::info(default_auth(), registry.clone()).expect("info stored");
		assert_eq!(info.maintainer(), &maintainer_token);
		assert_eq!(info.status(), RegistryStatus::Active);
		assert_eq!(info.kind, RegistryKind::Raw);
		assert_eq!(info.attributes.len(), attributes.len());
		assert_eq!(info.token_spec.key_count(), 1);
		assert!(<Pallet<Test> as RegistryView<Test>>::registry_active(default_auth(), &registry));
	});
}

#[test]
fn create_registry_requires_lookup_specs() {
	new_test_ext().execute_with(|| {
		let maintainer = account(50);
		let _ = bind_account(maintainer.clone());
		let attributes = attrs([(b"id".as_ref(), ElementType::Raw, AttributeFlags::empty())]);
		let lookups = LookupSpecListOf::<Test>::default();

		assert_noop!(
			Pallet::<Test>::create_registry(
				RuntimeOrigin::signed(maintainer.clone()),
				raw(b"MissingLookups"),
				RegistryKind::Raw,
				attributes,
				token_spec(&[b"id"]),
				lookups,
			),
			Error::<Test>::NoLookupSpecs
		);
	});
}

#[test]
fn create_registry_rejects_optional_lookup_attribute() {
	new_test_ext().execute_with(|| {
		let maintainer = account(51);
		let _ = bind_account(maintainer.clone());
		let attributes = attrs([
			(b"required".as_ref(), ElementType::Raw, AttributeFlags::empty()),
			(b"optional".as_ref(), ElementType::Raw, AttributeFlags::OPTIONAL),
		]);

		assert_noop!(
			Pallet::<Test>::create_registry(
				RuntimeOrigin::signed(maintainer.clone()),
				raw(b"OptionalLookup"),
				RegistryKind::Raw,
				attributes,
				token_spec(&[b"required"]),
				lookup_specs(&[&[b"optional"]]),
			),
			Error::<Test>::InvalidAttributeKey
		);
	});
}

#[test]
fn registry_status_transitions_follow_rules() {
	new_test_ext().execute_with(|| {
		let (registry, maintainer_token) = create_registry(
			account(11),
			attrs([(b"id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"id"]),
			lookup_specs(&[&[b"id"]]),
		);

		// Non-admin (with token) cannot revoke.
		let _ = bind_account(account(99));
		assert_noop!(
			Pallet::<Test>::revoke_registry(RuntimeOrigin::signed(account(99)), registry.clone()),
			Error::<Test>::PermissionDenied
		);

		// Maintainer can revoke and restore.
		assert_ok!(Pallet::<Test>::revoke_registry(
			RuntimeOrigin::signed(account(11)),
			registry.clone()
		));
		assert_eq!(
			Pallet::<Test>::info(default_auth(), registry.clone()).unwrap().status(),
			RegistryStatus::Revoked
		);

		assert_ok!(Pallet::<Test>::restore_registry(
			RuntimeOrigin::signed(account(11)),
			registry.clone()
		));
		assert_eq!(
			Pallet::<Test>::info(default_auth(), registry.clone()).unwrap().status(),
			RegistryStatus::Active
		);

		// Root may revoke and delete.
		assert_ok!(Pallet::<Test>::revoke_registry(RuntimeOrigin::root(), registry.clone()));
		assert_ok!(Pallet::<Test>::delete_registry(RuntimeOrigin::root(), registry.clone()));
		assert_eq!(
			Pallet::<Test>::info(default_auth(), registry.clone()).unwrap().status(),
			RegistryStatus::Deleted
		);

		// Deleted registry cannot be restored.
		assert_noop!(
			Pallet::<Test>::restore_registry(RuntimeOrigin::signed(account(11)), registry.clone()),
			Error::<Test>::RegistryDeleted
		);

		// Maintainer still recorded.
		let maintainer_perms =
			RegistryDelegates::<Test>::get(&registry, maintainer_token).expect("perms stored");
		assert!(maintainer_perms.has_admin());
		assert!(maintainer_perms.has_view());
	});
}

#[test]
fn update_registry_info_requires_admin() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(12),
			attrs([(b"a".as_ref(), ElementType::Raw, AttributeFlags::empty())]),
			token_spec(&[b"a"]),
			lookup_specs(&[&[b"a"]]),
		);

		// Non-admin cannot update info.
		let _ = bind_account(account(42));
		assert_noop!(
			Pallet::<Test>::update_registry_info(
				RuntimeOrigin::signed(account(42)),
				registry.clone(),
				raw(b"Forbidden"),
			),
			Error::<Test>::PermissionDenied
		);

		// Maintainer can update.
		assert_ok!(Pallet::<Test>::update_registry_info(
			RuntimeOrigin::signed(account(12)),
			registry.clone(),
			raw(b"Updated"),
		));
		assert_eq!(Pallet::<Test>::info(default_auth(), registry).unwrap().info, raw(b"Updated"));
	});
}

#[test]
fn set_registry_delegate_requires_admin() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(13),
			attrs([(b"a".as_ref(), ElementType::Raw, AttributeFlags::empty())]),
			token_spec(&[b"a"]),
			lookup_specs(&[&[b"a"]]),
		);

		let _ = bind_account(account(99));
		assert_noop!(
			Pallet::<Test>::set_registry_delegate(
				RuntimeOrigin::signed(account(99)),
				registry.clone(),
				account(100),
				vec![RegistryPermissions::ENTRY],
			),
			Error::<Test>::PermissionDenied
		);

		let _ = bind_account(account(100));
		assert_ok!(Pallet::<Test>::set_registry_delegate(
			RuntimeOrigin::signed(account(13)),
			registry,
			account(100),
			vec![RegistryPermissions::ENTRY],
		));
	});
}

#[test]
fn remove_registry_delegate_checks_permissions_and_maintainer() {
	new_test_ext().execute_with(|| {
		let (registry, maintainer_token) = create_registry(
			account(14),
			attrs([(b"a".as_ref(), ElementType::Raw, AttributeFlags::empty())]),
			token_spec(&[b"a"]),
			lookup_specs(&[&[b"a"]]),
		);
		let delegate = account(15);
		let delegate_token = bind_account(delegate.clone());

		assert_ok!(Pallet::<Test>::set_registry_delegate(
			RuntimeOrigin::signed(account(14)),
			registry.clone(),
			delegate.clone(),
			vec![RegistryPermissions::ENTRY],
		));

		// Delegate cannot remove itself.
		assert_noop!(
			Pallet::<Test>::remove_registry_delegate(
				RuntimeOrigin::signed(delegate.clone()),
				registry.clone(),
				delegate_token.clone(),
			),
			Error::<Test>::PermissionDenied
		);

		// Maintainer cannot be removed.
		assert_noop!(
			Pallet::<Test>::remove_registry_delegate(
				RuntimeOrigin::signed(account(14)),
				registry.clone(),
				maintainer_token.clone(),
			),
			Error::<Test>::CannotRemoveMaintainer
		);

		// Maintainer can remove delegate.
		assert_ok!(Pallet::<Test>::remove_registry_delegate(
			RuntimeOrigin::signed(account(14)),
			registry,
			delegate_token,
		));
	});
}

#[test]
fn restore_registry_requires_revoked_status() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(60),
			attrs([(b"id".as_ref(), ElementType::Raw, AttributeFlags::empty())]),
			token_spec(&[b"id"]),
			lookup_specs(&[&[b"id"]]),
		);

		assert_noop!(
			Pallet::<Test>::restore_registry(RuntimeOrigin::signed(account(60)), registry.clone()),
			Error::<Test>::RegistryNotRevoked
		);

		assert_ok!(Pallet::<Test>::revoke_registry(
			RuntimeOrigin::signed(account(60)),
			registry.clone()
		));
		assert_ok!(Pallet::<Test>::restore_registry(
			RuntimeOrigin::signed(account(60)),
			registry.clone()
		));
		assert_eq!(
			Pallet::<Test>::info(default_auth(), registry).unwrap().status(),
			RegistryStatus::Active
		);
	});
}

#[test]
fn delete_registry_requires_revoked_status() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(61),
			attrs([(b"id".as_ref(), ElementType::Raw, AttributeFlags::empty())]),
			token_spec(&[b"id"]),
			lookup_specs(&[&[b"id"]]),
		);

		assert_noop!(
			Pallet::<Test>::delete_registry(RuntimeOrigin::signed(account(61)), registry.clone()),
			Error::<Test>::RegistryNotRevoked
		);

		assert_ok!(Pallet::<Test>::revoke_registry(
			RuntimeOrigin::signed(account(61)),
			registry.clone()
		));
		assert_ok!(Pallet::<Test>::delete_registry(
			RuntimeOrigin::signed(account(61)),
			registry.clone()
		));
		assert_eq!(
			Pallet::<Test>::info(default_auth(), registry).unwrap().status(),
			RegistryStatus::Deleted
		);
	});
}

#[test]
fn packet_lifecycle_tracks_versions() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(20),
			attrs([
				(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty()),
				(b"snapshot".as_ref(), ElementType::Raw, AttributeFlags::empty()),
			]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"snapshot"]]),
		);
		let _delegate = bind_delegate(&registry, account(21));

		let payload: AttributePairsOf<Test> = BoundedVec::try_from(vec![
			(attr_key(b"asset_id"), value_u64(41)),
			(attr_key(b"snapshot"), value_raw(b"initial")),
		])
		.expect("within bounds");

		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(21)),
			registry.clone(),
			payload,
		));

		let packet_id = System::events()
			.iter()
			.find_map(|record| match &record.event {
				RuntimeEvent::Register(crate::Event::PacketCreated { packet, .. }) => {
					Some(packet.clone())
				},
				_ => None,
			})
			.expect("created token");

		let snapshot =
			Pallet::<Test>::packet(default_auth(), registry.clone(), packet_id.clone(), None)
				.expect("snapshot");
		assert_eq!(snapshot.state.version, 1);
		assert_eq!(snapshot.registry_status, RegistryStatus::Active);
		// Update packet with new data.
		let updated = BoundedVec::try_from(vec![
			(attr_key(b"asset_id"), value_u64(99)),
			(attr_key(b"snapshot"), value_raw(b"updated")),
		])
		.expect("within bounds");

		assert_ok!(Pallet::<Test>::update_packet(
			RuntimeOrigin::signed(account(21)),
			registry.clone(),
			packet_id.clone(),
			updated,
		));

		let updated_snapshot =
			Pallet::<Test>::packet(default_auth(), registry.clone(), packet_id.clone(), None)
				.expect("updated snapshot");
		assert_eq!(updated_snapshot.state.version, 2);
		let updated_digest = packet::prepare_lookup_keys::<Test>(
			&registry,
			&Registries::<Test>::get(&registry).unwrap(),
			&updated_snapshot.state.attributes,
		)
		.expect("prepared")
		.first()
		.map(|(digest, _)| *digest)
		.expect("digest");
		let latest_anchor =
			LookupIndex::<Test>::get(&updated_digest, &registry).expect("lookup pointer");
		assert_eq!(
			latest_anchor.pointer,
			PacketPointer { rtoken: registry.clone(), ptoken: packet_id.clone(), version: 2 }
		);

		// Revoke, restore, and delete the packet.
		assert_ok!(Pallet::<Test>::revoke_packet(
			RuntimeOrigin::signed(account(21)),
			registry.clone(),
			packet_id.clone(),
		));
		let revoked = Packets::<Test>::get(&packet_id).expect("metadata");
		assert_eq!(revoked.status, PacketStatus::Revoked);

		assert_ok!(Pallet::<Test>::restore_packet(
			RuntimeOrigin::signed(account(21)),
			registry.clone(),
			packet_id.clone(),
		));
		let restored_snapshot =
			Pallet::<Test>::packet(default_auth(), registry.clone(), packet_id.clone(), None)
				.expect("restored snapshot");
		assert_eq!(restored_snapshot.state.status, PacketStatus::Active);

		assert_ok!(Pallet::<Test>::revoke_packet(
			RuntimeOrigin::signed(account(21)),
			registry.clone(),
			packet_id.clone(),
		));
		assert_ok!(Pallet::<Test>::remove_packet(
			RuntimeOrigin::signed(account(21)),
			registry.clone(),
			packet_id.clone(),
		));
		let removed = Packets::<Test>::get(&packet_id).expect("metadata");
		assert_eq!(removed.status, PacketStatus::Deleted);
		assert_eq!(removed.latest_version, 6);

		// Further updates are rejected.
		let err = Pallet::<Test>::update_packet(
			RuntimeOrigin::signed(account(21)),
			registry.clone(),
			packet_id.clone(),
			BoundedVec::default(),
		)
		.unwrap_err();
		assert_eq!(err, Error::<Test>::PacketDeleted.into());
	});
}

#[test]
fn create_packet_requires_delegate_permission() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(70),
			attrs([(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);

		let _ = bind_account(account(71));
		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(1))]).unwrap();

		assert_noop!(
			Pallet::<Test>::create_packet(
				RuntimeOrigin::signed(account(71)),
				registry.clone(),
				payload.clone(),
			),
			Error::<Test>::PermissionDenied
		);

		let _delegate = bind_delegate(&registry, account(72));
		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(72)),
			registry,
			payload,
		));
	});
}

#[test]
fn create_packet_rejects_missing_required_attribute() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(73),
			attrs([(b"required".as_ref(), ElementType::Raw, AttributeFlags::empty())]),
			token_spec(&[b"required"]),
			lookup_specs(&[&[b"required"]]),
		);
		let _delegate = bind_delegate(&registry, account(74));

		let payload: AttributePairsOf<Test> = BoundedVec::default();
		assert_noop!(
			Pallet::<Test>::create_packet(RuntimeOrigin::signed(account(74)), registry, payload,),
			Error::<Test>::MissingAttribute
		);
	});
}

#[test]
fn create_packet_rejects_duplicate_entries() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(75),
			attrs([(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);
		let _delegate = bind_delegate(&registry, account(76));

		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(99))]).unwrap();
		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(76)),
			registry.clone(),
			payload.clone(),
		));
		assert_noop!(
			Pallet::<Test>::create_packet(RuntimeOrigin::signed(account(76)), registry, payload,),
			Error::<Test>::PacketAlreadyExists
		);
	});
}

#[test]
fn update_packet_rejects_unknown_attribute() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(77),
			attrs([(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);
		let _delegate = bind_delegate(&registry, account(78));

		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(1))]).unwrap();
		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(78)),
			registry.clone(),
			payload,
		));
		let packet_id = Packets::<Test>::iter_keys().next().unwrap();

		let bad_update: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"unknown"), value_u64(2))]).unwrap();
		assert_noop!(
			Pallet::<Test>::update_packet(
				RuntimeOrigin::signed(account(78)),
				registry.clone(),
				packet_id.clone(),
				bad_update,
			),
			Error::<Test>::UnknownAttribute
		);

		assert_ok!(Pallet::<Test>::revoke_registry(RuntimeOrigin::signed(account(77)), registry));
	});
}

#[test]
fn update_packet_rejects_inactive_registry() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(79),
			attrs([(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);
		let _delegate = bind_delegate(&registry, account(80));

		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(1))]).unwrap();
		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(80)),
			registry.clone(),
			payload,
		));
		let packet_id = Packets::<Test>::iter_keys().next().unwrap();

		assert_ok!(Pallet::<Test>::revoke_registry(
			RuntimeOrigin::signed(account(79)),
			registry.clone(),
		));

		let update: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(2))]).unwrap();
		assert_noop!(
			Pallet::<Test>::update_packet(
				RuntimeOrigin::signed(account(80)),
				registry,
				packet_id,
				update,
			),
			Error::<Test>::RegistryInactive
		);
	});
}

#[test]
fn revoke_packet_requires_active_status() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(81),
			attrs([(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);
		let _delegate = bind_delegate(&registry, account(82));

		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(1))]).unwrap();
		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(82)),
			registry.clone(),
			payload,
		));
		let packet_id = Packets::<Test>::iter_keys().next().unwrap();

		assert_ok!(Pallet::<Test>::revoke_packet(
			RuntimeOrigin::signed(account(82)),
			registry.clone(),
			packet_id.clone(),
		));
		assert_noop!(
			Pallet::<Test>::revoke_packet(RuntimeOrigin::signed(account(82)), registry, packet_id,),
			Error::<Test>::PacketRevoked
		);
	});
}

#[test]
fn restore_packet_requires_revoked_status() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(83),
			attrs([(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);
		let _delegate = bind_delegate(&registry, account(84));

		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(1))]).unwrap();
		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(84)),
			registry.clone(),
			payload,
		));
		let packet_id = Packets::<Test>::iter_keys().next().unwrap();

		assert_noop!(
			Pallet::<Test>::restore_packet(
				RuntimeOrigin::signed(account(84)),
				registry.clone(),
				packet_id.clone(),
			),
			Error::<Test>::PacketNotRevoked
		);

		assert_ok!(Pallet::<Test>::revoke_packet(
			RuntimeOrigin::signed(account(84)),
			registry.clone(),
			packet_id.clone(),
		));
		assert_ok!(Pallet::<Test>::restore_packet(
			RuntimeOrigin::signed(account(84)),
			registry,
			packet_id,
		));
	});
}

#[test]
fn remove_packet_requires_revoked_status() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(85),
			attrs([(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);
		let _delegate = bind_delegate(&registry, account(86));

		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(1))]).unwrap();
		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(86)),
			registry.clone(),
			payload,
		));
		let packet_id = Packets::<Test>::iter_keys().next().unwrap();

		assert_noop!(
			Pallet::<Test>::remove_packet(
				RuntimeOrigin::signed(account(86)),
				registry.clone(),
				packet_id.clone(),
			),
			Error::<Test>::PacketNotRevoked
		);

		assert_ok!(Pallet::<Test>::revoke_packet(
			RuntimeOrigin::signed(account(86)),
			registry.clone(),
			packet_id.clone(),
		));
		assert_ok!(Pallet::<Test>::remove_packet(
			RuntimeOrigin::signed(account(86)),
			registry,
			packet_id,
		));
	});
}

#[test]
fn optional_attributes_allow_absence() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(30),
			attrs([
				(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty()),
				(b"note".as_ref(), ElementType::Raw, AttributeFlags::OPTIONAL),
			]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);
		let _delegate = bind_delegate(&registry, account(31));

		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(1))])
				.expect("within bounds");

		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(31)),
			registry.clone(),
			payload,
		));

		let packet_id = Packets::<Test>::iter_keys().next().expect("packet stored");
		let snapshot =
			Pallet::<Test>::packet(default_auth(), registry.clone(), packet_id.clone(), None)
				.expect("snapshot");
		assert!(snapshot.state.attributes.get(b"note").is_none());

		let none_payload: AttributePairsOf<Test> = BoundedVec::try_from(vec![
			(attr_key(b"asset_id"), value_u64(1)),
			(attr_key(b"note"), Element::None),
		])
		.expect("within bounds");

		assert_ok!(Pallet::<Test>::update_packet(
			RuntimeOrigin::signed(account(31)),
			registry,
			packet_id,
			none_payload,
		));
	});
}

#[test]
fn lookup_queries_return_latest_state() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(40),
			attrs([
				(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty()),
				(b"snapshot".as_ref(), ElementType::Raw, AttributeFlags::empty()),
			]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"snapshot"]]),
		);
		let _delegate = bind_delegate(&registry, account(41));

		let payload: AttributePairsOf<Test> = BoundedVec::try_from(vec![
			(attr_key(b"asset_id"), value_u64(7)),
			(attr_key(b"snapshot"), value_raw(b"initial")),
		])
		.expect("within bounds");

		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(41)),
			registry.clone(),
			payload,
		));

		let packet_id = Packets::<Test>::iter_keys().next().expect("packet stored");
		let registry_info = Registries::<Test>::get(&registry).expect("registry info");
		let snapshot =
			Pallet::<Test>::packet(default_auth(), registry.clone(), packet_id.clone(), None)
				.expect("snapshot");
		let digest = packet::prepare_lookup_keys::<Test>(
			&registry,
			&registry_info,
			&snapshot.state.attributes,
		)
		.expect("prepared")
		.first()
		.map(|(digest, _)| *digest)
		.expect("digest");

		let lookup_snapshot =
			Pallet::<Test>::packet_by_lookup(default_auth(), registry.clone(), digest, None)
				.expect("lookup snapshot");
		assert_eq!(lookup_snapshot.state.version, snapshot.state.version);

		// Update only the asset identifier; lookup digest (based on snapshot attribute) remains.
		let partial_update =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(8))]).expect("bounded");
		assert_ok!(Pallet::<Test>::update_packet(
			RuntimeOrigin::signed(account(41)),
			registry.clone(),
			packet_id.clone(),
			partial_update,
		));

		let anchor = LookupIndex::<Test>::get(&digest, &registry).expect("lookup anchor");
		assert_eq!(anchor.pointer.ptoken, packet_id);
		assert_eq!(anchor.pointer.rtoken, registry);
		assert_eq!(anchor.pointer.version, 2);

		let updated_snapshot =
			Pallet::<Test>::packet_by_lookup(default_auth(), registry.clone(), digest, None)
				.expect("updated");
		assert_eq!(updated_snapshot.state.version, 2);
		assert_eq!(
			PacketStates::<Test>::get(&packet_id, 1).expect("previous version").status,
			PacketStatus::Revoked
		);
	});
}

#[test]
fn packets_by_token_prefix_returns_snapshots() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(90),
			attrs([(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);
		let _delegate = bind_delegate(&registry, account(91));
		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(5))]).expect("bounded");

		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(91)),
			registry.clone(),
			payload,
		));

		let packet_id = Packets::<Test>::iter_keys().next().expect("packet stored");
		let token_bytes = packet_id.as_ref().to_vec();
		let prefix_len = min(4, token_bytes.len());
		let token_prefix = token_bytes[..prefix_len].to_vec();

		let full_matches =
			Pallet::<Test>::packets_by_token(view_auth(account(92)), token_bytes, None);
		assert!(full_matches.iter().any(|snapshot| snapshot.state.registry == registry));

		let prefix_matches =
			Pallet::<Test>::packets_by_token(view_auth(account(93)), token_prefix, None);
		assert!(prefix_matches.iter().any(|snapshot| snapshot.state.registry == registry));
	});
}

#[test]
fn packets_by_lookup_digest_returns_snapshots() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(94),
			attrs([(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);
		let _delegate = bind_delegate(&registry, account(95));
		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(8))]).expect("bounded");

		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(95)),
			registry.clone(),
			payload,
		));

		let packet_id = Packets::<Test>::iter_keys().next().expect("packet stored");
		let (digest, _reg, _anchor) = LookupIndex::<Test>::iter()
			.find(|(_, reg, anchor)| reg == &registry && anchor.pointer.ptoken == packet_id)
			.expect("lookup entry");
		let digest_bytes = digest.as_ref().to_vec();
		let prefix_len = min(4, digest_bytes.len());
		let digest_prefix = digest_bytes[..prefix_len].to_vec();

		let full_matches =
			Pallet::<Test>::packets_by_lookup_digest(view_auth(account(96)), digest_bytes, None);
		assert!(full_matches.iter().any(|snapshot| snapshot.state.registry == registry));

		let prefix_matches =
			Pallet::<Test>::packets_by_lookup_digest(view_auth(account(97)), digest_prefix, None);
		assert!(prefix_matches.iter().any(|snapshot| snapshot.state.registry == registry));
	});
}

#[test]
fn registry_view_queries_increment_counter() {
	new_test_ext().execute_with(|| {
		let (registry, _) = create_registry(
			account(98),
			attrs([(b"asset_id".as_ref(), ElementType::U64, AttributeFlags::empty())]),
			token_spec(&[b"asset_id"]),
			lookup_specs(&[&[b"asset_id"]]),
		);
		let _delegate = bind_delegate(&registry, account(99));
		let payload: AttributePairsOf<Test> =
			BoundedVec::try_from(vec![(attr_key(b"asset_id"), value_u64(11))]).expect("bounded");

		assert_ok!(Pallet::<Test>::create_packet(
			RuntimeOrigin::signed(account(99)),
			registry.clone(),
			payload,
		));

		let packet_id = Packets::<Test>::iter_keys().next().expect("packet stored");
		let (digest, _registry, _anchor) = LookupIndex::<Test>::iter()
			.find(|(_, reg, anchor)| reg == &registry && anchor.pointer.ptoken == packet_id)
			.expect("lookup entry");
		let digest = digest.clone();

		let account = account(0);
		assert_eq!(RegistryQueryCounts::<Test>::get(&registry, account.clone()), 0);

		let _ = Pallet::<Test>::info(default_auth(), registry.clone()).expect("info");
		assert_eq!(RegistryQueryCounts::<Test>::get(&registry, account.clone()), 1);

		let _ = Pallet::<Test>::packet(default_auth(), registry.clone(), packet_id.clone(), None)
			.expect("packet");
		assert_eq!(RegistryQueryCounts::<Test>::get(&registry, account.clone()), 2);

		let _ = Pallet::<Test>::packet_by_lookup(
			default_auth(),
			registry.clone(),
			digest.clone(),
			None,
		)
		.expect("lookup packet");
		assert_eq!(RegistryQueryCounts::<Test>::get(&registry, account.clone()), 3);

		let token_bytes = packet_id.as_ref().to_vec();
		let token_matches =
			Pallet::<Test>::packets_by_token(default_auth(), token_bytes.clone(), None);
		assert!(!token_matches.is_empty());
		assert_eq!(
			RegistryQueryCounts::<Test>::get(&registry, account.clone()),
			3 + token_matches.len() as u64
		);

		let digest_matches = Pallet::<Test>::packets_by_lookup_digest(
			default_auth(),
			digest.as_ref().to_vec(),
			None,
		);
		assert!(!digest_matches.is_empty());
		assert_eq!(
			RegistryQueryCounts::<Test>::get(&registry, account),
			3 + token_matches.len() as u64 + digest_matches.len() as u64
		);
	});
}
