// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// View-friendly structs for SDKs and off-chain clients.

use crate::{
	identifier::Ss58Identifier,
	packet::{Attribute, Element, PacketMetadata, PacketState, PacketStatus},
};
use alloc::{borrow::ToOwned, string::String, vec::Vec};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use bs58;
use codec::{Decode, Encode};
use core::{marker::PhantomData, ops::Deref};
use frame_support::traits::Get;
use hex;
use scale_decode::{visitor, DecodeAsType, IntoVisitor, TypeResolver};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_runtime::{AccountId32 as RuntimeAccountId32, RuntimeDebug};

#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, RuntimeDebug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId32(RuntimeAccountId32);

impl AccountId32 {
	pub fn into_inner(self) -> RuntimeAccountId32 {
		self.0
	}

	pub fn as_inner(&self) -> &RuntimeAccountId32 {
		&self.0
	}
}

impl From<RuntimeAccountId32> for AccountId32 {
	fn from(value: RuntimeAccountId32) -> Self {
		Self(value)
	}
}

impl From<&RuntimeAccountId32> for AccountId32 {
	fn from(value: &RuntimeAccountId32) -> Self {
		Self(value.clone())
	}
}

impl Deref for AccountId32 {
	type Target = RuntimeAccountId32;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

pub struct AccountId32Visitor<R>(PhantomData<R>);

impl<R: TypeResolver> visitor::Visitor for AccountId32Visitor<R> {
	type Value<'scale, 'resolver> = AccountId32;
	type Error = scale_decode::Error;
	type TypeResolver = R;

	fn unchecked_decode_as_type<'scale, 'resolver>(
		self,
		input: &mut &'scale [u8],
		type_id: <Self::TypeResolver as TypeResolver>::TypeId,
		types: &'resolver Self::TypeResolver,
	) -> visitor::DecodeAsTypeResult<Self, Result<Self::Value<'scale, 'resolver>, Self::Error>> {
		let decoded =
			visitor::decode_with_visitor(input, type_id, types, <[u8; 32]>::into_visitor())
				.map(|bytes| AccountId32(RuntimeAccountId32::from(bytes)));
		visitor::DecodeAsTypeResult::Decoded(decoded)
	}
}

impl IntoVisitor for AccountId32 {
	type AnyVisitor<R: TypeResolver> = AccountId32Visitor<R>;

	fn into_visitor<R: TypeResolver>() -> Self::AnyVisitor<R> {
		AccountId32Visitor(PhantomData)
	}
}

/// Human-friendly representation of [`Element`].
#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum ElementView {
	None,
	Raw(Vec<u8>),
	Bool(bool),
	U64(u64),
	U128(u128),
	Hash([u8; 32]),
	Token(Ss58Identifier),
	Cid(Vec<u8>),
}

