// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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
//

use codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "runtime-benchmarks")]
use enumflags2::BitFlag;
use enumflags2::{bitflags, BitFlags};
use frame_support::{traits::Get, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound};
use scale_info::{build::Variants, Path, Type, TypeInfo};
use sp_runtime::{BoundedVec, RuntimeDebug};
use sp_std::prelude::*;

use crate::types::{Data, IdentityInformationProvider};

/// The fields that we use to identify the owner of an account with. Each
/// corresponds to a field in the `IdentityInfo` struct.
#[bitflags]
#[repr(u64)]
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub enum IdentityField {
	Display,
	Legal,
	Web,
	Email,
	Image,
}

impl TypeInfo for IdentityField {
	type Identity = Self;

	fn type_info() -> scale_info::Type {
		Type::builder().path(Path::new("IdentityField", module_path!())).variant(
			Variants::new()
				.variant("Display", |v| v.index(0))
				.variant("Legal", |v| v.index(1))
				.variant("Web", |v| v.index(2))
				.variant("Email", |v| v.index(3))
				.variant("Image", |v| v.index(4)),
		)
	}
}

/// Information concerning the identity of the controller of an account.
///
/// NOTE: This should be stored at the end of the storage item to facilitate the
/// addition of extra fields in a backwards compatible way through a specialized
/// `Decode` impl.
#[derive(
	CloneNoBound,
	Encode,
	Decode,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	RuntimeDebugNoBound,
	TypeInfo,
)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(FieldLimit))]
pub struct IdentityInfo<FieldLimit: Get<u32>> {
	/// Additional fields of the identity that are not catered for with the
	/// struct's explicit fields.
	pub additional: BoundedVec<(Data, Data), FieldLimit>,

	/// A reasonable display name for the controller of the account. This should
	/// be whatever it is that it is typically known as and should not be
	/// confusable with other entities, given reasonable context.
	///
	/// Stored as UTF-8.
	pub display: Data,

	/// The full legal name in the local jurisdiction of the entity. This might
	/// be a bit long-winded.
	///
	/// Stored as UTF-8.
	pub legal: Data,

	/// A representative website held by the controller of the account.
	///
	/// NOTE: `https://` is automatically prepended.
	///
	/// Stored as UTF-8.
	pub web: Data,

	/// The email address of the controller of the account.
	///
	/// Stored as UTF-8.
	pub email: Data,
	/// A graphic image representing the controller of the account. Should be a company,
	/// organization or project logo or a headshot in the case of a human.
	pub image: Data,
}

impl<FieldLimit: Get<u32> + 'static> IdentityInformationProvider for IdentityInfo<FieldLimit> {
	type FieldsIdentifier = u64;

	fn has_identity(&self, fields: Self::FieldsIdentifier) -> bool {
		self.fields().bits() & fields == fields
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn create_identity_info() -> Self {
		let data = Data::Raw(vec![0; 32].try_into().unwrap());

		IdentityInfo {
			additional: vec![(data.clone(), data.clone()); FieldLimit::get().try_into().unwrap()]
				.try_into()
				.unwrap(),
			display: data.clone(),
			legal: data.clone(),
			web: data.clone(),
			email: data.clone(),
			image: data,
		}
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn all_fields() -> Self::FieldsIdentifier {
		IdentityField::all().bits()
	}
}
impl<FieldLimit: Get<u32>> Default for IdentityInfo<FieldLimit> {
	fn default() -> Self {
		IdentityInfo {
			additional: BoundedVec::default(),
			display: Data::None,
			legal: Data::None,
			web: Data::None,
			email: Data::None,
			image: Data::None,
		}
	}
}

impl<FieldLimit: Get<u32>> IdentityInfo<FieldLimit> {
	pub(crate) fn fields(&self) -> BitFlags<IdentityField> {
		let mut res = <BitFlags<IdentityField>>::empty();
		if !self.display.is_none() {
			res.insert(IdentityField::Display);
		}
		if !self.legal.is_none() {
			res.insert(IdentityField::Legal);
		}
		if !self.web.is_none() {
			res.insert(IdentityField::Web);
		}
		if !self.email.is_none() {
			res.insert(IdentityField::Email);
		}
		if !self.image.is_none() {
			res.insert(IdentityField::Image);
		}
		res
	}
}
