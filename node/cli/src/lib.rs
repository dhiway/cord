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

//! CORD CLI library.

#![warn(missing_docs)]

pub mod benchmarking;
pub mod chain_spec;
pub mod fake_runtime_api;
pub mod service;

#[cfg(feature = "cli")]
mod cli;

#[cfg(feature = "cli")]
mod command;

#[cfg(feature = "service")]
pub use service::{self, Block, CoreApi, IdentifyVariant, ProvideRuntimeApi, TFullClient};

#[cfg(feature = "cli")]
pub use cli::*;

#[cfg(feature = "cli")]
pub use command::*;

#[cfg(feature = "cli")]
pub use sc_cli::{Error, Result};
