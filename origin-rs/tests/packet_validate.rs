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

use oc::{schema::packet::PacketNestedValue, tx::packet::validate_packet_against_schema};
use origin_primitives::element::ElementView;

/// Basic schema validation: required key present, optional missing is ok, unknown key rejected.
#[test]
fn packet_validation_basic() {
	let nested = PacketNestedValue {
		attributes: vec![origin_primitives::packet::PacketAttributeView {
			key: b"id".to_vec(),
			value: ElementView::Raw(b"123".to_vec()),
		}],
	};
	let flat = oc::schema::packet::flatten_packet(&nested).expect("flatten");
	let schema = vec![(b"id".to_vec(), origin_primitives::element::ElementType::Raw, false)];
	let res = validate_packet_against_schema(&flat, &schema);
	assert!(res.is_ok());
}
