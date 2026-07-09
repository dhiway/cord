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

//! AssetRate pallet instance tests.

use frame_support::traits::tokens::ConversionFromAssetBalance;
use polkadot_runtime_common::impls::VersionedLocatableAsset;
use xcm::prelude::*;

struct AssetRateWithNative;

impl ConversionFromAssetBalance<u128, VersionedLocatableAsset, u128> for AssetRateWithNative {
	type Error = frame_support::sp_runtime::DispatchError;

	fn from_asset_balance(
		balance: u128,
		asset_id: VersionedLocatableAsset,
	) -> Result<u128, Self::Error> {
		use polkadot_runtime_common::impls::LocatableAssetConverter;
		use sp_runtime::traits::TryConvert;

		let locatable = LocatableAssetConverter::try_convert(asset_id)
			.map_err(|_| frame_support::sp_runtime::DispatchError::Other("invalid asset"))?;
		let is_native_system_para = locatable.asset_id.0 == Location::parent() &&
			matches!(
				locatable.location.unpack(),
				(0, [Parachain(1000)]) | (0, [Parachain(1001)])
			);

		if is_native_system_para {
			Ok(balance)
		} else {
			Err(frame_support::sp_runtime::DispatchError::Other("not native system asset"))
		}
	}
}

#[test]
fn native_asset_rate_works() {
	sp_io::TestExternalities::default().execute_with(|| {
		// success: native asset on Asset Hub as xcm v5 location
		let native = VersionedLocatableAsset::V5 {
			location: Location::new(0, [Parachain(1000)]),
			asset_id: Location::parent().into(),
		};
		let actual = AssetRateWithNative::from_asset_balance(100, native).unwrap();
		assert_eq!(actual, 100);

		// success: native asset on Asset Hub as xcm v4 location
		let native = VersionedLocatableAsset::V4 {
			location: xcm::v4::Location::new(0, [xcm::v4::Junction::Parachain(1000)]),
			asset_id: xcm::v4::Location::parent().into(),
		};
		let actual = AssetRateWithNative::from_asset_balance(100, native).unwrap();
		assert_eq!(actual, 100);

		// success: native asset on Asset Hub as xcm v3 location
		let native = VersionedLocatableAsset::V3 {
			location: xcm::v3::Location::new(
				0,
				xcm::v3::Junctions::X1(xcm::v3::Junction::Parachain(1000)),
			),
			asset_id: xcm::v3::Location::parent().into(),
		};
		let actual = AssetRateWithNative::from_asset_balance(100, native).unwrap();
		assert_eq!(actual, 100);

		// success: native asset on People as xcm v5 location
		let native = VersionedLocatableAsset::V5 {
			location: Location::new(0, [Parachain(1001)]),
			asset_id: Location::parent().into(),
		};
		let actual = AssetRateWithNative::from_asset_balance(100, native).unwrap();
		assert_eq!(actual, 100);

		// success: native asset on People as xcm v4 location
		let native = VersionedLocatableAsset::V4 {
			location: xcm::v4::Location::new(0, [xcm::v4::Junction::Parachain(1001)]),
			asset_id: xcm::v4::Location::parent().into(),
		};
		let actual = AssetRateWithNative::from_asset_balance(100, native).unwrap();
		assert_eq!(actual, 100);

		// success: native asset on People as xcm v3 location
		let native = VersionedLocatableAsset::V3 {
			location: xcm::v3::Location::new(
				0,
				xcm::v3::Junctions::X1(xcm::v3::Junction::Parachain(1001)),
			),
			asset_id: xcm::v3::Location::parent().into(),
		};
		let actual = AssetRateWithNative::from_asset_balance(100, native).unwrap();
		assert_eq!(actual, 100);

		// failure: native asset on non system chain as xcm v5 location
		let native_non_system = VersionedLocatableAsset::V5 {
			location: Location::new(0, [Parachain(2000)]),
			asset_id: Location::parent().into(),
		};
		assert!(AssetRateWithNative::from_asset_balance(100, native_non_system).is_err());

		// failure: native asset on non system chain as xcm v4 location
		let native_non_system = VersionedLocatableAsset::V4 {
			location: xcm::v4::Location::new(0, [xcm::v4::Junction::Parachain(2000)]),
			asset_id: xcm::v4::Location::parent().into(),
		};
		assert!(AssetRateWithNative::from_asset_balance(100, native_non_system).is_err());

		// failure: native asset on non system chain as xcm v3 location
		let native_non_system = VersionedLocatableAsset::V3 {
			location: xcm::v3::Location::new(
				0,
				xcm::v3::Junctions::X1(xcm::v3::Junction::Parachain(2000)),
			),
			asset_id: xcm::v3::Location::parent().into(),
		};
		assert!(AssetRateWithNative::from_asset_balance(100, native_non_system).is_err());

		// failure: some asset on Asset Hub as xcm v5 location
		let non_native = VersionedLocatableAsset::V5 {
			location: Location::new(0, [Parachain(2000)]),
			asset_id: Location::new(0, [PalletInstance(50), GeneralIndex(1984)]).into(),
		};
		assert!(AssetRateWithNative::from_asset_balance(100, non_native).is_err());

		// failure: some asset on Asset Hub as xcm v4 location
		let non_native = VersionedLocatableAsset::V4 {
			location: xcm::v4::Location::new(0, [xcm::v4::Junction::Parachain(2000)]),
			asset_id: xcm::v4::Location::new(
				0,
				[xcm::v4::Junction::PalletInstance(50), xcm::v4::Junction::GeneralIndex(1984)],
			)
			.into(),
		};
		assert!(AssetRateWithNative::from_asset_balance(100, non_native).is_err());

		// failure: native asset with invalid system chain location as xcm v5 location
		let native_non_system = VersionedLocatableAsset::V5 {
			location: Location::new(1, [Parachain(1000)]),
			asset_id: Location::parent().into(),
		};
		assert!(AssetRateWithNative::from_asset_balance(100, native_non_system).is_err());

		// failure: native asset with invalid system chain location as xcm v4 location
		let native_non_system = VersionedLocatableAsset::V4 {
			location: xcm::v4::Location::new(1, [xcm::v4::Junction::Parachain(1000)]),
			asset_id: xcm::v4::Location::parent().into(),
		};
		assert!(AssetRateWithNative::from_asset_balance(100, native_non_system).is_err());
	});
}
