pub mod entity;
pub mod packet;
pub mod registry;
pub mod token;

use crate::client::OriginClient;

/// Unified query facade regrouped by pallet.
pub struct Query<'a> {
	client: &'a OriginClient,
}

impl<'a> Query<'a> {
	pub fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn entity(&self) -> entity::EntityClient<'a> {
		entity::EntityClient::new(self.client)
	}

	pub fn registry(&self) -> registry::RegistryClient<'a> {
		registry::RegistryClient::new(self.client)
	}

	pub fn packet(&self) -> packet::PacketClient<'a> {
		packet::PacketClient::new(self.client)
	}

	pub fn token(&self) -> token::TokenClient<'a> {
		token::TokenClient::new(self.client)
	}
}
