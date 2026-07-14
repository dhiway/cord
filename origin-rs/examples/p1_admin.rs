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

//! Disposable-network P1 control and Broker administration driver.
//!
//! This example is intentionally explicit and waits for finalized success. It is for the
//! checked-in Zombienet topology only; never pass production keys or endpoints.

use clap::{Parser, Subcommand};
use oc::{
	client::{signer::OriginSigner, OriginClient},
	types::{ss58_to_account_id, OriginAccount},
};
use scale_value::{Composite, Value};
use serde_json::json;

#[derive(Debug, Parser)]
struct Args {
	#[clap(long)]
	endpoint: String,
	#[clap(long, default_value = "//Alice")]
	seed: String,
	#[clap(subcommand)]
	action: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
	/// Print call names and field type IDs from live metadata.
	Inspect {
		pallet: String,
	},
	AuthorityNominate {
		account: String,
	},
	AuthorityRemove {
		account: String,
	},
	CollatorsSet {
		#[clap(required = true)]
		accounts: Vec<String>,
	},
	SessionSetKeys {
		/// Hex returned by `author_rotateKeys` (193 bytes on Origin, 32 on Orbis).
		opaque_hex: String,
	},
	BrokerConfigure,
	BrokerReserve {
		para_id: u32,
		#[clap(long, default_value_t = 1)]
		count: u16,
	},
	BrokerUnreserve {
		index: u32,
	},
	BrokerRequest {
		cores: u16,
	},
	BrokerStart {
		#[clap(long, default_value_t = 1_000_000_000_000u128)]
		end_price: u128,
	},
	BrokerSetLease {
		task: u32,
		until: u32,
	},
	BrokerPurchase {
		#[clap(long, default_value_t = u128::MAX)]
		price_limit: u128,
	},
	BrokerRenew {
		core: u16,
	},
	BrokerAssign {
		begin: u32,
		core: u16,
		task: u32,
		#[clap(long)]
		final_assignment: bool,
	},
	Pause {
		pallet: String,
		call: String,
	},
	Unpause {
		pallet: String,
		call: String,
	},
	Remark {
		text: String,
	},
	/// Submit `System.remark` and require finalized dispatch rejection.
	RemarkRejected {
		text: String,
	},
	/// Prove signed rejection, then root authorization and safe invalid-code rejection/cleanup.
	UpgradeRejection,
}

fn sudo(call: subxt::tx::DynamicPayload) -> subxt::tx::DynamicPayload {
	subxt::dynamic::tx("Sudo", "sudo", vec![call.into_value()])
}

fn account_value(account: &str) -> Result<Value<()>, Box<dyn std::error::Error>> {
	let account = ss58_to_account_id(account)?;
	Ok(Value::from_bytes(AsRef::<[u8]>::as_ref(&account)))
}

fn full_core_schedule(para_id: u32) -> Value<()> {
	Value::unnamed_composite(vec![Value::unnamed_composite(vec![
		core_mask(),
		Value::variant("Task", Composite::unnamed(vec![Value::u128(para_id.into())])),
	])])
}

fn core_mask() -> Value<()> {
	// `pallet_broker::CoreMask` is a tuple newtype around `[u8; 10]`.
	Value::unnamed_composite(vec![Value::from_bytes([0xffu8; 10])])
}

fn region_id(begin: u32, core: u16) -> Value<()> {
	Value::named_composite(vec![
		("begin", Value::u128(begin.into())),
		("core", Value::u128(core.into())),
		("mask", core_mask()),
	])
}

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
	let value = value.strip_prefix("0x").unwrap_or(value);
	if value.len() % 2 != 0 {
		return Err("opaque key hex must contain whole bytes".into());
	}
	(0..value.len())
		.step_by(2)
		.map(|index| Ok(u8::from_str_radix(&value[index..index + 2], 16)?))
		.collect()
}

