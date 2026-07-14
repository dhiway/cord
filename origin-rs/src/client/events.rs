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

use std::sync::Arc;

use tokio::sync::mpsc;

use super::connection::Connection;
use crate::{types::error::OriginSdkError, util::retry::RetryPolicy};

/// Minimal dynamic event envelope.
#[derive(Debug, Clone)]
pub struct EventEnvelope {
	pub block: subxt::utils::H256,
	pub pallet: String,
	pub variant: String,
	pub fields: Vec<scale_value::Value<()>>,
}

/// Event streaming client with optional pallet filter.
#[derive(Clone)]
pub struct EventClient {
	connection: Arc<Connection>,
}

impl EventClient {
	pub(crate) fn new(connection: Arc<Connection>) -> Self {
		Self { connection }
	}

	/// Subscribe to finalized events; optional pallet name filter.
	pub async fn subscribe(
		&self,
		pallet: Option<&str>,
	) -> Result<mpsc::UnboundedReceiver<EventEnvelope>, OriginSdkError> {
		let filter = pallet.map(|s| s.to_owned());
		let (tx_main, rx) = mpsc::unbounded_channel();
		let connection = self.connection.clone();
		let retry = RetryPolicy::default();
		tokio::spawn(async move {
			loop {
				let fut = || {
					let api = connection.online().clone();
					let filter = filter.clone();
					let tx = tx_main.clone();
					async move {
						let mut blocks = api.blocks().subscribe_finalized().await?;
						while let Some(next) = blocks.next().await {
							let block = match next {
								Ok(b) => b,
								Err(_) => break,
							};
							let block_hash = block.hash();
							let events = match block.events().await {
								Ok(ev) => ev,
								Err(_) => continue,
							};
							for ev in events.iter() {
								if let Ok(ev) = ev {
									if let Some(ref f) = filter {
										if ev.pallet_name() != f {
											continue;
										}
									}
									let fields =
										ev.field_values().map_or(Vec::new(), |comp| match comp {
											scale_value::Composite::Named(v) => v
												.into_iter()
												.map(|(_, val)| val.remove_context())
												.collect(),
											scale_value::Composite::Unnamed(v) => v
												.into_iter()
												.map(|val| val.remove_context())
												.collect(),
										});
									let _ = tx.send(EventEnvelope {
										block: block_hash,
										pallet: ev.pallet_name().to_string(),
										variant: ev.variant_name().to_string(),
										fields,
									});
								}
							}
						}
						Ok::<(), subxt::Error>(())
					}
				};
				let _ = retry.retry(fut).await;
			}
		});
		Ok(rx)
	}
}
