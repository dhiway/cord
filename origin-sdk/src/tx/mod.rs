pub mod entity;
pub mod packet;
pub mod registry;
pub mod token;

use crate::client::{signer::Signer, OriginClient};

/// High-level tx facade regrouped by pallet. Uses typed builders and schema transforms.
pub struct Tx<'a> {
	client: &'a OriginClient,
}

impl<'a> Tx<'a> {
	pub fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using<S>(&self, signer: S) -> TxWithSigner<'a, S>
	where
		S: Signer + Clone + 'static,
	{
		TxWithSigner { client: self.client, signer }
	}
}

/// Tx facade with signer bound for convenience.
pub struct TxWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> TxWithSigner<'a, S> {
	pub fn entity(&self) -> entity::EntityTx<'a, S> {
		entity::EntityTx::new(self.client, self.signer.clone())
	}

	pub fn registry(&self) -> registry::RegistryTx<'a, S> {
		registry::RegistryTx::new(self.client, self.signer.clone())
	}

	pub fn packet(&self) -> packet::PacketTx<'a, S> {
		packet::PacketTx::new(self.client, self.signer.clone())
	}

	pub fn token(&self) -> token::TokenTx<'a, S> {
		token::TokenTx::new(self.client, self.signer.clone())
	}

	pub fn call(
		&self,
		call: crate::extrinsic::builder::DynamicCall,
	) -> crate::extrinsic::batch::BatchBuilder {
		self.batch().call(call)
	}

	pub fn batch(&self) -> crate::extrinsic::batch::BatchBuilder {
		self.client.submit_with(self.signer.clone()).batch()
	}
}
