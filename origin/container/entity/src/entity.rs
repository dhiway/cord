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

use super::*;
use alloc::{vec, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use cord_primitives::doken::{
	Attribute, Attributes, DokenInformationProvider, DokenUpdateError, DokenUpdateOp, Element,
};
use enumflags2::{bitflags, BitFlag, BitFlags};
use frame_support::{
	ensure, parameter_types, traits::Get, CloneNoBound, EqNoBound, PartialEqNoBound,
	RuntimeDebugNoBound,
};
use scale_info::{build::Variants, Path, Type, TypeInfo};

parameter_types! {
	pub const MaxSubAccounts: u32 = 32;
	pub const MaxRawDataLength: u32 = 4096;
	pub const MaxUsernameLength: u32 = 32;
	pub const MaxAdditionalAttributes: u32 = 32;
	pub const GeneralAdminBodyId: BodyId = BodyId::Administration;
}

pub type IdentityAdminOrigin = EitherOfDiverse<
	EnsureRoot<AccountId>,
	EnsureXcm<IsVoiceOfBody<GovernanceLocation, GeneralAdminBodyId>>,
>;

impl pallet_entity::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Identifier = Identifier;
	type MaxSubAccounts = MaxSubAccounts;
	type MaxRawDataLength = MaxRawDataLength;
	type MaxAdditionalAttributes = MaxAdditionalAttributes;
	type EntityInformation = EntityInfo<MaxRawDataLength, MaxAdditionalAttributes>;
	type MaxUsernameLength = MaxUsernameLength;
	type ForceOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = weights::pallet_entity::WeightInfo<Runtime>;
}

/// Each field corresponds to a field in the `IdentityInfo` struct.
#[bitflags]
#[repr(u64)]
#[derive(
	Clone,
	Copy,
	PartialEq,
	Eq,
	RuntimeDebugNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
)]
pub enum EntityField {
	Display,
	Legal,
	Web,
	Attributes,
}

impl EntityField {
	/// Map a reserved key name to its enum variant, or `None` for dynamic attributes.
	pub fn from_bytes(key: &[u8]) -> Option<Self> {
		match key {
			b"display" => Some(EntityField::Display),
			b"legal" => Some(EntityField::Legal),
			b"web" => Some(EntityField::Web),
			b"attributes" => Some(EntityField::Attributes),
			_ => None,
		}
	}
}

impl TypeInfo for EntityField {
	type Identity = Self;
	fn type_info() -> scale_info::Type {
		Type::builder().path(Path::new("EntityField", module_path!())).variant(
			Variants::new()
				.variant("Display", |v| v.index(0))
				.variant("Legal", |v| v.index(1))
				.variant("Web", |v| v.index(2))
				.variant("Attributes", |v| v.index(3)),
		)
	}
}

/// Information concerning the identity of the controller of an account.
#[derive(
	CloneNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	RuntimeDebugNoBound,
	TypeInfo,
)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(MaxRawDataLength, MaxAdditionalAttributes))]
pub struct EntityInfo<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> {
	pub display: Element<MaxRawDataLength>,
	pub legal: Element<MaxRawDataLength>,
	pub web: Element<MaxRawDataLength>,
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
		if !self.legal.is_none() {
			bits.insert(EntityField::Legal)
		}
		if !self.web.is_none() {
			bits.insert(EntityField::Web)
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
	DokenInformationProvider for EntityInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	type FieldMask = u64;
	type MaxRawDataLength = MaxRawDataLength;
	type MaxAdditionalAttributes = MaxAdditionalAttributes;
	type UpdateOp = DokenUpdateOp<MaxRawDataLength>;

	fn attributes(
		&self,
	) -> Option<&Attributes<Self::MaxRawDataLength, Self::MaxAdditionalAttributes>> {
		self.attributes.as_ref()
	}

	fn get_key(&self, key: &[u8]) -> Element<Self::MaxRawDataLength> {
		if let Some(field) = EntityField::from_bytes(key) {
			return match field {
				EntityField::Display => self.display.clone(),
				EntityField::Legal => self.legal.clone(),
				EntityField::Web => self.web.clone(),
				EntityField::Attributes => Element::default(),
			};
		}
		self.attributes
			.as_ref()
			.and_then(|attrs| {
				attrs.iter().find(|(k, _)| k.as_slice() == key).map(|(_, v)| v.clone())
			})
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
		op: &DokenUpdateOp<MaxRawDataLength>,
	) -> Result<(), DokenUpdateError> {
		match op {
			DokenUpdateOp::AddAttribute(k, v) => {
				ensure!(EntityField::from_bytes(k).is_none(), DokenUpdateError::AttributeExists);
				let attrs = self.attributes.get_or_insert_with(Default::default);
				if attrs.iter().any(|(kk, _)| kk == k) {
					return Err(DokenUpdateError::AttributeExists);
				}
				attrs
					.try_push((k.clone(), v.clone()))
					.map_err(|_| DokenUpdateError::TooManyAttributes)
			},

			DokenUpdateOp::RemoveAttribute(k) => {
				if let Some(field) = EntityField::from_bytes(k) {
					match field {
						EntityField::Display => self.display = Element::default(),
						EntityField::Legal => self.legal = Element::default(),
						EntityField::Web => self.web = Element::default(),
						EntityField::Attributes => {
							return Err(DokenUpdateError::AttributeNotFound);
						},
					}
				} else {
					let attrs =
						self.attributes.as_mut().ok_or(DokenUpdateError::AttributeNotFound)?;
					let idx = attrs
						.iter()
						.position(|(kk, _)| kk == k)
						.ok_or(DokenUpdateError::AttributeNotFound)?;
					attrs.swap_remove(idx);
					if attrs.is_empty() {
						self.attributes = None;
					}
				}
				Ok(())
			},

			DokenUpdateOp::UpdateAttribute(k, v) => {
				if let Some(field) = EntityField::from_bytes(k) {
					match field {
						EntityField::Display => self.display = v.clone(),
						EntityField::Legal => self.legal = v.clone(),
						EntityField::Web => self.web = v.clone(),
						EntityField::Attributes => {
							return Err(DokenUpdateError::AttributeNotFound);
						},
					}
				} else {
					let attrs =
						self.attributes.as_mut().ok_or(DokenUpdateError::AttributeNotFound)?;
					let slot = attrs
						.iter_mut()
						.find(|(kk, _)| kk == k)
						.ok_or(DokenUpdateError::AttributeNotFound)?;
					slot.1 = v.clone();
				}
				Ok(())
			},
		}
	}

	fn create_info() -> Self {
		let empty = Element::default();
		let cap = MaxAdditionalAttributes::get() as usize;
		let mut attrs = Vec::with_capacity(cap);
		for i in 0..cap {
			let key: Attribute = vec![b'k', i as u8].try_into().unwrap();
			attrs.push((key, empty.clone()));
		}
		EntityInfo {
			display: empty.clone(),
			legal: empty.clone(),
			web: empty.clone(),
			attributes: Some(attrs.try_into().unwrap()),
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
			legal: Element::default(),
			web: Element::default(),
			attributes: None,
		}
	}
}
