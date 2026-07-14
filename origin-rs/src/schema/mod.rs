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

//! Schema transform layer for Origin SDK.
//!
//! These helpers make the nested ↔ flat mapping explicit so we can
//! keep the SDK aligned with pallet expectations while still offering
//! ergonomic nested structures to application developers.
//!
//! Current implementation keeps the flat and nested shapes identical;
//! dedicated flatten/expand logic can be filled in as the pallet types
//! are mirrored in the SDK.

pub mod entity;
pub mod packet;
pub mod registry;
