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

/// Decode event field bytes into dynamic values using metadata.
pub(crate) fn decode_event_fields(
	metadata: &subxt::Metadata,
	pallet_name: &str,
	event_name: &str,
	field_bytes: &[u8],
) -> Result<Vec<scale_value::Value<()>>, OriginSdkError> {
	let pallet = metadata
		.pallet_by_name(pallet_name)
		.ok_or_else(|| OriginSdkError::Decode(format!("pallet {pallet_name} not found")))?;
	let variants = pallet
		.event_variants()
		.ok_or_else(|| OriginSdkError::Decode(format!("pallet {pallet_name} has no events")))?;
	let variant = variants
		.iter()
		.find(|v| v.name == event_name)
		.ok_or_else(|| OriginSdkError::Decode(format!("event {event_name} not found")))?;

	let mut cursor = field_bytes;
	let mut fields = Vec::new();
	for field in &variant.fields {
		let value = scale_value::scale::decode_as_type(&mut cursor, field.ty.id, metadata.types())
			.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
		fields.push(value.remove_context());
	}
	Ok(fields)
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
						let mut blocks = api.stream_blocks().await?;
						while let Some(next) = blocks.next().await {
							let block = match next {
								Ok(b) => b,
								Err(_) => break,
							};
							let block_hash = block.hash();
							let at = match block.at().await {
								Ok(a) => a,
								Err(_) => continue,
							};
							let metadata = at.metadata_ref();
							let events = match at.events().fetch().await {
								Ok(ev) => ev,
								Err(_) => continue,
							};
							for ev in events.iter().flatten() {
								if let Some(ref f) = filter {
									if ev.pallet_name() != f {
										continue;
									}
								}
								let fields = decode_event_fields(
									metadata,
									ev.pallet_name(),
									ev.event_name(),
									ev.field_bytes(),
								)
								.unwrap_or_default();
								let _ = tx.send(EventEnvelope {
									block: block_hash,
									pallet: ev.pallet_name().to_string(),
									variant: ev.event_name().to_string(),
									fields,
								});
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
