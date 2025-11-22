use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::mpsc;

use crate::types::error::OriginSdkError;
use super::connection::Connection;

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
		let api = self.connection.online().clone();
		let (tx, rx) = mpsc::unbounded_channel();
		tokio::spawn(async move {
			let mut blocks = match api.blocks().subscribe_finalized().await {
				Ok(s) => s,
				Err(_) => return,
			};
			while let Some(Ok(block)) = blocks.next().await {
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
						let fields = ev.field_values().map_or(Vec::new(), |comp| match comp {
							scale_value::Composite::Named(v) => v.into_iter().map(|(_, val)| val.remove_context()).collect(),
							scale_value::Composite::Unnamed(v) => v.into_iter().map(|val| val.remove_context()).collect(),
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
		});
		Ok(rx)
	}
}
