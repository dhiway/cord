use super::{build_params_from, PreparedTxOptions};

pub type OrbParams = super::ParamsPayload;

pub fn params(opts: PreparedTxOptions) -> OrbParams {
	build_params_from(opts)
}
