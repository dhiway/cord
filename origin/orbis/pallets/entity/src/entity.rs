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

use alloc::vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use enumflags2::{bitflags, BitFlag, BitFlags};
use frame_support::{ensure, traits::Get, CloneNoBound, DebugNoBound, EqNoBound, PartialEqNoBound};
use origin_primitives::{
	attribute::{Attribute, Attributes, AttributesError, Element},
	packet::{PacketInformationProvider, PacketUpdateError, PacketUpdateOp},
};
use scale_info::{build::Variants, Path, Type, TypeInfo};

/// Each field corresponds to a field in the `EntityInfo` struct.
#[bitflags]
#[repr(u64)]
#[derive(
	Clone, Copy, PartialEq, Eq, DebugNoBound, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen,
)]
pub enum EntityField {
	Display,
	Web,
	Email,
	Attributes,
}

impl EntityField {
	/// Map a reserved key name to its enum variant, or `None` for dynamic attributes.
	pub fn from_bytes(key: &[u8]) -> Option<Self> {
		match key {
			b"display" => Some(EntityField::Display),
			b"web" => Some(EntityField::Web),
			b"email" => Some(EntityField::Email),
			b"attributes" => Some(EntityField::Attributes),
			_ => None,
		}
	}
}

impl TypeInfo for EntityField {
	type Identity = Self;
	fn type_info() -> Type {
		Type::builder().path(Path::new("EntityField", module_path!())).variant(
			Variants::new()
				.variant("Display", |v| v.index(0))
				.variant("Web", |v| v.index(1))
				.variant("Email", |v| v.index(2))
				.variant("Attributes", |v| v.index(3)),
		)
	}
}

fn map_attributes_error(err: AttributesError) -> PacketUpdateError {
	match err {
		AttributesError::DuplicateKey => PacketUpdateError::AttributeExists,
		AttributesError::TooManyAttributes => PacketUpdateError::TooManyAttributes,
		AttributesError::InvalidElement => PacketUpdateError::InvalidElement,
	}
}

/// On-chain entity information for an account.
///
/// Parameterized by:
///  - `MaxRawDataLength`: capacity for each `Element<…>` field
///  - `MaxAdditionalAttributes`: max length of the `attributes` Vec
#[derive(
	CloneNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	DebugNoBound,
	TypeInfo,
)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(MaxRawDataLength, MaxAdditionalAttributes))]
pub struct EntityInfo<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> {
	pub display: Element<MaxRawDataLength>,
	pub web: Element<MaxRawDataLength>,
	pub email: Element<MaxRawDataLength>,
	pub attributes: Option<Attributes<MaxRawDataLength, MaxAdditionalAttributes>>,
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>>
	EntityInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	pub(crate) fn fields(&self) -> BitFlags<EntityField> {
		let mut bits = BitFlags::empty();
		if !self.display.is_none() {
			bits.insert(EntityField::Display)
		}
		if !self.web.is_none() {
			bits.insert(EntityField::Web)
		}
		if !self.email.is_none() {
			bits.insert(EntityField::Email)
		}
		if let Some(attrs) = &self.attributes {
			if !attrs.is_empty() {
				bits.insert(EntityField::Attributes);
			}
		}
		bits
	}
}

impl<MaxRawDataLength: Get<u32> + 'static, MaxAdditionalAttributes: Get<u32>>
	PacketInformationProvider for EntityInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	type FieldMask = u64;
	type MaxRawDataLength = MaxRawDataLength;
	type MaxAdditionalAttributes = MaxAdditionalAttributes;
	type UpdateOp = PacketUpdateOp<MaxRawDataLength>;

	fn attributes(
		&self,
	) -> Option<&Attributes<Self::MaxRawDataLength, Self::MaxAdditionalAttributes>> {
		self.attributes.as_ref()
	}

	fn get_key(&self, key: &[u8]) -> Element<Self::MaxRawDataLength> {
		if let Some(field) = EntityField::from_bytes(key) {
			return match field {
				EntityField::Display => self.display.clone(),
				EntityField::Web => self.web.clone(),
				EntityField::Email => self.email.clone(),
				EntityField::Attributes => Element::default(),
			};
		}
		self.attributes
			.as_ref()
			.and_then(|attrs| attrs.get(key).cloned())
			.unwrap_or_else(Element::default)
	}

	fn present_fields(&self) -> Self::FieldMask {
		self.fields().bits()
	}

	fn has_info_fields(&self, mask: Self::FieldMask) -> bool {
		self.present_fields() & mask == mask
	}

	fn apply_update(
		&mut self,
		op: &PacketUpdateOp<MaxRawDataLength>,
	) -> Result<(), PacketUpdateError> {
		match op {
			PacketUpdateOp::AddAttribute(k, v) => {
				v.validate().map_err(|_| PacketUpdateError::InvalidElement)?;
				ensure!(EntityField::from_bytes(k).is_none(), PacketUpdateError::AttributeExists);
				let attrs = self.attributes.get_or_insert_with(Attributes::default);
				attrs.try_insert(k.clone(), v.clone()).map_err(map_attributes_error)?;
				Ok(())
			},

			PacketUpdateOp::RemoveAttribute(k) => {
				if let Some(field) = EntityField::from_bytes(k) {
					match field {
						EntityField::Display => self.display = Element::default(),
						EntityField::Web => self.web = Element::default(),
						EntityField::Email => self.email = Element::default(),
						EntityField::Attributes => {
							return Err(PacketUpdateError::AttributeNotFound);
						},
					}
				} else {
					let attrs =
						self.attributes.as_mut().ok_or(PacketUpdateError::AttributeNotFound)?;
					if attrs.remove(k.as_slice()).is_none() {
						return Err(PacketUpdateError::AttributeNotFound);
					}
					if attrs.is_empty() {
						self.attributes = None;
					}
				}
				Ok(())
			},

			PacketUpdateOp::UpdateAttribute(k, v) => {
				if let Some(field) = EntityField::from_bytes(k) {
					v.validate().map_err(|_| PacketUpdateError::InvalidElement)?;
					match field {
						EntityField::Display => self.display = v.clone(),
						EntityField::Web => self.web = v.clone(),
						EntityField::Email => self.email = v.clone(),
						EntityField::Attributes => {
							return Err(PacketUpdateError::AttributeNotFound);
						},
					}
				} else {
					let attrs =
						self.attributes.as_mut().ok_or(PacketUpdateError::AttributeNotFound)?;
					let slot =
						attrs.get_mut(k.as_slice()).ok_or(PacketUpdateError::AttributeNotFound)?;
					v.validate().map_err(|_| PacketUpdateError::InvalidElement)?;
					*slot = v.clone();
				}
				Ok(())
			},
		}
	}

	fn create_info() -> Self {
		let empty = Element::default();
		let cap = MaxAdditionalAttributes::get() as usize;
		let mut attrs = Attributes::default();
		for i in 0..cap {
			let key: Attribute = vec![b'k', i as u8].try_into().unwrap();
			attrs.try_insert(key, empty.clone()).expect("within bounds; qed");
		}
		EntityInfo {
			display: empty.clone(),
			web: empty.clone(),
			email: empty.clone(),
			attributes: Some(attrs),
		}
	}

	fn all_fields() -> Self::FieldMask {
		EntityField::all().bits()
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> Default
	for EntityInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	fn default() -> Self {
		EntityInfo {
			display: Element::default(),
			web: Element::default(),
			email: Element::default(),
			attributes: None,
		}
	}
}
