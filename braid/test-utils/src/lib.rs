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

#![cfg(unix)]

#[macro_export]
macro_rules! assert_eq_uvec {
	( $x:expr, $y:expr $(,)? ) => {
		$crate::__assert_eq_uvec!($x, $y);
		$crate::__assert_eq_uvec!($y, $x);
	};
}

#[macro_export]
#[doc(hidden)]
macro_rules! __assert_eq_uvec {
	( $x:expr, $y:expr ) => {
		$x.iter().for_each(|e| {
			if !$y.contains(e) {
				panic!("vectors not equal: {:?} != {:?}", $x, $y);
			}
		});
	};
}
