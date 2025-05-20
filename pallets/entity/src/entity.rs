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

#[cfg(feature = "runtime-benchmarks")]
use crate::types::Attribute;
use crate::types::{
	Attributes, Data, EntityInformationProvider, EntityUpdateError, EntityUpdateOp,
};
#[cfg(feature = "runtime-benchmarks")]
use alloc::vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
#[cfg(feature = "runtime-benchmarks")]
use enumflags2::BitFlag;
use enumflags2::{bitflags, BitFlags};
use frame_support::{
	ensure, traits::Get, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound,
};
use scale_info::{build::Variants, Path, Type, TypeInfo};

/// Each field corresponds to a field in the `EntityInfo` struct.
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
	Email,
	Twitter,
	Attributes,
}

impl TypeInfo for EntityField {
	type Identity = Self;
	fn type_info() -> scale_info::Type {
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

/// Information concerning the entity of the controller of an account.
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
#[scale_info(skip_type_params(MaxRawDataLength))]
pub struct EntityInfo<MaxRawDataLength: Get<u32>> {
	pub display: Data<MaxRawDataLength>,
	pub legal: Data<MaxRawDataLength>,
	pub web: Data<MaxRawDataLength>,
	pub email: Data<MaxRawDataLength>,
	pub twitter: Data<MaxRawDataLength>,
	pub attributes: Option<Attributes<MaxRawDataLength>>,
}

impl<MaxRawDataLength: Get<u32>> EntityInfo<MaxRawDataLength> {
	pub(crate) fn fields(&self) -> BitFlags<EntityField> {
		let mut bits = BitFlags::empty();
		if !self.display.is_none() {
			bits.insert(EntityField::Display);
		}
		if !self.legal.is_none() {
			bits.insert(EntityField::Legal);
		}
		if !self.web.is_none() {
			bits.insert(EntityField::Web);
		}
		if !self.email.is_none() {
			bits.insert(EntityField::Email);
		}
		if !self.twitter.is_none() {
			bits.insert(EntityField::Twitter);
		}
		if let Some(attrs) = &self.attributes {
			if !attrs.is_empty() {
				bits.insert(EntityField::Attributes);
			}
		}
		bits
	}
}

impl<MaxRawDataLength: Get<u32> + 'static> EntityInformationProvider
	for EntityInfo<MaxRawDataLength>
{
	type FieldsIdentifier = u64;
	type MaxRawDataLength = MaxRawDataLength;
	type UpdateOp = EntityUpdateOp<MaxRawDataLength>;

	fn attributes(&self) -> Option<&Attributes<Self::MaxRawDataLength>> {
		self.attributes.as_ref()
	}

	fn get_key(&self, key: &[u8]) -> Data<Self::MaxRawDataLength> {
		match key {
			b"display" => self.display.clone(),
			b"legal" => self.legal.clone(),
			b"web" => self.web.clone(),
			b"email" => self.email.clone(),
			b"twitter" => self.twitter.clone(),
			_ => self
				.attributes
				.as_ref()
				.and_then(|attrs| {
					attrs
						.iter()
						// compare the raw byte‐slices
						.find(|(k, _)| &k[..] == &key[..])
						.map(|(_, v)| v.clone())
				})
				.unwrap_or(Data::None),
		}
	}

	fn present_fields(&self) -> Self::FieldsIdentifier {
		self.fields().bits()
	}

	fn has_info_fields(&self, fields: Self::FieldsIdentifier) -> bool {
		self.present_fields() & fields == fields
	}

	fn apply_update(
		&mut self,
		op: &EntityUpdateOp<MaxRawDataLength>,
	) -> Result<(), EntityUpdateError> {
		match op {
			EntityUpdateOp::AddAttribute(k, v) => {
				// reserved keys cannot be used
				let b = k.as_slice();
				ensure!(
					!matches!(b, b"display" | b"legal" | b"web" | b"email" | b"twitter"),
					EntityUpdateError::AttributeExists
				);
				let attrs = self.attributes.get_or_insert_with(Default::default);
				if attrs.iter().any(|(kk, _)| kk == k) {
					return Err(EntityUpdateError::AttributeExists);
				}
				attrs
					.try_push((k.clone(), v.clone()))
					.map_err(|_| EntityUpdateError::TooManyAttributes)
			},
			EntityUpdateOp::RemoveAttribute(k) => {
				match k.as_slice() {
					b"display" => self.display = Data::None,
					b"legal" => self.legal = Data::None,
					b"web" => self.web = Data::None,
					b"email" => self.email = Data::None,
					b"twitter" => self.twitter = Data::None,
					_ => {
						let attrs =
							self.attributes.as_mut().ok_or(EntityUpdateError::AttributeNotFound)?;
						let idx = attrs
							.iter()
							.position(|(kk, _)| kk == k)
							.ok_or(EntityUpdateError::AttributeNotFound)?;
						attrs.swap_remove(idx);
						if attrs.is_empty() {
							self.attributes = None;
						}
					},
				}
				Ok(())
			},
			EntityUpdateOp::UpdateAttribute(k, v) => {
				match k.as_slice() {
					b"display" => self.display = v.clone(),
					b"legal" => self.legal = v.clone(),
					b"web" => self.web = v.clone(),
					b"email" => self.email = v.clone(),
					b"twitter" => self.twitter = v.clone(),
					_ => {
						let attrs =
							self.attributes.as_mut().ok_or(EntityUpdateError::AttributeNotFound)?;
						let slot = attrs
							.iter_mut()
							.find(|(kk, _)| kk == k)
							.ok_or(EntityUpdateError::AttributeNotFound)?;
						slot.1 = v.clone();
					},
				}
				Ok(())
			},
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn create_entity_info() -> Self {
		let empty = Data::<MaxRawDataLength>::Raw(Default::default());
		let mut attrs = Vec::new();
		const ATTR_CAP: usize = 32;
		for i in 0..ATTR_CAP {
			// e.g. keys "k0", "k1", ... "k31"
			let raw_key = vec![b'k', i as u8];
			let bounded_key: Attribute = raw_key.clone().try_into().unwrap();
			attrs.push((bounded_key, empty.clone()));
		}
		EntityInfo {
			display: empty.clone(),
			legal: empty.clone(),
			web: empty.clone(),
			email: empty.clone(),
			twitter: empty.clone(),
			attributes: Some(attrs.try_into().unwrap()),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn all_fields() -> Self::FieldsIdentifier {
		EntityField::all().bits()
	}
}

impl<MaxRawDataLength: Get<u32>> Default for EntityInfo<MaxRawDataLength> {
	fn default() -> Self {
		EntityInfo {
			display: Data::<MaxRawDataLength>::None,
			legal: Data::<MaxRawDataLength>::None,
			web: Data::<MaxRawDataLength>::None,
			email: Data::<MaxRawDataLength>::None,
			twitter: Data::<MaxRawDataLength>::None,
			attributes: None,
		}
	}
}
