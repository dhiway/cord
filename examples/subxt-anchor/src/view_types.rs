#[allow(unused_imports)]
use codec::Decode;
use hex::encode as hex_encode;
use sp_core::H256;
use std::fmt;

#[derive(Clone, Debug)]
pub struct IdentifierView {
	raw: Vec<u8>,
	label: String,
}

impl IdentifierView {
	pub fn new(raw: Vec<u8>) -> Self {
		let label = bytes_to_label(&raw);
		Self { raw, label }
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.raw
	}
}

impl fmt::Display for IdentifierView {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.label)
	}
}

#[derive(Clone, Debug)]
pub struct RegistryInfoView {
	pub info: ElementValue,
	pub maintainer: IdentifierView,
	pub attributes: Vec<AttributeSpecView>,
	pub token_spec: LookupSpecView,
	pub lookup_specs: Vec<LookupSpecView>,
	pub kind: RegistryKindView,
	pub status: RegistryStatusView,
}

#[derive(Clone, Debug)]
pub struct AttributeSpecView {
	pub key_bytes: Vec<u8>,
	pub key_label: String,
	pub kind: ElementKindView,
	pub optional: bool,
}

#[derive(Clone, Debug)]
pub enum LookupSpecView {
	Single(String),
	Combo(Vec<String>),
}

#[derive(Clone, Debug)]
pub struct PacketSnapshotView {
	pub state: PacketStateView,
	pub registry_status: RegistryStatusView,
}

#[derive(Clone, Debug)]
pub struct PacketStateView {
	pub registry: IdentifierView,
	pub controller: IdentifierView,
	pub status: PacketStatusView,
	pub version: u32,
	pub attributes_hash: H256,
	pub attributes: Vec<AttributeValueView>,
}

#[derive(Clone, Debug)]
pub struct AttributeValueView {
	pub key_bytes: Vec<u8>,
	pub key_label: String,
	pub value: ElementValue,
}

#[derive(Clone, Debug)]
pub enum ElementValue {
	None,
	Raw { bytes: Vec<u8>, preview: String },
	Bool(bool),
	U64(u64),
	U128(u128),
	Hash(String),
	Token(IdentifierView),
	Cid { bytes: Vec<u8>, hex: String },
}

#[derive(Clone, Debug)]
pub enum ElementKindView {
	None,
	Raw,
	Bool,
	U64,
	U128,
	Hash,
	Token,
	Cid,
}

#[derive(Clone, Debug)]
pub enum RegistryKindView {
	Raw,
	Token,
	Hash,
}

#[derive(Clone, Debug)]
pub enum RegistryStatusView {
	Active,
	Revoked,
	Deleted,
}

#[derive(Clone, Debug)]
pub enum PacketStatusView {
	Active,
	Revoked,
	Deleted,
}

impl From<scale::RegistryInfo> for RegistryInfoView {
	fn from(raw: scale::RegistryInfo) -> Self {
		let attributes = raw.attributes.into_iter().map(AttributeSpecView::from).collect();
		let lookup_specs = raw.lookup_specs.into_iter().map(LookupSpecView::from).collect();
		Self {
			info: ElementValue::from(raw.info),
			maintainer: IdentifierView::new(raw.maintainer),
			attributes,
			token_spec: LookupSpecView::from(raw.token_spec),
			lookup_specs,
			kind: RegistryKindView::from(raw.kind),
			status: RegistryStatusView::from(raw.status),
		}
	}
}

impl From<scale::legacy::RegistryInfo> for RegistryInfoView {
	fn from(raw: scale::legacy::RegistryInfo) -> Self {
		let canonical = scale::RegistryInfo {
			info: scale::Element::Raw(raw.info),
			maintainer: raw.maintainer,
			attributes: raw.attributes,
			token_spec: raw.token_spec,
			lookup_specs: raw.lookup_specs,
			kind: raw.kind,
			status: raw.status,
		};
		RegistryInfoView::from(canonical)
	}
}

