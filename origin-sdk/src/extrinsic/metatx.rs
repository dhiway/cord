use std::sync::Arc;

use crate::types::error::OriginSdkError;
use crate::client::connection::Connection;
use crate::client::submit::SubxtSignerAdapter;
use super::builder::DynamicCall;
use crate::client::Signer;
use subxt::tx::Signer as _;

/// Meta-transaction helper (scaffold).
#[derive(Clone)]
pub struct MetaTxClient {
	#[allow(dead_code)]
	connection: Arc<Connection>,
}

impl MetaTxClient {
	pub(crate) fn new(connection: Arc<Connection>) -> Self {
		Self { connection }
	}

	/// Wrap a call into MetaTx::submit(pallet, function, args_bytes).
	pub fn wrap(&self, call: DynamicCall) -> Result<DynamicCall, OriginSdkError> {
		let inner_value = subxt::dynamic::tx(call.pallet, call.function, call.args).into_value();
		let wrapped = DynamicCall {
			pallet: "MetaTx".into(),
			function: "submit".into(),
			args: vec![inner_value],
		};
		Ok(wrapped)
	}

	/// Sign and submit a meta-transaction via MetaTx pallet.
	pub async fn sign_and_submit(
		&self,
		call: DynamicCall,
		signer: &dyn Signer,
    ) -> Result<crate::client::submit::TxHandle, OriginSdkError> {
		let wrapped = self.wrap(call)?;
		let payload = subxt::dynamic::tx(wrapped.pallet, wrapped.function, wrapped.args);
		let adapter = super::super::client::submit::SubxtSignerAdapter::new(signer);
		let nonce = self.connection.online().tx().account_nonce(&adapter.account_id()).await?;
		let params =
			subxt::config::DefaultExtrinsicParamsBuilder::<super::super::client::OriginConfig>::new()
				.nonce(nonce)
				.build();
		let progress = self
			.connection
			.online()
			.tx()
			.sign_and_submit_then_watch(&payload, &adapter, params)
			.await?;
		let hash = progress.extrinsic_hash();
		let _ = progress.wait_for_finalized_success().await?;
		Ok(crate::client::submit::TxHandle { hash })
	}
}