fn session_keys(value: &str) -> Result<Value<()>, Box<dyn std::error::Error>> {
	let keys = decode_hex(value)?;
	match keys.len() {
		32 => Ok(Value::named_composite(vec![("aura", Value::from_bytes(keys))])),
		193 => Ok(Value::named_composite(vec![
			("babe", Value::from_bytes(&keys[0..32])),
			("grandpa", Value::from_bytes(&keys[32..64])),
			("para_validator", Value::from_bytes(&keys[64..96])),
			("para_assignment", Value::from_bytes(&keys[96..128])),
			("authority_discovery", Value::from_bytes(&keys[128..160])),
			("beefy", Value::from_bytes(&keys[160..193])),
		])),
		length => {
			Err(format!("unexpected opaque session-key length {length}; expected 32 or 193").into())
		},
	}
}

fn call_name(pallet: String, call: String) -> Value<()> {
	Value::unnamed_composite(vec![Value::from_bytes(pallet), Value::from_bytes(call)])
}

async fn submit(
	client: &OriginClient,
	signer: OriginSigner,
	call: subxt::tx::DynamicPayload,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
	let outcome = client.tx().using(signer).submit(call).await?.wait_finalized().await?;
	Ok(json!({
		"status": "finalized-success",
		"extrinsic_hash": format!("{:?}", outcome.hash),
		"block_hash": outcome.block.map(|hash| format!("{hash:?}")),
		"events": outcome.events.into_iter().map(|event| json!({
			"pallet": event.pallet,
			"variant": event.variant,
			"fields": format!("{:?}", event.fields),
		})).collect::<Vec<_>>(),
	}))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();
	let account = OriginAccount::from_uri(&args.seed, None)?;
	let signer = OriginSigner::from_account(&account)?;
	let client = OriginClient::connect(&args.endpoint).await?;

	if let Action::Inspect { pallet } = &args.action {
		let metadata = client.metadata();
		let pallet_metadata = metadata
			.pallet_by_name(pallet)
			.ok_or_else(|| format!("pallet {pallet} is absent"))?;
		let calls = pallet_metadata
			.call_variants()
			.ok_or_else(|| format!("pallet {pallet} has no calls"))?;
		for call in calls {
			println!("{}#{} {:?}", call.name, call.index, call.fields);
		}
		return Ok(());
	}

	let call = match args.action {
		Action::Inspect { .. } => unreachable!(),
		Action::AuthorityNominate { account } => {
			sudo(subxt::dynamic::tx("AuthorityManager", "nominate", vec![account_value(&account)?]))
		},
		Action::AuthorityRemove { account } => {
			sudo(subxt::dynamic::tx("AuthorityManager", "remove", vec![account_value(&account)?]))
		},
		Action::CollatorsSet { accounts } => sudo(subxt::dynamic::tx(
			"CollatorSelection",
			"set_invulnerables",
			vec![Value::unnamed_composite(
				accounts
					.iter()
					.map(|account| account_value(account))
					.collect::<Result<Vec<_>, _>>()?,
			)],
		)),
		Action::SessionSetKeys { opaque_hex } => subxt::dynamic::tx(
			"Session",
			"set_keys",
			vec![session_keys(&opaque_hex)?, Value::from_bytes(Vec::<u8>::new())],
		),
		Action::BrokerConfigure => {
			let config = Value::named_composite(vec![
				("advance_notice", Value::u128(2)),
				("interlude_length", Value::u128(1)),
				("leadin_length", Value::u128(1)),
				("region_length", Value::u128(3)),
				(
					"ideal_bulk_proportion",
					Value::unnamed_composite(vec![Value::u128(1_000_000_000)]),
				),
				(
					"limit_cores_offered",
					Value::variant("Some", Composite::unnamed(vec![Value::u128(3)])),
				),
				("renewal_bump", Value::unnamed_composite(vec![Value::u128(100_000_000)])),
				("contribution_timeout", Value::u128(5)),
			]);
			sudo(subxt::dynamic::tx("Broker", "configure", vec![config]))
		},
		Action::BrokerReserve { para_id, count } => {
			if count == 0 {
				return Err("--count must be greater than zero".into());
			}
			let calls: Vec<Value<()>> = (0..count)
				.map(|_| {
					subxt::dynamic::tx("Broker", "reserve", vec![full_core_schedule(para_id)])
						.into_value()
				})
				.collect();
			sudo(subxt::dynamic::tx("Utility", "batch_all", vec![Value::unnamed_composite(calls)]))
		},
		Action::BrokerUnreserve { index } => {
			sudo(subxt::dynamic::tx("Broker", "unreserve", vec![Value::u128(index.into())]))
		},
		Action::BrokerRequest { cores } => sudo(subxt::dynamic::tx(
			"Broker",
			"request_core_count",
			vec![Value::u128(cores.into())],
		)),
		Action::BrokerStart { end_price } => sudo(subxt::dynamic::tx(
			"Broker",
			"start_sales",
			vec![Value::u128(end_price), Value::u128(0)],
		)),
		Action::BrokerSetLease { task, until } => sudo(subxt::dynamic::tx(
			"Broker",
			"set_lease",
			vec![Value::u128(task.into()), Value::u128(until.into())],
		)),
		Action::BrokerPurchase { price_limit } => {
			subxt::dynamic::tx("Broker", "purchase", vec![Value::u128(price_limit)])
		},
		Action::BrokerRenew { core } => {
			subxt::dynamic::tx("Broker", "renew", vec![Value::u128(core.into())])
		},
		Action::BrokerAssign { begin, core, task, final_assignment } => subxt::dynamic::tx(
			"Broker",
			"assign",
			vec![
				region_id(begin, core),
				Value::u128(task.into()),
				Value::variant(
					if final_assignment { "Final" } else { "Provisional" },
					Composite::unnamed(Vec::new()),
				),
			],
		),
		Action::Pause { pallet, call } => {
			sudo(subxt::dynamic::tx("TxPause", "pause", vec![call_name(pallet, call)]))
		},
		Action::Unpause { pallet, call } => {
			sudo(subxt::dynamic::tx("TxPause", "unpause", vec![call_name(pallet, call)]))
		},
		Action::Remark { text } => {
			subxt::dynamic::tx("System", "remark", vec![Value::from_bytes(text)])
		},
		Action::RemarkRejected { text } => {
			let tx = client
				.tx()
				.using(signer)
				.submit(subxt::dynamic::tx("System", "remark", vec![Value::from_bytes(text)]))
				.await?;
			let hash = format!("{:?}", tx.hash());
			match tx.wait_finalized().await {
				Ok(_) => return Err("paused System.remark unexpectedly succeeded".into()),
				Err(error) => println!(
					"{}",
					serde_json::to_string_pretty(&json!({
						"status": "finalized-rejection",
						"extrinsic_hash": hash,
						"error": error.to_string(),
					}))?
				),
			}
			return Ok(());
		},
		Action::UpgradeRejection => {
			let invalid_code = b"p1-invalid-runtime".to_vec();
			let code_hash = sp_crypto_hashing::blake2_256(&invalid_code);
			let direct = subxt::dynamic::tx(
				"System",
				"authorize_upgrade",
				vec![Value::from_bytes(code_hash)],
			);
			let tx = client.tx().using(signer.clone()).submit(direct).await?;
			let rejected_hash = format!("{:?}", tx.hash());
			let rejected = tx.wait_finalized().await;
			if rejected.is_ok() {
				return Err("signed authorize_upgrade unexpectedly succeeded".into());
			}
			let authorized = submit(
				&client,
				signer.clone(),
				sudo(subxt::dynamic::tx(
					"System",
					"authorize_upgrade",
					vec![Value::from_bytes(code_hash)],
				)),
			)
			.await?;
			let applied = submit(
				&client,
				signer,
				subxt::dynamic::tx(
					"System",
					"apply_authorized_upgrade",
					vec![Value::from_bytes(invalid_code)],
				),
			)
			.await?;
			println!(
				"{}",
				serde_json::to_string_pretty(&json!({
					"status": "safe-upgrade-rejection-complete",
					"unauthorized_extrinsic_hash": rejected_hash,
					"unauthorized_error": rejected.unwrap_err().to_string(),
					"authorized": authorized,
					"invalid_apply": applied,
				}))?
			);
			return Ok(());
		},
	};

	println!("{}", serde_json::to_string_pretty(&submit(&client, signer, call).await?)?);
	Ok(())
}
