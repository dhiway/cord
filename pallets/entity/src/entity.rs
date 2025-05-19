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
use alloc::vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::fmt::Debug;
#[cfg(feature = "runtime-benchmarks")]
use enumflags2::BitFlag;
use enumflags2::{bitflags, BitFlags};
use frame_support::{traits::Get, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound};
use scale_info::{build::Variants, Path, Type, TypeInfo};
use sp_runtime::BoundedVec;

use crate::types::{
	Attribute, Attributes, Data, EntityInformationProvider, EntityUpdateError, EntityUpdateOp,
	ProfileCid,
};
/// Each field corresponds to a field in the `EntityInfo` struct.
#[bitflags]
#[repr(u64)]
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebugNoBound)]
pub enum EntityField {
	Display,
	Legal,
	Web,
	Profile,
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
				.variant("Profile", |v| v.index(3))
				.variant("Attributes", |v| v.index(4)),
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
#[scale_info(skip_type_params(FieldLimit, RawLimit))]
pub struct EntityInfo<FieldLimit: Get<u32>, RawLimit: Get<u32>> {
	pub display: Data<RawLimit>,
	pub legal: Data<RawLimit>,
	pub web: Data<RawLimit>,
	pub profile: Option<ProfileCid>,
	pub attributes: Attributes<FieldLimit, RawLimit>,
}

impl<FieldLimit: Get<u32>, DataLimit: Get<u32>> EntityInfo<FieldLimit, DataLimit> {
	pub fn set_attributes(&mut self, new: BoundedVec<(Attribute, Data<DataLimit>), FieldLimit>) {
		self.attributes = new;
	}

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
		if self.profile.is_some() {
			bits.insert(EntityField::Profile);
		}
		if !self.attributes.is_empty() {
			bits.insert(EntityField::Attributes);
		}
		bits
	}
}

impl<
		FieldLimit: Get<u32> + 'static,
		DataLimit: Get<u32> + Clone + PartialEq + Debug + 'static + TypeInfo,
	> EntityInformationProvider for EntityInfo<FieldLimit, DataLimit>
{
	type FieldsIdentifier = u64;
	type FieldLimit = FieldLimit;
	type DataLimit = DataLimit;
	type UpdateOp = EntityUpdateOp<DataLimit>;

	fn attributes(&self) -> &Attributes<Self::FieldLimit, Self::DataLimit> {
		&self.attributes
	}

	fn present_fields(&self) -> Self::FieldsIdentifier {
		self.fields().bits()
	}

	fn has_info_fields(&self, fields: Self::FieldsIdentifier) -> bool {
		self.fields().bits() & fields == fields
	}

	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), EntityUpdateError> {
		match op {
			EntityUpdateOp::SetDisplay(x) => {
				self.display = x.clone();
				Ok(())
			},
			EntityUpdateOp::SetLegal(x) => {
				self.legal = x.clone();
				Ok(())
			},
			EntityUpdateOp::SetWeb(x) => {
				self.web = x.clone();
				Ok(())
			},
			EntityUpdateOp::SetProfile(o) => {
				self.profile = o.clone();
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
		let empty = Data::<DataLimit>::Raw(Default::default());
		let mut all = Vec::new();
		let cap: usize = FieldLimit::get().try_into().unwrap();
		for _ in 0..cap {
			all.push((Attribute::default(), empty.clone()));
		}
		EntityInfo {
			display: empty.clone(),
			legal: empty.clone(),
			web: empty.clone(),
			profile: Some(ProfileCid([0u8; 64])),
			attributes: all.try_into().unwrap(),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn all_fields() -> Self::FieldsIdentifier {
		EntityField::all().bits()
	}
}

impl<FieldLimit: Get<u32>, RawLimit: Get<u32>> Default for EntityInfo<FieldLimit, RawLimit> {
	fn default() -> Self {
		EntityInfo {
			display: Data::None,
			legal: Data::None,
			web: Data::None,
			profile: None,
			attributes: Default::default(),
		}
	}
}