impl From<scale::AttributeSpec> for AttributeSpecView {
	fn from(raw: scale::AttributeSpec) -> Self {
		Self {
			key_label: bytes_to_label(&raw.key),
			key_bytes: raw.key,
			kind: ElementKindView::from(raw.kind),
			optional: raw.flags & 1 == 1,
		}
	}
}

impl From<scale::LookupSpec> for LookupSpecView {
	fn from(raw: scale::LookupSpec) -> Self {
		match raw {
			scale::LookupSpec::Single(bytes) => LookupSpecView::Single(bytes_to_label(&bytes)),
			scale::LookupSpec::Combo(list) => {
				LookupSpecView::Combo(list.into_iter().map(|b| bytes_to_label(&b)).collect())
			},
		}
	}
}

impl From<scale::PacketSnapshot> for PacketSnapshotView {
	fn from(raw: scale::PacketSnapshot) -> Self {
		Self {
			registry_status: RegistryStatusView::from(raw.registry_status),
			state: raw.state.into(),
		}
	}
}

impl From<scale::PacketState> for PacketStateView {
	fn from(raw: scale::PacketState) -> Self {
		let attributes = raw
			.attributes
			.into_iter()
			.map(|(key, value)| AttributeValueView {
				key_label: bytes_to_label(&key),
				key_bytes: key,
				value: ElementValue::from(value),
			})
			.collect();
		Self {
			registry: IdentifierView::new(raw.registry),
			controller: IdentifierView::new(raw.controller),
			status: PacketStatusView::from(raw.status),
			version: raw.version,
			attributes_hash: raw.attributes_hash,
			attributes,
		}
	}
}

impl From<scale::Element> for ElementValue {
	fn from(raw: scale::Element) -> Self {
		match raw {
			scale::Element::None => ElementValue::None,
			scale::Element::Raw(bytes) => {
				ElementValue::Raw { preview: preview_text(&bytes), bytes }
			},
			scale::Element::Bool(flag) => ElementValue::Bool(flag != 0),
			scale::Element::U64(bytes) => ElementValue::U64(u64::from_le_bytes(bytes)),
			scale::Element::U128(bytes) => ElementValue::U128(u128::from_le_bytes(bytes)),
			scale::Element::Hash(digest) => ElementValue::Hash(format!("0x{}", hex_encode(digest))),
			scale::Element::Token(bytes) => ElementValue::Token(IdentifierView::new(bytes)),
			scale::Element::Cid(bytes) => {
				ElementValue::Cid { hex: format!("0x{}", hex_encode(&bytes)), bytes }
			},
		}
	}
}

impl fmt::Display for ElementValue {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			ElementValue::None => write!(f, "None"),
			ElementValue::Raw { preview, .. } => write!(f, "Raw({preview})"),
			ElementValue::Bool(flag) => write!(f, "Bool({flag})"),
			ElementValue::U64(value) => write!(f, "U64({value})"),
			ElementValue::U128(value) => write!(f, "U128({value})"),
			ElementValue::Hash(hex) => write!(f, "Hash({hex})"),
			ElementValue::Token(id) => write!(f, "Token({id})"),
			ElementValue::Cid { hex, .. } => write!(f, "CID({hex})"),
		}
	}
}

impl From<scale::ElementType> for ElementKindView {
	fn from(raw: scale::ElementType) -> Self {
		match raw {
			scale::ElementType::None => ElementKindView::None,
			scale::ElementType::Raw => ElementKindView::Raw,
			scale::ElementType::Bool => ElementKindView::Bool,
			scale::ElementType::U64 => ElementKindView::U64,
			scale::ElementType::U128 => ElementKindView::U128,
			scale::ElementType::Hash => ElementKindView::Hash,
			scale::ElementType::Token => ElementKindView::Token,
			scale::ElementType::Cid => ElementKindView::Cid,
		}
	}
}

impl From<scale::RegistryKind> for RegistryKindView {
	fn from(raw: scale::RegistryKind) -> Self {
		match raw {
			scale::RegistryKind::Raw => RegistryKindView::Raw,
			scale::RegistryKind::Token => RegistryKindView::Token,
			scale::RegistryKind::Hash => RegistryKindView::Hash,
		}
	}
}

