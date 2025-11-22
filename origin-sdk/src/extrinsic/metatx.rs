use super::builder::DynamicCall;
use crate::client::connection::Connection;
use crate::client::signer::Signer;
use crate::types::error::OriginSdkError;
use std::sync::Arc;

/// Meta-transaction helper (scaffold).
#[derive(Clone)]
pub struct MetaTxClient {
	connection: Arc<Connection>,
	signer: Arc<dyn Signer>,
}

impl MetaTxClient {
	pub(crate) fn new(connection: Arc<Connection>, signer: Arc<dyn Signer>) -> Self {
		Self { connection, signer }
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

	/// Prepare a payload for off-chain relay (currently returns the dynamic call descriptor).
	pub fn prepare_payload(&self, call: DynamicCall) -> Result<DynamicCall, OriginSdkError> {
		self.wrap(call)
	}

	/// Sign and submit a meta-transaction via MetaTx pallet using provided signer (meta key).
	pub async fn sign_and_submit_with(
		&self,
		call: DynamicCall,
		signer: Arc<dyn Signer>,
	) -> Result<crate::client::submit::TxHandle, OriginSdkError> {
		let wrapped = self.wrap(call)?;
		let payload = subxt::dynamic::tx(wrapped.pallet, wrapped.function, wrapped.args);
		let submit = crate::client::submit::SubmitClient::new(self.connection.clone(), signer);
		submit.submit_payload(payload).await
	}

	/// Convenience: use the SDK signer to sign + submit locally.
	pub async fn sign_and_submit(
		&self,
		call: DynamicCall,
	) -> Result<crate::client::submit::TxHandle, OriginSdkError> {
		self.sign_and_submit_with(call, self.signer.clone()).await
	}
}
