// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

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

#![allow(clippy::too_many_lines)]

use bitflags::bitflags;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use cord_primitives::doket::{
	Attributes, DoketInformationProvider, DoketUpdateError, DoketUpdateOp, Element,
};
// use enumflags2::{bitflags, BitFlag, BitFlags};
use frame_support::{
	ensure, traits::Get, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound,
};
use scale_info::{build::Variants, Path, Type, TypeInfo};

// Each field corresponds to a field in the `EntityInfo` struct.
bitflags! {
	#[derive(Encode, Decode,  MaxEncodedLen, DecodeWithMemTracking)]
	pub struct EntityField: u64 {
		const DISPLAY    = 1 << 0;
		const LEGAL      = 1 << 1;
		const WEB        = 1 << 2;
		const EMAIL      = 1 << 3;
		const TWITTER    = 1 << 4;
		const ATTRIBUTES = 1 << 5;
	}
}

// Static mapping between reserved byte keys and EntityField variants for ergonomic lookup
const RESERVED_KEYS: &[(EntityField, &'static [u8])] = &[
	(EntityField::DISPLAY, b"display"),
	(EntityField::LEGAL, b"legal"),
	(EntityField::WEB, b"web"),
	(EntityField::EMAIL, b"email"),
	(EntityField::TWITTER, b"twitter"),
	(EntityField::ATTRIBUTES, b"attributes"),
];

impl EntityField {
	/// Map a reserved key name to its enum variant, or `None` for dynamic attributes.
	pub fn from_bytes(key: &[u8]) -> Option<Self> {
		RESERVED_KEYS.iter().find(|(_, name)| *name == key).map(|(field, _)| *field)
	}

	/// Convert the flag to its byte key representation (for reserved fields).
	pub fn to_bytes(self) -> &'static [u8] {
		RESERVED_KEYS
			.iter()
			.find(|(field, _)| *field == self)
			.map(|(_, name)| *name)
			.unwrap_or(&[])
	}
}

impl TypeInfo for EntityField {
	type Identity = Self;
	fn type_info() -> Type {
		Type::builder().path(Path::new("EntityField", module_path!())).variant(
			Variants::new()
				.variant("Display", |v| v.index(0))
				.variant("Legal", |v| v.index(1))
				.variant("Web", |v| v.index(2))
				.variant("Email", |v| v.index(3))
				.variant("Twitter", |v| v.index(4))
				.variant("Attributes", |v| v.index(5)),
		)
	}
}

/// On-chain entity information for an account.
#[derive(
	CloneNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	PartialEqNoBound,
	EqNoBound,
	RuntimeDebugNoBound,
	TypeInfo,
	MaxEncodedLen,
)]
#[scale_info(skip_type_params(MaxRawDataLength, MaxAdditionalAttributes))]
pub struct EntityInfo<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> {
	pub display: Element<MaxRawDataLength>,
	pub legal: Element<MaxRawDataLength>,
	pub web: Element<MaxRawDataLength>,
	pub email: Element<MaxRawDataLength>,
	pub twitter: Element<MaxRawDataLength>,
	pub attributes: Option<Attributes<MaxRawDataLength, MaxAdditionalAttributes>>,
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>>
	EntityInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	/// Returns a bitmask of reserved fields currently set, including ATTRIBUTES if non-empty.
	pub fn fields_mask(&self) -> EntityField {
		let mut bits = EntityField::empty();
		if !self.display.is_none() {
			bits |= EntityField::DISPLAY;
		}
		if !self.legal.is_none() {
			bits |= EntityField::LEGAL;
		}
		if !self.web.is_none() {
			bits |= EntityField::WEB;
		}
		if !self.email.is_none() {
			bits |= EntityField::EMAIL;
		}
		if !self.twitter.is_none() {
			bits |= EntityField::TWITTER;
		}
		if let Some(attrs) = &self.attributes {
			if !attrs.is_empty() {
				bits |= EntityField::ATTRIBUTES;
			}
		}
		bits
	}
}

