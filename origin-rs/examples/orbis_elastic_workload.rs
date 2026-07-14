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

//! Deterministic finalized-successful-call workload for the Orbis elastic campaign.
//!
//! The process submits unique `System::remark_with_event` intents until the measurement deadline,
//! then waits for finality and emits exactly one JSON summary for the campaign runner.

use std::{env, time::Duration};

use clap::Parser;
use futures::{stream::FuturesUnordered, StreamExt};
use oc::{
	client::{signer::SubxtSignerAdapter, OriginSigner},
	config::{build_orbis_params, OrbisClient, OrbisConfig},
	types::OriginAccount,
};
use serde::Serialize;
use std::sync::Arc;
use subxt::{config::DefaultExtrinsicParamsBuilder, dynamic::Value};

#[derive(Debug, Parser)]
struct Args {
	#[clap(long, default_value = "ws://127.0.0.1:9810")]
	endpoint: String,
	#[clap(long, default_value = "//Alice")]
	seed: String,
	#[clap(long)]
	duration_seconds: u64,
	#[clap(long)]
	repetition: u32,
	#[clap(long)]
	cores: u32,
	#[clap(long, default_value_t = 20_260_713)]
	campaign_seed: u64,
	#[clap(long, default_value_t = 64)]
	max_inflight: usize,
	#[clap(long, default_value_t = 120)]
	finality_grace_seconds: u64,
}

#[derive(Serialize)]
struct Summary {
	attempted_calls: u64,
	finalized_successful_calls: u64,
	failed_calls: u64,
	state_sha256: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();
	if args.duration_seconds == 0 || args.max_inflight == 0 || ![1, 3].contains(&args.cores) {
		return Err("duration/max-inflight must be positive and cores must be 1 or 3".into());
	}
	let state_sha256 = env::var("CAMPAIGN_STATE_SHA256")?;
	if state_sha256.len() != 64 || !state_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
		return Err("CAMPAIGN_STATE_SHA256 must be a lowercase SHA-256 digest".into());
	}

	let account =
		OriginAccount::from_uri(&args.seed, None).map_err(|error| format!("{error:?}"))?;
	let signer = OriginSigner::from_account(&account).map_err(|error| format!("{error:?}"))?;
	let account_id = signer.account_id();
	let signer = SubxtSignerAdapter::new(Arc::new(signer));
	let client = OrbisClient::from_url(&args.endpoint).await?;
	let runtime = client.runtime_version();
	if runtime.spec_version != 29 || runtime.transaction_version != 8 {
		return Err(format!(
			"unsupported Orbis runtime identity {}/{}",
			runtime.spec_version, runtime.transaction_version
		)
		.into());
	}
	let first_nonce = client.tx().account_nonce(&account_id).await?;

	let deadline = tokio::time::Instant::now() + Duration::from_secs(args.duration_seconds);
	let mut sequence = 0u64;
	let mut attempted = 0u64;
	let mut succeeded = 0u64;
	let mut failed = 0u64;
	let mut inflight = FuturesUnordered::new();

	loop {
		while tokio::time::Instant::now() < deadline && inflight.len() < args.max_inflight {
			let nonce = first_nonce.saturating_add(sequence);
			let intent = format!(
				"orbis-elastic:{}:{}:{}:{}",
				args.campaign_seed, args.cores, args.repetition, sequence
			);
			sequence = sequence.saturating_add(1);
			attempted = attempted.saturating_add(1);
			let call = subxt::dynamic::tx(
				"System",
				"remark_with_event",
				vec![Value::from_bytes(intent.into_bytes())],
			);
			let client = client.clone();
			let signer = signer.clone();
			inflight.push(async move {
				let params = build_orbis_params(
					DefaultExtrinsicParamsBuilder::<OrbisConfig>::new().nonce(nonce),
				);
				let signed = client
					.tx()
					.create_signed(&call, &signer, params)
					.await
					.map_err(|error| error.to_string())?;
				let progress =
					signed.submit_and_watch().await.map_err(|error| error.to_string())?;
				progress
					.wait_for_finalized_success()
					.await
					.map(|_| ())
					.map_err(|error| error.to_string())
			});
		}
		if tokio::time::Instant::now() >= deadline {
			break;
		}
		if let Some(result) = inflight.next().await {
			match result {
				Ok(()) => succeeded = succeeded.saturating_add(1),
				Err(error) => {
					if failed < 5 {
						eprintln!("workload call failed: {error}");
					}
					failed = failed.saturating_add(1);
				},
			}
		}
	}

	let drain = async {
		while let Some(result) = inflight.next().await {
			match result {
				Ok(()) => succeeded = succeeded.saturating_add(1),
				Err(error) => {
					if failed < 5 {
						eprintln!("workload call failed while draining: {error}");
					}
					failed = failed.saturating_add(1);
				},
			}
		}
	};
	if tokio::time::timeout(Duration::from_secs(args.finality_grace_seconds), drain)
		.await
		.is_err()
	{
		failed = attempted.saturating_sub(succeeded);
	}
	if succeeded.saturating_add(failed) != attempted {
		failed = attempted.saturating_sub(succeeded);
	}

	println!(
		"{}",
		serde_json::to_string(&Summary {
			attempted_calls: attempted,
			finalized_successful_calls: succeeded,
			failed_calls: failed,
			state_sha256,
		})?
	);
	Ok(())
}
