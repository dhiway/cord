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

//! Identity pallet types.

use super::*;
use codec::{Decode, Encode, MaxEncodedLen};
use enumflags2::{bitflags, BitFlags};
use frame_support::{
	traits::{ConstU32, Get},
	BoundedVec, CloneNoBound, PartialEqNoBound, RuntimeDebugNoBound,
};
use scale_info::{
	build::{Fields, Variants},
	meta_type, Path, Type, TypeInfo, TypeParameter,
};
use sp_runtime::RuntimeDebug;
use sp_std::{fmt::Debug, iter::once, prelude::*};

/// Either underlying data blob if it is at most 32 bytes, or a hash of it. If
/// the data is greater than 32-bytes then it will be truncated when encoding.
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

		Type::builder().path(Path::new("Data", module_path!())).variant(variants)
	}
}

impl Default for Data {
	fn default() -> Self {
		Self::None
	}
}

/// An attestation of a registrar over how accurate some `IdentityInfo` is in
/// describing an account.
#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum Judgement {
	/// The default value; no opinion is held.
	Unknown,
	/// No judgement is yet in place, but a request is placed for providing one.
	Requested,
	/// The data appears to be reasonably acceptable in terms of its accuracy,
	/// however no in depth checks (such as in-person meetings or formal KYC)
	/// have been conducted.
	Reasonable,
	/// The target is known directly by the registrar and the registrar can
	/// fully attest to the the data's accuracy.
	KnownGood,
	/// The data was once good but is currently out of date. There is no
	/// malicious intent in the inaccuracy. This judgement can be removed
	/// through updating the data.
	OutOfDate,
	/// The data is imprecise or of sufficiently low-quality to be problematic.
	/// It is not indicative of malicious intent. This judgement can be removed
	/// through updating the data.
	LowQuality,
	/// The data is erroneous. This may be indicative of malicious intent. This
	/// cannot be removed except by the registrar.
	Erroneous,
}

impl Judgement {
	/// Returns `true` if this judgement is indicative of a request being
	/// placed.
	pub(crate) fn has_requested(&self) -> bool {
		matches!(self, Judgement::Requested)
	}

	/// Returns `true` if this judgement is one that should not be generally be
	/// replaced outside of specialized handlers.
	pub(crate) fn is_sticky(&self) -> bool {
		matches!(self, Judgement::Requested | Judgement::Erroneous)
	}
}

/// The fields that we use to identify the owner of an account with. Each
/// corresponds to a field in the `IdentityInfo` struct.
#[bitflags]
#[repr(u64)]
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum IdentityField {
	Display = 0b0000000000000000000000000000000000000000000000000000000000000001,
	Legal = 0b0000000000000000000000000000000000000000000000000000000000000010,
	Web = 0b0000000000000000000000000000000000000000000000000000000000000100,
	Email = 0b0000000000000000000000000000000000000000000000000000000000001000,
}

/// Wrapper type for `BitFlags<IdentityField>` that implements `Codec`.
#[derive(Clone, Copy, PartialEq, Default, RuntimeDebug)]
pub struct IdentityFields(pub BitFlags<IdentityField>);

impl MaxEncodedLen for IdentityFields {
	fn max_encoded_len() -> usize {
		u64::max_encoded_len()
	}
}

impl Eq for IdentityFields {}
impl Encode for IdentityFields {
	fn using_encoded<R, F: FnOnce(&[u8]) -> R>(&self, f: F) -> R {
		self.0.bits().using_encoded(f)
	}
}
impl Decode for IdentityFields {
	fn decode<I: codec::Input>(input: &mut I) -> sp_std::result::Result<Self, codec::Error> {
		let field = u64::decode(input)?;
		Ok(Self(<BitFlags<IdentityField>>::from_bits(field).map_err(|_| "invalid value")?))
	}
}
impl TypeInfo for IdentityFields {
	type Identity = Self;

	fn type_info() -> Type {
		Type::builder()
			.path(Path::new("BitFlags", module_path!()))
			.type_params(vec![TypeParameter::new("T", Some(meta_type::<IdentityField>()))])
			.composite(Fields::unnamed().field(|f| f.ty::<u64>().type_name("IdentityField")))
	}
}

/// Information concerning the identity of the controller of an account.
///
/// NOTE: This should be stored at the end of the storage item to facilitate the
/// addition of extra fields in a backwards compatible way through a specialized
/// `Decode` impl.
#[derive(
	CloneNoBound, Encode, Decode, Eq, MaxEncodedLen, PartialEqNoBound, RuntimeDebugNoBound, TypeInfo,
)]
#[codec(mel_bound())]
#[cfg_attr(test, derive(frame_support::DefaultNoBound))]
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
}

impl<FieldLimit: Get<u32>> IdentityInfo<FieldLimit> {
	pub(crate) fn fields(&self) -> IdentityFields {
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
		IdentityFields(res)
	}
}

/// Information concerning the identity of the controller of an account.
///
/// NOTE: This is stored separately primarily to facilitate the addition of
/// extra fields in a backwards compatible way through a specialized `Decode`
/// impl.
#[derive(
	CloneNoBound, Encode, Eq, MaxEncodedLen, PartialEqNoBound, RuntimeDebugNoBound, TypeInfo,
)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(MaxJudgements, MaxAdditionalFields))]
pub struct Registration<
	AccountId: Encode + Decode + Clone + Debug + Eq + PartialEq + MaxEncodedLen,
	MaxJudgements: Get<u32>,
	MaxAdditionalFields: Get<u32>,
> {
	/// Judgements from the registrars on this identity. Stored ordered by
	/// `RegistrarIndex`. There may be only a single judgement from each
	/// registrar.
	pub judgements: BoundedVec<(AccountId, Judgement), MaxJudgements>,

	/// Information on the identity.
	pub info: IdentityInfo<MaxAdditionalFields>,
}

impl<
		AccountId: Encode + Decode + Clone + Debug + Eq + PartialEq + MaxEncodedLen,
		MaxJudgements: Get<u32>,
		MaxAdditionalFields: Get<u32>,
	> Decode for Registration<AccountId, MaxJudgements, MaxAdditionalFields>
{
	fn decode<I: codec::Input>(input: &mut I) -> sp_std::result::Result<Self, codec::Error> {
		let (judgements, info) = Decode::decode(&mut AppendZerosInput::new(input))?;
		Ok(Self { judgements, info })
	}
}

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
				Data::Raw(bytes) => format!("Raw{}", bytes.len()),
				Data::BlakeTwo256(_) => "BlakeTwo256".to_string(),
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

		let mut data = vec![Data::None, Data::BlakeTwo256(Default::default())];

		// A Raw instance for all possible sizes of the Raw data
		for n in 0..32 {
			data.push(Data::Raw(vec![0u8; n as usize].try_into().unwrap()))
		}

		for d in data.iter() {
			check_type_info(d);
		}
	}
}