impl<MaxRawDataLength: Get<u32> + 'static, MaxAdditionalAttributes: Get<u32>>
	DoketInformationProvider for EntityInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	type FieldMask = u64;
	type MaxRawDataLength = MaxRawDataLength;
	type MaxAdditionalAttributes = MaxAdditionalAttributes;
	type UpdateOp = DoketUpdateOp<MaxRawDataLength>;

	fn create_info() -> Self {
		Default::default()
	}

	fn all_fields() -> Self::FieldMask {
		EntityField::all().bits()
	}

	fn attributes(
		&self,
	) -> Option<&Attributes<Self::MaxRawDataLength, Self::MaxAdditionalAttributes>> {
		self.attributes.as_ref()
	}

	fn get_key(&self, key: &[u8]) -> Element<Self::MaxRawDataLength> {
		if let Some(field) = EntityField::from_bytes(key) {
			return match field {
				EntityField::DISPLAY => self.display.clone(),
				EntityField::LEGAL => self.legal.clone(),
				EntityField::WEB => self.web.clone(),
				EntityField::EMAIL => self.email.clone(),
				EntityField::TWITTER => self.twitter.clone(),
				EntityField::ATTRIBUTES => Element::default(),
				_ => unreachable!(),
			};
		}
		self.attributes
			.as_ref()
			.and_then(|attrs| {
				attrs.iter().find(|(k, _)| k.as_slice() == key).map(|(_, v)| v.clone())
			})
			.unwrap_or_default()
	}

	fn present_fields(&self) -> Self::FieldMask {
		self.fields_mask().bits()
	}

	fn has_info_fields(&self, mask: Self::FieldMask) -> bool {
		let present = self.present_fields();
		present & mask == mask
	}

	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), DoketUpdateError> {
		match op {
			DoketUpdateOp::AddAttribute(k, v) => {
				ensure!(EntityField::from_bytes(k).is_none(), DoketUpdateError::AttributeExists);
				let attrs = self.attributes.get_or_insert_with(Default::default);
				ensure!(!attrs.iter().any(|(kk, _)| kk == k), DoketUpdateError::AttributeExists);
				attrs
					.try_push((k.clone(), v.clone()))
					.map_err(|_| DoketUpdateError::TooManyAttributes)
			},
			DoketUpdateOp::RemoveAttribute(k) => {
				if let Some(field) = EntityField::from_bytes(k) {
					match field {
						EntityField::DISPLAY => self.display = Element::default(),
						EntityField::LEGAL => self.legal = Element::default(),
						EntityField::WEB => self.web = Element::default(),
						EntityField::EMAIL => self.email = Element::default(),
						EntityField::TWITTER => self.twitter = Element::default(),
						EntityField::ATTRIBUTES => return Err(DoketUpdateError::AttributeNotFound),
						_ => return Err(DoketUpdateError::AttributeNotFound),
					}
				} else {
					let attrs =
						self.attributes.as_mut().ok_or(DoketUpdateError::AttributeNotFound)?;
					let idx = attrs
						.iter()
						.position(|(kk, _)| kk == k)
						.ok_or(DoketUpdateError::AttributeNotFound)?;
					attrs.swap_remove(idx);
					if attrs.is_empty() {
						self.attributes = None;
					}
				}
				Ok(())
			},
			DoketUpdateOp::UpdateAttribute(k, v) => {
				if let Some(field) = EntityField::from_bytes(k) {
					match field {
						EntityField::DISPLAY => self.display = v.clone(),
						EntityField::LEGAL => self.legal = v.clone(),
						EntityField::WEB => self.web = v.clone(),
						EntityField::EMAIL => self.email = v.clone(),
						EntityField::TWITTER => self.twitter = v.clone(),
						EntityField::ATTRIBUTES => return Err(DoketUpdateError::AttributeNotFound),
						_ => return Err(DoketUpdateError::AttributeNotFound),
					}
				} else {
					let attrs =
						self.attributes.as_mut().ok_or(DoketUpdateError::AttributeNotFound)?;
					if let Some((_, val)) = attrs.iter_mut().find(|(kk, _)| kk == k) {
						*val = v.clone();
					} else {
						return Err(DoketUpdateError::AttributeNotFound);
					}
				}
				Ok(())
			},
		}
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> Default
	for EntityInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	fn default() -> Self {
		EntityInfo {
			display: Element::default(),
			legal: Element::default(),
			web: Element::default(),
			email: Element::default(),
			twitter: Element::default(),
			attributes: None,
		}
	}
}
