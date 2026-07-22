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

use oc::{client::signer::OriginSigner, types::OriginAccount, OriginClient};
use origin_primitives::Ss58Identifier;
use serde_json::Value as JsonValue;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let _label = load_label().unwrap_or_else(|_| "demo".into());

	let account = OriginAccount::from_dev("//Alice")?;
	let signer = OriginSigner::from_account(&account)?;
	let client = OriginClient::connect("ws://localhost:9910").await?;

	let entity_id =
		Ss58Identifier::try_from(String::from("5FLSigC9H8J9tDFkhiBSGAL7iFusJqSQuJtVUXwwc7G7R6nW"))
			.map_err(|e| format!("{e:?}"))?;

	let entity = client.query().using(signer).entity().overview(entity_id).await?;
	match entity {
		Some(v) => println!("Entity overview: {:?}", v),
		None => println!("Entity not found or authorization failed"),
	}
	Ok(())
}

fn load_label() -> Result<String, Box<dyn std::error::Error>> {
	let data = std::fs::read_to_string("origin-sdk/examples/sample_data/demo.json")?;
	let v: JsonValue = serde_json::from_str(&data)?;
	let label = v
		.get("entity")
		.and_then(|e| e.get("display"))
		.and_then(|d| d.as_str())
		.unwrap_or("demo")
		.replace("{label}", "demo");
	Ok(label)
}
