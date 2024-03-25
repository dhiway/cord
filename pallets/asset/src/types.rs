// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
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

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

use crate::AssetQtyOf;

pub type EntryHashOf<T> = <T as frame_system::Config>::Hash;

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct AssetInputEntry<AssetDescription, AssetTypeOf, AssetTag, AssetMeta> {
	/// type of the asset
	pub asset_type: AssetTypeOf,
	/// asset description
	pub asset_desc: AssetDescription,
	/// asset quantity
	pub asset_qty: u64,
	/// asset value
	pub asset_value: u32,
	/// open structure - 1024 bytes max
	pub asset_tag: AssetTag,
	/// open structure - 1024 bytes max
	pub asset_meta: AssetMeta,
}
#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum AssetTypeOf {
	ART,
	BOND,
	MF,
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum AssetStatusOf {
	ACTIVE,
	INACTIVE,
	EXPIRED,
}

impl AssetTypeOf {
	pub fn is_valid_asset_type(&self) -> bool {
		matches!(self, Self::ART | Self::BOND | Self::MF)
	}
}

impl AssetStatusOf {
	pub fn is_valid_status_type(&self) -> bool {
		matches!(self, Self::ACTIVE | Self::INACTIVE | Self::EXPIRED)
	}
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct AssetEntry<
	AssetDescription,
	AssetTypeOf,
	AssetStatusOf,
	AssetCreatorOf,
	AssetTag,
	AssetMeta,
	BlockNumber,
> {
	pub asset_detail: AssetInputEntry<AssetDescription, AssetTypeOf, AssetTag, AssetMeta>,
	/// asset issuance count
	pub asset_issuance: u64,
	/// status of the asset
	pub asset_status: AssetStatusOf,
	/// asset issuer
	pub asset_issuer: AssetCreatorOf,
	/// asset inlclusion block
	pub created_at: BlockNumber,
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct VCAssetEntry<AssetStatusOf, AssetCreatorOf, BlockNumber, EntryHashOf> {
	/// digest of the input entry
	pub digest: EntryHashOf,
	/// asset issuance count
	pub asset_issuance: u64,
	/// status of the asset
	pub asset_status: AssetStatusOf,
	/// asset issuer
	pub asset_issuer: AssetCreatorOf,
	/// asset quantity
	pub asset_qty: AssetQtyOf,
	/// asset inlclusion block
	pub created_at: BlockNumber,
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct AssetDistributionEntry<
	AssetDescription,
	AssetTypeOf,
	AssetStatusOf,
	AssetCreatorOf,
	AssetTag,
	AssetMeta,
	BlockNumber,
	AssetId,
> {
	pub asset_instance_detail: AssetInputEntry<AssetDescription, AssetTypeOf, AssetTag, AssetMeta>,
	/// asset parent reference
	pub asset_instance_parent: AssetId,
	/// status of the asset
	pub asset_instance_status: AssetStatusOf,
	/// asset issuer
	pub asset_instance_issuer: AssetCreatorOf,
	/// asset owner
	pub asset_instance_owner: AssetCreatorOf,
	/// asset inlclusion block
	pub created_at: BlockNumber,
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct VCAssetDistributionEntry<AssetStatusOf, AssetCreatorOf, BlockNumber, AssetId> {
	pub asset_qty: AssetQtyOf,
	/// asset parent reference
	pub asset_instance_parent: AssetId,
	/// status of the asset
	pub asset_instance_status: AssetStatusOf,
	/// asset issuer
	pub asset_instance_issuer: AssetCreatorOf,
	/// asset owner
	pub asset_instance_owner: AssetCreatorOf,
	/// asset inlclusion block
	pub created_at: BlockNumber,
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct AssetIssuanceEntry<AssetIdOf, AssetCreatorOf> {
	/// type of the asset
	pub asset_id: AssetIdOf,
	/// asset owner
	pub asset_owner: AssetCreatorOf,
	/// issuance quantity
	pub asset_issuance_qty: Option<u64>,
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct AssetTransferEntry<AssetIdOf, AssetInstanceIdOf, AssetCreatorOf> {
	/// type of the asset
	pub asset_id: AssetIdOf,
	/// asset instance identifier
	pub asset_instance_id: AssetInstanceIdOf,
	/// asset owner
	pub asset_owner: AssetCreatorOf,
	/// new asset owner
	pub new_asset_owner: AssetCreatorOf,
}

//This test case will help ensure that the InvalidDigest error is properly handled in the Asset pallet
// when attempting to create an asset with an invalid digest.
#[test]
fn asset_create_should_fail_invalid_digest() {
    let creator = DID_00;
    let author = ACCOUNT_00;
    let capacity = 5u64;

    let raw_space = [2u8; 256].to_vec();
    let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
    let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
        &[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
    );
    let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

    let auth_digest = <Test as frame_system::Config>::Hashing::hash(
        &[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
    );
    let authorization_id: Ss58Identifier =
        generate_authorization_id::<Test>(&auth_digest);

    let asset_desc = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
    let asset_tag = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
    let asset_meta = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
    let asset_qty = 10;
    let asset_value = 10;
    let asset_type = AssetTypeOf::MF;

    let entry = AssetInputEntryOf::<Test> {
        asset_desc,
        asset_qty,
        asset_type,
        asset_value,
        asset_tag,
        asset_meta,
    };

    // Create an invalid digest (e.g., an empty vector)
    let invalid_digest: Vec<u8> = vec![];

    new_test_ext().execute_with(|| {
        assert_ok!(Space::create(
            DoubleOrigin(author.clone(), creator.clone()).into(),
            space_digest,
        ));

        assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, capacity));

        // Attempt to create asset with invalid digest
        assert_err!(
            Asset::create(
                DoubleOrigin(author.clone(), creator.clone()).into(),
                entry,
                invalid_digest,
                authorization_id,
            ),
            Error::<Test>::InvalidDigest
        );
    });
}

