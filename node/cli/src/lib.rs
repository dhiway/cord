// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Polkadot CLI library.

#![warn(missing_docs)]

#[cfg(feature = "cli")]
mod benchmarking;
pub mod chain_spec;

#[cfg(feature = "cli")]
mod cli;

#[cfg(feature = "cli")]
mod error;

#[cfg(feature = "cli")]
mod command;

pub mod fake_runtime_api;
pub mod service;

#[cfg(feature = "service")]
pub use service::{self, Block, CoreApi, IdentifyVariant, ProvideRuntimeApi, TFullClient};

#[cfg(feature = "cli")]
pub use cli::*;

#[cfg(feature = "cli")]
pub use command::*;

#[cfg(feature = "cli")]
pub use sc_cli::{Error, Result};
