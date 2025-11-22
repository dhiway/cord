use std::sync::Arc;

use crate::types::error::OriginSdkError;
use crate::client::connection::Connection;
use super::builder::DynamicCall;

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

pub fn wrap(&self, call: DynamicCall) -> Result<DynamicCall, OriginSdkError> {
	// Minimal placeholder: wrap original call into MetaTx::submit(pallet, call, args)
	let wrapped = DynamicCall {
		pallet: "MetaTx".into(),
		function: "submit".into(),
		args: vec![
			subxt::dynamic::Value::from(call.pallet),
			subxt::dynamic::Value::from(call.function),
			subxt::dynamic::Value::from(call.args),
		],
	};
	Ok(wrapped)
}
}
