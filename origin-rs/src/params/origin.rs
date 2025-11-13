use super::{build_params_from, PreparedTxOptions};

pub type OriginParams = super::ParamsPayload;

pub fn params(opts: PreparedTxOptions) -> OriginParams {
	build_params_from(opts)
}
