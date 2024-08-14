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

//! DeDir pallet types.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::ConstU32, BoundedVec};
use scale_info::{
	build::{Fields, Variants},
	Path, Type, TypeInfo,
};
use sp_runtime::RuntimeDebug;
use sp_std::{iter::once, prelude::*};

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum RegistrySupportedStateOf {
	DRAFT,
	ACTIVE,
	REVOKED,
}

impl RegistrySupportedStateOf {
	pub fn is_valid_state(&self) -> bool {
		matches!(self, Self::DRAFT | Self::ACTIVE | Self::REVOKED)
	}
}

/// Either underlying data blob if it is at most 32 bytes, or a hash of it. If
/// the data is greater than 32-bytes then it will be truncated when encoding.
///
/// Can also be `None`.
#[derive(Clone, Eq, PartialEq, RuntimeDebug, MaxEncodedLen)]
pub enum Data {
	/// No data here.
	None,
	/// The data is stored directly.
	Raw(BoundedVec<u8, ConstU32<32>>),
	/// Only the Blake2 hash of the data is stored. The preimage of the hash may
	/// be retrieved through some hash-lookup service.
	BlakeTwo256([u8; 32]),
}

impl Data {
	pub fn is_none(&self) -> bool {
		self == &Data::None
	}
}

impl Decode for Data {
	fn decode<I: codec::Input>(input: &mut I) -> sp_std::result::Result<Self, codec::Error> {
		let b = input.read_byte()?;
		Ok(match b {
			0 => Data::None,
			n @ 1..=33 => {
				let mut r: BoundedVec<_, _> = vec![0u8; n as usize - 1]
					.try_into()
					.expect("bound checked in match arm condition; qed");
				input.read(&mut r[..])?;
				Data::Raw(r)
			},
			34 => Data::BlakeTwo256(<[u8; 32]>::decode(input)?),
			// 35 => Data::Sha256(<[u8; 32]>::decode(input)?),
			// 36 => Data::Keccak256(<[u8; 32]>::decode(input)?),
			// 37 => Data::ShaThree256(<[u8; 32]>::decode(input)?),
			_ => return Err(codec::Error::from("invalid leading byte")),
		})
	}
}

impl Encode for Data {
	fn encode(&self) -> Vec<u8> {
		match self {
			Data::None => vec![0u8; 1],
			Data::Raw(ref x) => {
				let l = x.len().min(32);
				let mut r = vec![l as u8 + 1; l + 1];
				r[1..].copy_from_slice(&x[..l]);
				r
			},
			Data::BlakeTwo256(ref h) => once(34u8).chain(h.iter().cloned()).collect(),
			// Data::Sha256(ref h) => once(35u8).chain(h.iter().cloned()).collect(),
			// Data::Keccak256(ref h) => once(36u8).chain(h.iter().cloned()).collect(),
			// Data::ShaThree256(ref h) => once(37u8).chain(h.iter().cloned()).collect(),
		}
	}
}
impl codec::EncodeLike for Data {}

/// Add a Raw variant with the given index and a fixed sized byte array
macro_rules! data_raw_variants {
    ($variants:ident, $(($index:literal, $size:literal)),* ) => {
		$variants
		$(
			.variant(concat!("Raw", stringify!($size)), |v| v
				.index($index)
				.fields(Fields::unnamed().field(|f| f.ty::<[u8; $size]>()))
			)
		)*
    }
}

impl TypeInfo for Data {
	type Identity = Self;

	fn type_info() -> Type {
		let variants = Variants::new().variant("None", |v| v.index(0));

		// create a variant for all sizes of Raw data from 0-32
		let variants = data_raw_variants!(
			variants,
			(1, 0),
			(2, 1),
			(3, 2),
			(4, 3),
			(5, 4),
			(6, 5),
			(7, 6),
			(8, 7),
			(9, 8),
			(10, 9),
			(11, 10),
			(12, 11),
			(13, 12),
			(14, 13),
			(15, 14),
			(16, 15),
			(17, 16),
			(18, 17),
			(19, 18),
			(20, 19),
			(21, 20),
			(22, 21),
			(23, 22),
			(24, 23),
			(25, 24),
			(26, 25),
			(27, 26),
			(28, 27),
			(29, 28),
			(30, 29),
			(31, 30),
			(32, 31),
			(33, 32)
		);

		let variants = variants.variant("BlakeTwo256", |v| {
			v.index(34).fields(Fields::unnamed().field(|f| f.ty::<[u8; 32]>()))
		});
		// .variant("Sha256", |v| {
		// 	v.index(35).fields(Fields::unnamed().field(|f| f.ty::<[u8; 32]>()))
		// })
		// .variant("Keccak256", |v| {
		// 	v.index(36).fields(Fields::unnamed().field(|f| f.ty::<[u8; 32]>()))
		// })
		// .variant("ShaThree256", |v| {
		// 	v.index(37).fields(Fields::unnamed().field(|f| f.ty::<[u8; 32]>()))
		// });

		Type::builder().path(Path::new("Data", module_path!())).variant(variants)
	}
}