impl<MaxRawDataLength: Get<u32>> From<&Element<MaxRawDataLength>> for ElementView {
	fn from(value: &Element<MaxRawDataLength>) -> Self {
		match value {
			Element::None => ElementView::None,
			Element::Raw(bytes) => ElementView::Raw(bytes.to_vec()),
			Element::Bool(flag) => ElementView::Bool(*flag != 0),
			Element::U64(bytes) => ElementView::U64(u64::from_le_bytes(*bytes)),
			Element::U128(bytes) => ElementView::U128(u128::from_le_bytes(*bytes)),
			Element::Hash(digest) => ElementView::Hash(*digest),
			Element::Token(token) => ElementView::Token(token.clone()),
			Element::CID(bytes) => ElementView::Cid(bytes.to_vec()),
		}
	}
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub struct AttributeValueView {
	pub key: Vec<u8>,
	pub value: ElementView,
}

impl AttributeValueView {
	pub fn from_pair<MaxRawDataLength: Get<u32>>(
		pair: &(Attribute, Element<MaxRawDataLength>),
	) -> Self {
		let (key, value) = pair;
		Self { key: key.to_vec(), value: ElementView::from(value) }
	}
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub struct PacketStateView {
	pub registry: Ss58Identifier,
	pub controller: Ss58Identifier,
	pub status: PacketStatus,
	pub version: u32,
	pub attributes_hash: Vec<u8>,
	pub attributes: Vec<AttributeValueView>,
}

impl<
		MaxRawDataLength: Get<u32>,
		MaxAdditionalAttributes: Get<u32>,
		Hash: Clone + PartialEq + Eq + core::fmt::Debug + Encode,
	> From<&PacketState<MaxRawDataLength, MaxAdditionalAttributes, Hash>> for PacketStateView
{
	fn from(state: &PacketState<MaxRawDataLength, MaxAdditionalAttributes, Hash>) -> Self {
		let attributes = state.attributes.iter().map(AttributeValueView::from_pair).collect();
		Self {
			registry: state.registry.clone(),
			controller: state.controller.clone(),
			status: state.status.clone(),
			version: state.version,
			attributes_hash: state.attributes_hash.encode(),
			attributes,
		}
	}
}

impl PacketStateView {
	pub fn attribute(&self, key: &[u8]) -> Option<&ElementView> {
		self.attributes
			.iter()
			.find(|attr| attr.key.as_slice() == key)
			.map(|attr| &attr.value)
	}
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub struct PacketMetadataView {
	pub registry: Ss58Identifier,
	pub controller: Ss58Identifier,
	pub status: PacketStatus,
	pub latest_version: u32,
	pub attributes_hash: Vec<u8>,
}

impl<Hash: Clone + PartialEq + Eq + core::fmt::Debug + Encode> From<&PacketMetadata<Hash>>
	for PacketMetadataView
{
	fn from(meta: &PacketMetadata<Hash>) -> Self {
		Self {
			registry: meta.registry.clone(),
			controller: meta.controller.clone(),
			status: meta.status.clone(),
			latest_version: meta.latest_version,
			attributes_hash: meta.attributes_hash.encode(),
		}
	}
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub struct DevAttr {
	pub key_utf8: Option<String>,
	pub key_hex: String,
	pub value: DevElement,
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum DevElement {
	None,
	Bool(bool),
	U64(u64),
	U128(u128),
	HashHex(String),
	TokenSs58(String),
	CidBase58(String),
	RawBase64(String),
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub struct DevPacketState {
	pub registry_ss58: String,
	pub controller_ss58: String,
	pub status: PacketStatus,
	pub version: u32,
	pub attributes_hash_hex: String,
	pub attributes: Vec<DevAttr>,
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub struct DevPacketSnapshot<S>
where
	S: Serialize + Clone + PartialEq + Eq + Encode + Decode + TypeInfo + DecodeAsType,
{
	pub state: DevPacketState,
	pub registry_status: S,
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub struct DevEventBlockView {
	pub height: u32,
	pub index: u32,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, RuntimeDebug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityEventBlock {
	pub account: crate::view::AccountId32,
	pub height: u32,
	pub index: u32,
}

pub fn dev_element_from(ev: &ElementView) -> DevElement {
	match ev {
		ElementView::None => DevElement::None,
		ElementView::Bool(value) => DevElement::Bool(*value),
		ElementView::U64(value) => DevElement::U64(*value),
		ElementView::U128(value) => DevElement::U128(*value),
		ElementView::Hash(bytes) => DevElement::HashHex(hex_string(bytes)),
		ElementView::Token(token) => DevElement::TokenSs58(ss58_string(token)),
		ElementView::Cid(bytes) => DevElement::CidBase58(base58_string(bytes)),
		ElementView::Raw(bytes) => DevElement::RawBase64(base64_string(bytes)),
	}
}

pub fn dev_attr_from(view: &AttributeValueView) -> DevAttr {
	DevAttr {
		key_utf8: maybe_utf8(&view.key),
		key_hex: hex_string(&view.key),
		value: dev_element_from(&view.value),
	}
}

pub fn dev_packet_state_from(view: &PacketStateView) -> DevPacketState {
	DevPacketState {
		registry_ss58: ss58_string(&view.registry),
		controller_ss58: ss58_string(&view.controller),
		status: view.status.clone(),
		version: view.version,
		attributes_hash_hex: hex_string(&view.attributes_hash),
		attributes: view.attributes.iter().map(dev_attr_from).collect(),
	}
}

pub fn dev_packet_snapshot_from<S>(
	state: &PacketStateView,
	registry_status: S,
) -> DevPacketSnapshot<S>
where
	S: Serialize + Clone + PartialEq + Eq + Encode + Decode + TypeInfo + DecodeAsType,
{
	DevPacketSnapshot { state: dev_packet_state_from(state), registry_status }
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub struct EntityInfoView {
	pub display: ElementView,
	pub web: ElementView,
	pub email: ElementView,
	pub attributes: Option<Vec<AttributeValueView>>,
}

impl EntityInfoView {
	pub fn attribute(&self, key: &[u8]) -> Option<&ElementView> {
		self.attributes
			.as_ref()
			.and_then(|attrs| attrs.iter().find(|attr| attr.key.as_slice() == key))
			.map(|attr| &attr.value)
	}
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, RuntimeDebug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityOverview {
	pub info: EntityInfoView,
	pub nym: Option<Vec<u8>>,
	pub linked_accounts: Vec<crate::view::AccountId32>,
	pub history: Vec<InfoAttributeHistoryEntry>,
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub struct InfoAttributeHistoryEntry {
	pub key_hex: String,
	pub key_utf8: Option<String>,
	pub version: u64,
	pub old_value_base64: String,
	pub block: DevEventBlockView,
}

#[derive(
	Clone,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub struct InfoTokenHistoryEntry {
	pub action_utf8: Option<String>,
	pub action_hex: String,
	pub action_base64: String,
	pub digest_hex: String,
	pub block: DevEventBlockView,
}

pub fn ss58_string(id: &Ss58Identifier) -> String {
	id.to_string_lossy()
}

pub fn hex_string(bytes: &[u8]) -> String {
	let mut s = String::from("0x");
	s.push_str(&hex::encode(bytes));
	s
}

pub fn base58_string(bytes: &[u8]) -> String {
	bs58::encode(bytes).into_string()
}

pub fn base64_string(bytes: &[u8]) -> String {
	BASE64_STANDARD.encode(bytes)
}

pub fn maybe_utf8(bytes: &[u8]) -> Option<String> {
	core::str::from_utf8(bytes).ok().map(|s| s.to_owned())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		packet::{Attribute, Element},
		Ss58Identifier,
	};
	use alloc::vec;
	use frame_support::{traits::ConstU32, BoundedVec};

	#[test]
	fn element_view_normalizes_variants() {
		let raw: Element<ConstU32<64>> =
			Element::Raw(BoundedVec::try_from(vec![1u8, 2, 3]).expect("bounded"));
		let bool_elem: Element<ConstU32<64>> = Element::Bool(1);
		let u64_elem: Element<ConstU32<64>> = Element::U64(42u64.to_le_bytes());

		assert!(matches!(ElementView::from(&raw), ElementView::Raw(bytes) if bytes == vec![1,2,3]));
		assert_eq!(ElementView::from(&bool_elem), ElementView::Bool(true));
		assert_eq!(ElementView::from(&u64_elem), ElementView::U64(42));
	}

	#[test]
	fn attribute_view_from_pair_clones_key() {
		let key: Attribute = BoundedVec::try_from(b"id".to_vec()).unwrap();
		let element: Element<ConstU32<64>> = Element::Bool(0);
		let pair = (key.clone(), element);
		let view = AttributeValueView::from_pair(&pair);
		assert_eq!(view.key, b"id".to_vec());
		assert_eq!(view.value, ElementView::Bool(false));
	}

	#[test]
	fn packet_state_attribute_lookup() {
		let attrs = vec![AttributeValueView { key: b"id".to_vec(), value: ElementView::U128(7) }];
		let view = PacketStateView {
			registry: sample_identifier('r'),
			controller: sample_identifier('c'),
			status: PacketStatus::Active,
			version: 3,
			attributes_hash: vec![0, 1],
			attributes: attrs,
		};
		assert_eq!(view.attribute(b"id"), Some(&ElementView::U128(7)));
		assert!(view.attribute(b"missing").is_none());
	}

	#[test]
	fn info_helpers_render_expected_shapes() {
		let state = PacketStateView {
			registry: sample_identifier('r'),
			controller: sample_identifier('c'),
			status: PacketStatus::Revoked,
			version: 5,
			attributes_hash: vec![0xAA, 0xBB],
			attributes: vec![AttributeValueView {
				key: b"name".to_vec(),
				value: ElementView::Raw(b"hi".to_vec()),
			}],
		};
		let info_state = dev_packet_state_from(&state);
		assert_eq!(info_state.version, 5);
		assert_eq!(info_state.attributes_hash_hex, "0xaabb");
		assert_eq!(info_state.attributes[0].key_utf8.as_deref(), Some("name"));
		if let DevElement::RawBase64(encoded) = &info_state.attributes[0].value {
			assert_eq!(encoded, &base64_string(b"hi"));
		} else {
			panic!("expected raw base64");
		}
		let snapshot = dev_packet_snapshot_from(&state, PacketStatus::Deleted);
		assert_eq!(snapshot.registry_status, PacketStatus::Deleted);
	}

	#[test]
	fn ss58_string_falls_back_to_hex_when_not_utf8() {
		let raw = vec![0xFFu8; 4];
		let inner: BoundedVec<u8, ConstU32<64>> = BoundedVec::try_from(raw).unwrap();
		let ident = Ss58Identifier(inner);
		let rendered = ss58_string(&ident);
		assert!(rendered.starts_with("0x"));
	}

	fn sample_identifier(tag: char) -> Ss58Identifier {
		// Derive deterministic digest by repeating the ASCII tag.
		let digest = [tag as u8; 32];
		Ss58Identifier::to_encoded(digest, 1, 2, 1).expect("identifier")
	}
}
