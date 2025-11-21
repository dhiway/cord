#![allow(dead_code)]

use base64::{self, Engine};
use bs58;
use hex;
use origin_primitives::{
	identifier::Ss58Identifier,
	packet::PacketStatus,
	registry::{
		LookupSpecView, RegistryAttributeView, RegistryInfoView, RegistryKind, RegistryStatus,
	},
	view::{
		AttributeValueView, DevAttr, DevElement, DevPacketState, ElementView, PacketMetadataView,
		PacketStateView,
	},
};
use serde::{Deserialize, Serialize};

use crate::types::{
	element::{ElementJson, LocalizedElementJson},
	entity::HistoryEntry as LegacyHistoryEntry,
	token::StateEventRecord,
};

/// Canonical identifiers.
pub type EntityId = Ss58Identifier;
pub type RegisterId = Ss58Identifier;
pub type PacketId = Ss58Identifier;
pub type TokenId = Ss58Identifier;

/// Elements are represented in a view-friendly, typed form.
pub type Element = ElementView;

/// Attribute key/value pair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attribute {
	pub key: Vec<u8>,
	pub value: Element,
}

impl From<AttributeValueView> for Attribute {
	fn from(value: AttributeValueView) -> Self {
		Self { key: value.key, value: value.value }
	}
}

impl From<DevAttr> for Attribute {
	fn from(value: DevAttr) -> Self {
		let key = value.key_utf8.map(|s| s.into_bytes()).unwrap_or_else(|| {
			hex::decode(value.key_hex.trim_start_matches("0x")).unwrap_or_default()
		});
		Self { key, value: dev_element_into_element(&value.value) }
	}
}

/// Entity snapshot returned by the SDK.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entity {
	pub id: EntityId,
	pub display: Element,
	pub web: Element,
	pub email: Element,
	pub attributes: Vec<Attribute>,
}

impl Entity {
	pub fn attribute(&self, key: &[u8]) -> Option<&Attribute> {
		self.attributes.iter().find(|attr| attr.key.as_slice() == key)
	}
}

/// Register metadata view mapped into the SDK domain model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Register {
	pub id: RegisterId,
	pub info: Element,
	pub maintainer: Ss58Identifier,
	pub attributes: Vec<RegistryAttributeView>,
	pub token_spec: LookupSpecView,
	pub lookup_specs: Vec<LookupSpecView>,
	pub kind: RegistryKind,
	pub status: RegistryStatus,
}

impl Register {
	pub fn from_view(id: RegisterId, view: RegistryInfoView) -> Self {
		Self {
			id,
			info: view.info,
			maintainer: view.maintainer,
			attributes: view.attributes,
			token_spec: view.token_spec,
			lookup_specs: view.lookup_specs,
			kind: view.kind,
			status: view.status,
		}
	}
}

/// Packet state snapshot for a given version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PacketState {
	pub id: PacketId,
	pub registry: RegisterId,
	pub controller: Ss58Identifier,
	pub status: PacketStatus,
	pub version: u32,
	pub attributes_hash: Vec<u8>,
	pub attributes: Vec<Attribute>,
}

impl PacketState {
	pub fn from_view(id: PacketId, view: PacketStateView) -> Self {
		let attributes = view.attributes.into_iter().map(Attribute::from).collect();
		Self {
			id,
			registry: view.registry,
			controller: view.controller,
			status: view.status,
			version: view.version,
			attributes_hash: view.attributes_hash,
			attributes,
		}
	}

	pub fn from_dev(id: PacketId, view: DevPacketState) -> Self {
		let attributes = view.attributes.into_iter().map(Attribute::from).collect();
		let registry = decode_ss58_or_zero(&view.registry_ss58);
		let controller = decode_ss58_or_zero(&view.controller_ss58);
		Self {
			id,
			registry,
			controller,
			status: view.status,
			version: view.version,
			attributes_hash: hex::decode(view.attributes_hash_hex.trim_start_matches("0x"))
				.unwrap_or_default(),
			attributes,
		}
	}
}

/// Packet metadata view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PacketMetadata {
	pub id: PacketId,
	pub registry: RegisterId,
	pub controller: Ss58Identifier,
	pub status: PacketStatus,
	pub latest_version: u32,
	pub attributes_hash: Vec<u8>,
}

