use super::{build_params_from, PreparedTxOptions};

pub type HubParams = super::ParamsPayload;

pub fn params(opts: PreparedTxOptions) -> HubParams {
	build_params_from(opts)
}
