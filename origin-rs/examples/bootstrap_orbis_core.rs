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

//! Assign bootstrap execution cores to Orbis through Origin Sudo.
//!
//! Usage:
//! `cargo run -p origin-rs --example bootstrap_orbis_core -- --endpoint ws://127.0.0.1:9900`

use clap::Parser;
use oc::{
	client::{signer::OriginSigner, OriginClient},
	types::OriginAccount,
};
use scale_value::{Composite, Value};

#[derive(Debug, Parser)]
struct Args {
	#[clap(long, default_value = "ws://127.0.0.1:9900")]
	endpoint: String,
	#[clap(long, default_value = "//Alice")]
	seed: String,
	#[clap(long, default_value_t = 1006)]
	para_id: u32,
	/// First relay core to assign.
	#[clap(long, default_value_t = 0)]
	first_core: u16,
	/// Number of consecutive full cores to assign atomically. Orbis targets three.
	#[clap(long, default_value_t = 3)]
	cores: u16,
	/// Relay block at which the assignment starts; defaults to best block plus two.
	#[clap(long)]
	begin: Option<u32>,
}

fn full_core_assignment(para_id: u32) -> Value<()> {
	Value::unnamed_composite(vec![Value::unnamed_composite(vec![
		Value::variant("Task", Composite::unnamed(vec![Value::u128(para_id.into())])),
		Value::unnamed_composite(vec![Value::u128(57_600)]),
	])])
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();
	if args.cores == 0 {
		return Err("--cores must be greater than zero".into());
	}

	let account =
		OriginAccount::from_uri(&args.seed, None).map_err(|error| format!("{error:?}"))?;
	let signer = OriginSigner::from_account(&account).map_err(|error| format!("{error:?}"))?;
	let client = OriginClient::connect(&args.endpoint).await?;
	let best = client.online().blocks().at_latest().await?.number();
	let begin = args.begin.unwrap_or(best.saturating_add(2));
	let tx = client.tx().using(signer);

	let mut assignments = Vec::with_capacity(args.cores.into());
	let mut assigned_cores = Vec::with_capacity(args.cores.into());
	for offset in 0..args.cores {
		let core = args.first_core.checked_add(offset).ok_or("core index overflow")?;
		assignments.push(
			subxt::dynamic::tx(
				"Coretime",
				"assign_core",
				vec![
					Value::u128(core.into()),
					Value::u128(begin.into()),
					full_core_assignment(args.para_id),
					Value::variant("None", Composite::unnamed(vec![])),
				],
			)
			.into_value(),
		);
		assigned_cores.push(core);
	}

	// A partial multi-core assignment leaves the parachain in a surprising intermediate state.
	// Match the upstream Orbis Storage/storage operator flow by applying every Coretime call
	// atomically under one Sudo dispatch.
	let batch =
		subxt::dynamic::tx("Utility", "batch_all", vec![Value::unnamed_composite(assignments)]);
	let sudo = subxt::dynamic::tx("Sudo", "sudo", vec![batch.into_value()]);
	let outcome = tx.submit(sudo).await?.wait_finalized().await?;
	println!(
		"assigned cores {assigned_cores:?} to task {} from relay block {begin}; finalized in {:?}",
		args.para_id, outcome.block
	);

	Ok(())
}
