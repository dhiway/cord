use crate::{
	client::Client,
	flavors::ChainFlavor,
	sdk::{entities::EntityApi, packets::PacketApi, registers::RegisterApi, tokens::TokenApi},
};

use super::error::Result;

/// Lightweight domain-first client built on top of the existing `Client`.
pub struct OriginClient {
	inner: Client,
}

impl OriginClient {
	pub async fn connect(url: &str) -> Result<Self> {
		let inner = Client::connect(url, ChainFlavor::Auto).await?;
		Ok(Self { inner })
	}

	pub fn entities(&self) -> EntityApi<'_> {
		EntityApi::new(&self.inner)
	}

	pub fn registers(&self) -> RegisterApi<'_> {
		RegisterApi::new(&self.inner)
	}

	pub fn tokens(&self) -> TokenApi<'_> {
		TokenApi::new(&self.inner)
	}

	pub fn packets(&self) -> PacketApi<'_> {
		PacketApi::new(&self.inner)
	}
}

impl AsRef<Client> for OriginClient {
	fn as_ref(&self) -> &Client {
		&self.inner
	}
}
