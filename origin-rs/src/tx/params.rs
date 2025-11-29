use subxt::config::{DefaultExtrinsicParams, DefaultExtrinsicParamsBuilder, ExtrinsicParams};

use crate::config::OriginConfig;

/// Origin SDK extrinsic params (type alias for now; centralized for future meta-tx tweaks).
pub type OriginExtrinsicParams<T> = DefaultExtrinsicParams<T>;

/// Builder for Origin extrinsic params.
pub type OriginExtrinsicParamsBuilder<T> = DefaultExtrinsicParamsBuilder<T>;

/// Concrete params type used with `OriginConfig`.
pub type OriginParams =
	<OriginExtrinsicParams<OriginConfig> as ExtrinsicParams<OriginConfig>>::Params;
