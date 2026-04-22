use subxt::config::{DefaultExtrinsicParamsBuilder, TransactionExtensions};

use crate::config::OriginConfig;

/// Origin SDK transaction extension tuple.
pub type OriginExtrinsicParams<T> = crate::config::OriginTransactionExtensions<T>;

/// Builder for Origin extrinsic params.
pub type OriginExtrinsicParamsBuilder<T> = DefaultExtrinsicParamsBuilder<T>;

/// Concrete params type used with `OriginConfig`.
pub type OriginParams =
	<OriginExtrinsicParams<OriginConfig> as TransactionExtensions<OriginConfig>>::Params;