impl PacketMetadata {
	pub fn from_view(id: PacketId, view: PacketMetadataView) -> Self {
		Self {
			id,
			registry: view.registry,
			controller: view.controller,
			status: view.status,
			latest_version: view.latest_version,
			attributes_hash: view.attributes_hash,
		}
	}

	pub fn from_dev(
		id: PacketId,
		registry: RegisterId,
		controller: Ss58Identifier,
		status: PacketStatus,
		latest_version: u32,
		attributes_hash_hex: String,
	) -> Self {
		Self {
			id,
			registry,
			controller,
			status,
			latest_version,
			attributes_hash: hex::decode(attributes_hash_hex.trim_start_matches("0x"))
				.unwrap_or_default(),
		}
	}
}

/// Token wrapper; for now a thin alias around the identifier.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
	pub id: TokenId,
}

/// History entry alias for readability within the new SDK surface.
pub type HistoryEntry = LegacyHistoryEntry;

/// Composed entity view (info + limited history + timeline).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityOverview {
	pub entity: Entity,
	pub history: Vec<HistoryEntry>,
	pub timeline: Vec<StateEventRecord>,
	pub nym: Option<String>,
	pub linked_accounts: Vec<subxt::utils::AccountId32>,
}

/// Register overview (currently same as register info; reserved for expansion).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegisterOverview {
	pub register: Register,
}

/// Packet overview (metadata + state + timeline).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PacketOverview {
	pub metadata: PacketMetadata,
	pub state: PacketState,
	pub timeline: Vec<StateEventRecord>,
}

fn dev_element_into_element(dev: &DevElement) -> ElementView {
	match dev {
		DevElement::None => ElementView::None,
		DevElement::RawBase64(b64) => {
			let bytes = base64::engine::general_purpose::STANDARD.decode(b64).unwrap_or_default();
			ElementView::Raw(bytes)
		},
		DevElement::Bool(v) => ElementView::Bool(*v),
		DevElement::U64(v) => ElementView::U64(*v),
		DevElement::U128(v) => ElementView::U128(*v),
		DevElement::HashHex(hexstr) => {
			let mut digest = [0u8; 32];
			if let Ok(bytes) = hex::decode(hexstr.trim_start_matches("0x")) {
				let take = core::cmp::min(32, bytes.len());
				digest[..take].copy_from_slice(&bytes[..take]);
			}
			ElementView::Hash(digest)
		},
		DevElement::TokenSs58(ss58) => ElementView::Token(decode_ss58_or_zero(ss58)),
		DevElement::CidBase58(b58) => {
			let bytes = bs58::decode(b58).into_vec().unwrap_or_default();
			ElementView::Cid(bytes)
		},
	}
}

fn decode_ss58_or_zero(s: &str) -> Ss58Identifier {
	Ss58Identifier::try_from(s.to_string()).unwrap_or_else(|_| {
		Ss58Identifier::to_encoded([0u8; 32], 0, 0, 0).expect("static ss58 identifier")
	})
}

pub fn element_view_to_json(view: &ElementView) -> ElementJson {
	match view {
		ElementView::None => ElementJson::None,
		ElementView::Raw(bytes) => {
			ElementJson::RawBase64(base64::engine::general_purpose::STANDARD.encode(bytes))
		},
		ElementView::Bool(v) => ElementJson::Bool(*v),
		ElementView::U64(v) => ElementJson::U64(*v),
		ElementView::U128(v) => ElementJson::U128(*v),
		ElementView::Hash(h) => ElementJson::HashHex(hex::encode(h)),
		ElementView::Token(tok) => {
			ElementJson::TokenSs58(String::from_utf8_lossy(tok.as_ref()).into_owned())
		},
		ElementView::Cid(cid) => ElementJson::CidBase58(bs58::encode(cid).into_string()),
		ElementView::Localized(entries) => {
			let mapped = entries
				.iter()
				.map(|(locale, value)| LocalizedElementJson {
					locale: String::from_utf8_lossy(locale).into_owned(),
					value: element_view_to_json(value),
				})
				.collect();
			ElementJson::Localized(mapped)
		},
	}
}