impl From<scale::RegistryStatus> for RegistryStatusView {
	fn from(raw: scale::RegistryStatus) -> Self {
		match raw {
			scale::RegistryStatus::Active => RegistryStatusView::Active,
			scale::RegistryStatus::Revoked => RegistryStatusView::Revoked,
			scale::RegistryStatus::Deleted => RegistryStatusView::Deleted,
		}
	}
}

impl From<scale::PacketStatus> for PacketStatusView {
	fn from(raw: scale::PacketStatus) -> Self {
		match raw {
			scale::PacketStatus::Active => PacketStatusView::Active,
			scale::PacketStatus::Revoked => PacketStatusView::Revoked,
			scale::PacketStatus::Deleted => PacketStatusView::Deleted,
		}
	}
}

fn preview_text(bytes: &[u8]) -> String {
	if bytes.is_empty() {
		return String::from("<empty>");
	}
	if bytes.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
		String::from_utf8_lossy(bytes).into_owned()
	} else {
		format!("0x{}", hex_encode(bytes))
	}
}

fn bytes_to_label(bytes: &[u8]) -> String {
	String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| format!("0x{}", hex_encode(bytes)))
}

pub mod scale {
	use super::*;
	#[allow(unused_imports)]
	use codec::Decode;

	#[derive(Clone, Debug, Decode)]
	pub struct RegistryInfo {
		pub info: Element,
		pub maintainer: Vec<u8>,
		pub attributes: Vec<AttributeSpec>,
		pub token_spec: LookupSpec,
		pub lookup_specs: Vec<LookupSpec>,
		pub kind: RegistryKind,
		pub status: RegistryStatus,
	}

	#[derive(Clone, Debug, Decode)]
	pub struct AttributeSpec {
		pub key: Vec<u8>,
		pub kind: ElementType,
		pub flags: u8,
	}

	#[derive(Clone, Debug, Decode)]
	pub enum LookupSpec {
		Single(Vec<u8>),
		Combo(Vec<Vec<u8>>),
	}

	#[derive(Clone, Debug, Decode)]
	pub struct PacketSnapshot {
		pub state: PacketState,
		pub registry_status: RegistryStatus,
	}

	#[derive(Clone, Debug, Decode)]
	pub struct PacketState {
		pub registry: Vec<u8>,
		pub controller: Vec<u8>,
		pub status: PacketStatus,
		pub version: u32,
		pub attributes_hash: H256,
		pub attributes: Vec<(Vec<u8>, Element)>,
	}

	#[derive(Clone, Debug, Decode)]
	pub enum Element {
		None,
		Raw(Vec<u8>),
		Bool(u8),
		U64([u8; 8]),
		U128([u8; 16]),
		Hash([u8; 32]),
		Token(Vec<u8>),
		Cid(Vec<u8>),
	}

	#[derive(Clone, Debug, Decode)]
	pub enum ElementType {
		None,
		Raw,
		Bool,
		U64,
		U128,
		Hash,
		Token,
		Cid,
	}

	#[derive(Clone, Debug, Decode)]
	pub enum RegistryKind {
		Raw,
		Token,
		Hash,
	}

	#[derive(Clone, Debug, Decode)]
	pub enum RegistryStatus {
		Active,
		Revoked,
		Deleted,
	}

	#[derive(Clone, Debug, Decode)]
	pub enum PacketStatus {
		Active,
		Revoked,
		Deleted,
	}

	pub mod legacy {
		use super::*;
		use codec::Decode;

		#[derive(Clone, Debug, Decode)]
		pub struct RegistryInfo {
			pub info: Vec<u8>,
			pub maintainer: Vec<u8>,
			pub attributes: Vec<AttributeSpec>,
			pub token_spec: LookupSpec,
			pub lookup_specs: Vec<LookupSpec>,
			pub kind: RegistryKind,
			pub status: RegistryStatus,
		}
	}
}
