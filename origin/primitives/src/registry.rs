use crate::{identifier::Ss58Identifier, packet::ElementType, view::ElementView};
use alloc::{format, string::String, vec::Vec};
use bitflags::bitflags;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_decode::DecodeAsType;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;

bitflags! {
	#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
	pub struct RegistryPermissions: u16 {
		const VIEW     = 1 << 0;
		const ENTRY    = 1 << 1;
		const DELEGATE = 1 << 2;
		const ADMIN    = 1 << 3;
	}
}

impl Default for RegistryPermissions {
	fn default() -> Self {
		RegistryPermissions::ENTRY | RegistryPermissions::VIEW
	}
}

impl RegistryPermissions {
	pub fn from_list(list: &[RegistryPermissions]) -> Self {
		let mut mask = list.iter().copied().fold(Self::empty(), |acc, p| acc | p);
		if mask.intersects(
			RegistryPermissions::ADMIN | RegistryPermissions::ENTRY | RegistryPermissions::DELEGATE,
		) {
			mask |= RegistryPermissions::VIEW;
		}
		mask
	}

	pub fn has_entry(self) -> bool {
		self.contains(RegistryPermissions::ADMIN) || self.contains(RegistryPermissions::ENTRY)
	}

	pub fn has_delegate(self) -> bool {
		self.contains(RegistryPermissions::ADMIN) || self.contains(RegistryPermissions::DELEGATE)
	}

	pub fn has_admin(self) -> bool {
		self.contains(RegistryPermissions::ADMIN)
	}

	pub fn has_view(self) -> bool {
		self.contains(RegistryPermissions::VIEW)
			|| self.contains(RegistryPermissions::ENTRY)
			|| self.contains(RegistryPermissions::ADMIN)
	}
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
	Default,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub enum RegistryKind {
	#[default]
	Raw,
	Token,
	Hash,
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	Copy,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
	Default,
	Serialize,
	Deserialize,
	DecodeAsType,
)]
#[serde(rename_all = "camelCase")]
pub enum RegistryStatus {
	#[default]
	Active,
	Revoked,
	Deleted,
}

impl RegistryStatus {
	pub fn is_active(self) -> bool {
		matches!(self, RegistryStatus::Active)
	}

	pub fn is_revoked(self) -> bool {
		matches!(self, RegistryStatus::Revoked)
	}

	pub fn is_deleted(self) -> bool {
		matches!(self, RegistryStatus::Deleted)
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
pub struct RegistryAttributeView {
	pub key: Vec<u8>,
	pub kind: ElementType,
	pub optional: bool,
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
pub enum LookupSpecView {
	Single(Vec<u8>),
	Combo(Vec<Vec<u8>>),
}

impl LookupSpecView {
	pub fn fingerprint(&self) -> String {
		match self {
			LookupSpecView::Single(key) => format!("single:0x{}", hex::encode(key)),
			LookupSpecView::Combo(list) => {
				let mut buf = String::from("combo:");
				for key in list {
					buf.push_str(&format!("0x{};", hex::encode(key)));
				}
				buf
			},
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
pub struct RegistryInfoView {
	pub info: ElementView,
	pub maintainer: Ss58Identifier,
	pub attributes: Vec<RegistryAttributeView>,
	pub token_spec: LookupSpecView,
	pub lookup_specs: Vec<LookupSpecView>,
	pub kind: RegistryKind,
	pub status: RegistryStatus,
}
