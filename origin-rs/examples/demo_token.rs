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

//! Token demo: resolve identifier and show timeline.
//! cargo run -p origin-sdk --example demo_token -- --endpoint ws://localhost:9944 --token 5C8F...
//! [--seed //Alice] [--meta]

use clap::Parser;
use oc::{client::signer::OriginSigner, types::OriginAccount, OriginClient};

#[derive(Parser, Debug)]
struct Args {
	#[clap(long, default_value = "ws://localhost:9944")]
	endpoint: String,
	#[clap(long, help = "token ss58 string to resolve")]
	token: String,
	#[clap(long, default_value = "//Alice")]
	seed: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let args = Args::parse();
	let token = origin_primitives::Ss58Identifier::try_from(args.token.clone())
		.map_err(|e| format!("token parse: {e:?}"))?;
	let account = OriginAccount::from_uri(&args.seed, None)?;
	let signer = OriginSigner::from_account(&account)?;
	let client = OriginClient::connect(&args.endpoint).await?;

	let decoded = client
		.query()
		.using(signer.clone())
		.token()
		.resolve_identifier(token.clone())
		.await?;
	match decoded {
		Some(id) => println!("Decoded identifier: {:?}", id),
		None => println!("Token not found or unauthorized"),
	}
	let timeline = client.query().using(signer).token().timeline(token, None, Some(10)).await?;
	match timeline {
		Some(tl) => println!("Token timeline: {:?}", tl),
		None => println!("No timeline available (not found or unauthorized)"),
	}
	Ok(())
}
