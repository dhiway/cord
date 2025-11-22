use crate::{error::Error, origin_client::DynamicEvent};
use subxt::OnlineClient;

use crate::params::config::OriginConfig;

/// Basic event filter definition used by the event watcher helper.
#[derive(Clone, Debug, Default)]
pub struct EventFilter {
	pub pallet: Option<String>,
	pub variant: Option<String>,
}

impl EventFilter {
	pub fn by(pallet: impl Into<String>, variant: impl Into<String>) -> Self {
		Self { pallet: Some(pallet.into()), variant: Some(variant.into()) }
	}
}

/// Lightweight event watcher that runs against finalized blocks.
pub struct EventWatcher {
	client: OnlineClient<OriginConfig>,
	filter: EventFilter,
}

impl EventWatcher {
	pub fn new(client: OnlineClient<OriginConfig>, filter: EventFilter) -> Self {
		Self { client, filter }
	}

	pub async fn run<F>(&self, mut callback: F) -> Result<(), Error>
	where
		F: FnMut(DynamicEvent) + Send + 'static,
	{
		let mut sub = self.client.blocks().subscribe_finalized().await.map_err(Error::from)?;
		while let Some(block_res) = sub.next().await {
			let block = match block_res {
				Ok(block) => block,
				Err(err) => return Err(Error::from(err)),
			};

			let events = block.events().await.map_err(Error::from)?;
			for ev in events.iter() {
				if let Ok(ev) = ev {
					let pallet_name = ev.pallet_name().to_string();
					let variant_name = ev.variant_name().to_string();

					if let Some(ref pallet) = self.filter.pallet {
						if pallet != &pallet_name {
							continue;
						}
					}
					if let Some(ref variant) = self.filter.variant {
						if variant != &variant_name {
							continue;
						}
					}

					if let Ok(fields) = ev.field_values() {
						let dynamic = DynamicEvent {
							block_hash: block.hash(),
							block_number: block.number().into(),
							pallet: pallet_name,
							variant: variant_name,
							fields,
						};
						callback(dynamic);
					}
				}
			}
		}
		Ok(())
	}
}