impl Default for Data {
	fn default() -> Self {
		Self::None
	}
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum RegistrySupportedTypeOf {
	Raw,
	BlakeTwo256,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Entry<RegistryKeyIdOf, RegistrySupportedTypeOf> {
	/// Type of Registry Key
	pub registry_key: RegistryKeyIdOf,
	/// Type of Registry Key Type
	pub registry_key_type: RegistrySupportedTypeOf,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Registry<
	RegistryKeyIdOf,
	//MaxRegistryEntries: Get<u32>,
	RegistrySupportedTypeOf,
	RegistryHashOf,
> {
	/// Type of Registry Entry
	pub entries: BoundedVec<
		Entry<RegistryKeyIdOf, RegistrySupportedTypeOf>,
		//MaxRegistryEntries,
		ConstU32<25>,
	>,
	pub digest: RegistryHashOf,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct RegistryEntry<
	RegistryEntryKeyIdOf,
	Data,
	RegistryEntryHashOf,
	RegistryIdOf,
	RegistrySupportedStateOf,
> {
	/// Type of Entries
	pub entries: BoundedVec<(RegistryEntryKeyIdOf, Data), ConstU32<25>>,
	/// Type of Digest
	pub digest: RegistryEntryHashOf,
	/// Type of Registry Identifier
	pub registry_id: RegistryIdOf,
	/// Type of Current State of Registry Entry
	pub current_state: RegistrySupportedStateOf,
}

// impl<RegistryKeyIdOf,
// 	 RegistrySupportedTypeOf,
// 	 RegistryHashOf
// > Registry <RegistryKeyIdOf, RegistrySupportedTypeOf, RegistryHashOf,
// > {
// 	pub fn new() -> Self {
// 		Registry {
// 			entries: BoundedVec::default(),
// 		}
// 	}
// }

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn manual_data_type_info() {
		let mut registry = scale_info::Registry::new();
		let type_id = registry.register_type(&scale_info::meta_type::<Data>());
		let registry: scale_info::PortableRegistry = registry.into();
		let type_info = registry.resolve(type_id.id).unwrap();

		let check_type_info = |data: &Data| {
			let variant_name = match data {
				Data::None => "None".to_string(),
				Data::BlakeTwo256(_) => "BlakeTwo256".to_string(),
				// Data::Sha256(_) => "Sha256".to_string(),
				// Data::Keccak256(_) => "Keccak256".to_string(),
				// Data::ShaThree256(_) => "ShaThree256".to_string(),
				Data::Raw(bytes) => format!("Raw{}", bytes.len()),
			};
			if let scale_info::TypeDef::Variant(variant) = &type_info.type_def {
				let variant = variant
					.variants
					.iter()
					.find(|v| v.name == variant_name)
					.expect(&format!("Expected to find variant {}", variant_name));

				let field_arr_len = variant
					.fields
					.first()
					.and_then(|f| registry.resolve(f.ty.id))
					.map(|ty| {
						if let scale_info::TypeDef::Array(arr) = &ty.type_def {
							arr.len
						} else {
							panic!("Should be an array type")
						}
					})
					.unwrap_or(0);

				let encoded = data.encode();
				assert_eq!(encoded[0], variant.index);
				assert_eq!(encoded.len() as u32 - 1, field_arr_len);
			} else {
				panic!("Should be a variant type")
			};
		};

		let mut data = vec![
			Data::None,
			Data::BlakeTwo256(Default::default()),
			// Data::Sha256(Default::default()),
			// Data::Keccak256(Default::default()),
			// Data::ShaThree256(Default::default()),
		];

		// A Raw instance for all possible sizes of the Raw data
		for n in 0..32 {
			data.push(Data::Raw(vec![0u8; n as usize].try_into().unwrap()))
		}

		for d in data.iter() {
			check_type_info(d);
		}
	}
}
