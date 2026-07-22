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
use scale_value::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::init();
	let acct = OriginAccount::from_dev("//Alice")?;
	let signer = OriginSigner::from_account(&acct)?;
	let client = OriginClient::connect("ws://localhost:9944").await?;
	let account = client.tx().using(signer.clone());

	let builder = oc::extrinsic::builder::DynamicCallBuilder::new();
	let call1 = builder.call("Entity", "set_entity_nym", vec![Value::from_bytes(b"nym-a")]);
	let call2 = builder.call("Entity", "remove_entity_nym", vec![Value::from_bytes(b"some-id")]);

	let outcome = account.batch().call(call1).call(call2).submit_and_wait_finalized().await?;

	println!("batch finalized in block {:?}", outcome.block);
	Ok(())
}
