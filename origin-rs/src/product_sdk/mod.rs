// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Transport-neutral product SDK contracts for the native Orbis application model.
//!
//! This module deliberately exposes typed JSON/domain contracts rather than SCALE bytes or
//! contract ABIs. The exact clean-break prelaunch identity fails closed and is not a production
//! genesis declaration.

pub mod attestation_events;
pub mod contract;
pub mod domains;
pub mod eqc;
pub mod host;
pub mod names_events;
pub mod orbis_reads;
pub mod route_registry;
pub mod sponsored_intent;
pub mod storage_events;
pub mod transport;
pub mod version;

pub use crate::tx::meta::SponsoredIntent;
pub use attestation_events::OrbisAttestationEventSubscription;
pub use contract::{
	assert_composite_snapshot, decode_host_request, validate_descriptor_contract, Capability,
	Consent, DescriptorContract, Finality, HostRequest, NativeError, NativeErrorCode,
	NativeHostMethod, NativeLifecycle, NativeLifecycleState, NetworkAccessMode,
	NetworkActivationState, NetworkIdentity,
};
pub use eqc::{validate_eqc_result, validate_slo_manifest, EqcClass, EqcResult, SloManifest};
pub use host::{FakeHost, HostSigner, HostTransport, SignedRequest, TerminalObserver};
pub use names_events::OrbisNamesEventSubscription;
pub use orbis_reads::OrbisFinalizedReadBinding;
pub use route_registry::{instantiate_native_route, NativeRouteBinding};
pub use sponsored_intent::{
	prepare_sponsored_intent, submit_sponsored_intent, SponsoredIntentOutcome,
	SponsoredIntentRequest, SponsoredMortality, SponsoredNativeTarget,
};
pub use storage_events::OrbisStorageEventSubscription;
pub use transport::{
	prepare_attestation_command, prepare_drive_command, prepare_identity_personhood_command,
	prepare_names_command, prepare_s3_command, prepare_storage_command,
	prepare_storage_provider_command, FinalizedReadBinding, GovernedSudoBinding,
	MissingFinalizedReadBinding, MissingGovernedSudoBinding, NativeDomainTransport,
	OrbisDomainTransport, OrbisNativeClient, OrbisTxPipeline,
};
