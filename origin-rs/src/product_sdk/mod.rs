//! Transport-neutral product SDK contracts for the native Orbis application model.
//!
//! This module deliberately exposes typed JSON/domain contracts rather than SCALE bytes or
//! contract ABIs. The P0 identity is a fail-closed fixture contract; it is not a production
//! genesis declaration.

pub mod attestation_events;
pub mod contract;
pub mod domains;
pub mod dotns_events;
pub mod eqc;
pub mod host;
pub mod orbis_reads;
pub mod storage_events;
pub mod transport;

pub use attestation_events::OrbisAttestationEventSubscription;
pub use contract::{
	assert_composite_snapshot, decode_host_request, validate_descriptor_contract, Capability,
	Consent, DescriptorContract, Finality, HostMethod, HostRequest, NativeError, NativeErrorCode,
	NativeLifecycle, NativeLifecycleState, NetworkIdentity,
};
pub use dotns_events::OrbisDotnsEventSubscription;
pub use eqc::{validate_eqc_result, validate_slo_manifest, EqcClass, EqcResult, SloManifest};
pub use host::{FakeHost, HostSigner, HostTransport, SignedRequest, TerminalObserver};
pub use orbis_reads::OrbisFinalizedReadBinding;
pub use storage_events::OrbisStorageEventSubscription;
pub use transport::{
	prepare_attestation_command, prepare_dotns_command, prepare_drive_command, prepare_s3_command,
	prepare_storage_command, prepare_storage_provider_command, FinalizedReadBinding,
	GovernedSudoBinding, MissingFinalizedReadBinding, MissingGovernedSudoBinding,
	NativeDomainTransport, OrbisDomainTransport, OrbisNativeClient, OrbisTxPipeline,
};
