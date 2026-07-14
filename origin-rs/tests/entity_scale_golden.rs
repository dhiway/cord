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

use oc::schema::entity::to_entity_input;
use origin_primitives::{element::ElementView, AttributeValueView};

/// Ensure EntityInfoInput SCALE matches ElementView encoding expectations.
#[test]
fn entity_info_input_matches_view_bytes() {
	let nested = oc::schema::entity::EntityNestedValue {
		display: ElementView::Raw(b"display".to_vec()),
		web: ElementView::Raw(b"web".to_vec()),
		email: ElementView::Raw(b"mail".to_vec()),
		attributes: Some(vec![AttributeValueView {
			key: b"id".to_vec(),
			value: ElementView::U64(42),
		}]),
	};
	let input = to_entity_input(&nested).expect("convert entity input");

	// Round-trip ElementView conversions.
	assert_eq!(ElementView::from(&input.display), nested.display);
	assert_eq!(ElementView::from(&input.web), nested.web);
	assert_eq!(ElementView::from(&input.email), nested.email);

	let attrs = input.attributes.as_ref().expect("attributes");
	let (key, val) = attrs.iter().next().expect("one attr");
	assert_eq!(key.as_slice(), b"id");
	assert_eq!(ElementView::from(val), ElementView::U64(42));
}
