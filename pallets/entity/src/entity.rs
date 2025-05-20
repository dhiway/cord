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

use crate::types::{
	Attributes, Data, EntityInformationProvider, EntityUpdateError, EntityUpdateOp,
};
#[cfg(feature = "runtime-benchmarks")]
use alloc::vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
#[cfg(feature = "runtime-benchmarks")]
use enumflags2::BitFlag;
use enumflags2::{bitflags, BitFlags};
use frame_support::{traits::Get, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound};
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
#[scale_info(skip_type_params(MaxRaw))]
pub struct EntityInfo<MaxRaw: Get<u32>> {
	pub display: Data<MaxRaw>,
	pub legal: Data<MaxRaw>,
	pub web: Data<MaxRaw>,
	pub email: Data<MaxRaw>,
	pub twitter: Data<MaxRaw>,
	pub attributes: Attributes<MaxRaw>,
}

impl<MaxRaw: Get<u32>> EntityInfo<MaxRaw> {
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
		if !self.attributes.is_empty() {
			bits.insert(EntityField::Attributes);
		}
		bits
	}
}

impl<MaxRaw: Get<u32> + 'static> EntityInformationProvider for EntityInfo<MaxRaw> {
	type FieldsIdentifier = u64;
	type MaxRaw = MaxRaw;
	type UpdateOp = EntityUpdateOp<MaxRaw>;

	fn attributes(&self) -> &Attributes<Self::MaxRaw> {
		&self.attributes
	}

	fn present_fields(&self) -> Self::FieldsIdentifier {
		self.fields().bits()
	}

	fn has_info_fields(&self, fields: Self::FieldsIdentifier) -> bool {
		self.present_fields() & fields == fields
	}

	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), EntityUpdateError> {
		match op {
			EntityUpdateOp::SetField(field, val) => {
				match field {
					EntityField::Display => self.display = val.clone(),
					EntityField::Legal => self.legal = val.clone(),
					EntityField::Web => self.web = val.clone(),
					EntityField::Email => self.email = val.clone(),
					EntityField::Twitter => self.twitter = val.clone(),
					EntityField::Attributes => return Err(EntityUpdateError::AttributeExists),
				}
				Ok(())
			},
			EntityUpdateOp::AddAttribute(k, v) => {
				if self.attributes.iter().any(|(kk, _)| kk == k) {
					return Err(EntityUpdateError::AttributeExists);
				}
				self.attributes
					.try_push((k.clone(), v.clone()))
					.map_err(|_| EntityUpdateError::TooManyAttributes)?;
				Ok(())
			},
			EntityUpdateOp::UpdateAttribute(k, v) => {
				if let Some((_, val)) = self.attributes.iter_mut().find(|(kk, _)| kk == k) {
					*val = v.clone();
					Ok(())
				} else {
					Err(EntityUpdateError::AttributeNotFound)
				}
			},
			EntityUpdateOp::RemoveAttribute(k) => {
				if let Some(i) = self.attributes.iter().position(|(kk, _)| kk == k) {
					self.attributes.swap_remove(i);
					Ok(())
				} else {
					Err(EntityUpdateError::AttributeNotFound)
				}
			},
			EntityUpdateOp::ClearAttribute => {
				self.attributes.clear();
				Ok(())
			},
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn create_entity_info() -> Self {
		let empty = Data::<MaxRaw>::Raw(Default::default());
		let mut all = Vec::new();
		let cap: usize = FieldLimit::get().try_into().unwrap();
		for _ in 0..cap {
			all.push((Attribute::default(), empty.clone()));
		}
		EntityInfo {
			display: empty.clone(),
			legal: empty.clone(),
			web: empty.clone(),
			email: empty.clone(),
			twitter: empty.clone(),
			attributes: all.try_into().unwrap(),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn all_fields() -> Self::FieldsIdentifier {
		EntityField::all().bits()
	}
}

impl<MaxRaw: Get<u32>> Default for EntityInfo<MaxRaw> {
	fn default() -> Self {
		EntityInfo {
			display: Data::<MaxRaw>::None,
			legal: Data::<MaxRaw>::None,
			web: Data::<MaxRaw>::None,
			email: Data::<MaxRaw>::None,
			twitter: Data::<MaxRaw>::None,
			attributes: Default::default(),
		}
	}
}
