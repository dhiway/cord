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

//! A list of the different weight modules for our runtime.

pub mod frame_system;
pub mod frame_system_extensions;
pub mod pallet_babe;
pub mod pallet_balances;
pub mod pallet_entity;
pub mod pallet_indices;
pub mod pallet_meta_tx;
pub mod pallet_migrations;
pub mod pallet_multisig;
pub mod pallet_preimage;
pub mod pallet_proxy;
pub mod pallet_remark;
pub mod pallet_safe_mode;
pub mod pallet_scheduler;
pub mod pallet_session;
pub mod pallet_sudo;
pub mod pallet_timestamp;
pub mod pallet_transaction_payment;
pub mod pallet_tx_pause;
pub mod pallet_utility;
pub mod pallet_verify_signature;
