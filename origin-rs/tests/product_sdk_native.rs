use std::sync::{
	atomic::{AtomicUsize, Ordering},
	Arc, Mutex,
};

use async_trait::async_trait;
use oc::product_sdk::{
	assert_composite_snapshot,
	contract::{
		assert_no_contract_surface, DescriptorContract, NativeLifecycleState,
		ORBIS_DESCRIPTOR_CONTRACT_SHA256,
	},
	host::{HostResponse, NativeErrorCodeOrSuccess},
	instantiate_native_route, validate_descriptor_contract, validate_eqc_result,
	validate_slo_manifest, Capability, Consent, EqcResult, FakeHost, Finality, HostRequest,
	HostSigner, HostTransport, NativeError, NativeErrorCode, NativeHostMethod, NativeLifecycle,
	NetworkIdentity, SignedRequest, SloManifest, TerminalObserver,
};
use serde_json::{json, Map, Value};
use tokio::sync::{watch, Notify};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

fn load(path: &str) -> String {
	std::fs::read_to_string(format!("{ROOT}/{path}"))
		.expect("shared native SDK fixture must be readable")
}

fn payload(value: Value) -> Map<String, Value> {
	value.as_object().expect("test payload is an object").clone()
}

fn request(id: &str, nonce: &str) -> HostRequest {
	HostRequest {
		version: 1,
		request_id: id.into(),
		application_id: "festival".into(),
		capability: Capability::Attestation,
		method: NativeHostMethod::new("schema_by_id"),
		network: NetworkIdentity::orbis_candidate(),
		finality: Finality::Finalized,
		payload: payload(json!({"schema": format!("0x{}", "11".repeat(32))})),
		consent: Consent {
			scope: vec!["attestation:schema_by_id".into()],
			expires_at: 2_000,
			nonce: nonce.into(),
		},
	}
}

fn authorize(host: &FakeHost, request: HostRequest) -> HostRequest {
	host.issue_consent(request.application_id.clone(), &request.consent).unwrap();
	request
}

fn code<T>(result: Result<T, NativeError>) -> &'static str {
	match result {
		Ok(_) => "success",
		Err(error) => match error.code {
			NativeErrorCode::PermissionDenied => "permission_denied",
			NativeErrorCode::PermissionRevoked => "permission_revoked",
			NativeErrorCode::ConsentExpired => "consent_expired",
			NativeErrorCode::Replay => "replay",
			NativeErrorCode::Cancelled => "cancelled",
			NativeErrorCode::Timeout => "timeout",
			NativeErrorCode::UnsupportedRuntime => "unsupported_runtime",
			NativeErrorCode::InconsistentSnapshot => "inconsistent_snapshot",
			NativeErrorCode::UnsupportedSurface => "unsupported_surface",
			_ => "other",
		},
	}
}

#[tokio::test]
async fn shared_host_scenario_registry_has_rust_conformance_outcomes() {
	let registry: Value =
		serde_json::from_str(&load("docs/sdk/host/fake-host-scenarios.json")).unwrap();
	let registered: Vec<&str> = registry["scenarios"]
		.as_array()
		.unwrap()
		.iter()
		.map(|scenario| scenario["id"].as_str().unwrap())
		.collect();
	let expected = [
		"allow-scoped-read",
		"deny-unknown-capability",
		"deny-scope-escalation",
		"deny-missing-host-consent",
		"deny-caller-self-authorized-consent",
		"deny-revoked-consent",
		"deny-expired-consent",
		"deny-replayed-nonce",
		"revoke-before-sign",
		"cancel-in-flight",
		"transport-timeout",
		"metadata-drift",
		"composite-cross-hash",
		"contract-abi-request",
	];
	assert_eq!(registered, expected);

	let host = FakeHost::default();
	assert_eq!(
		host.grant("festival", &["attestation:unknown"]).unwrap_err().code,
		NativeErrorCode::InvalidInput
	);
	let unknown_consent = Consent {
		scope: vec!["attestation:unknown".into()],
		expires_at: 2_000,
		nonce: "nonce-unknown-0001".into(),
	};
	assert_eq!(
		host.issue_consent("festival", &unknown_consent).unwrap_err().code,
		NativeErrorCode::InvalidInput
	);
	host.grant("festival", &["attestation:schema_by_id"]).unwrap();
	assert_eq!(
		code(
			host.execute(authorize(&host, request("request-0000000001", "nonce-00000000001")))
				.await
		),
		"success"
	);

	let mut hostile = request("request-0000000002", "nonce-00000000002");
	hostile.capability = Capability::Unsupported;
	assert_eq!(code(host.execute(hostile).await), "unsupported_surface");

	let mut hostile = request("request-0000000003", "nonce-00000000003");
	hostile.method = NativeHostMethod::new("attestation_live_status");
	hostile.payload = payload(json!({"attestation": format!("0x{}", "11".repeat(32))}));
	hostile.consent.scope = vec!["attestation:attestation_live_status".into()];
	assert_eq!(code(host.execute(authorize(&host, hostile)).await), "permission_denied");

	assert_eq!(
		code(host.execute(request("request-missing-0001", "nonce-missing-00001")).await),
		"permission_denied"
	);

	let mut self_authorized = request("request-selfauth-001", "nonce-selfauth-0001");
	host.issue_consent(self_authorized.application_id.clone(), &self_authorized.consent)
		.unwrap();
	self_authorized.consent.scope.push("attestation:attestation_live_status".into());
	assert_eq!(code(host.execute(self_authorized).await), "permission_denied");

	let revoked_consent = authorize(&host, request("request-consent-rev", "nonce-consent-revoke"));
	host.revoke_consent(&revoked_consent.consent.nonce);
	assert_eq!(code(host.execute(revoked_consent).await), "permission_revoked");

	let mut hostile = request("request-0000000004", "nonce-00000000004");
	hostile.consent.expires_at = 999;
	assert_eq!(code(host.execute(authorize(&host, hostile)).await), "consent_expired");

	let replay_host = FakeHost::default();
	replay_host.grant("festival", &["attestation:schema_by_id"]).unwrap();
	replay_host
		.execute(authorize(&replay_host, request("request-0000000005", "nonce-replay-00001")))
		.await
		.unwrap();
	assert_eq!(
		code(replay_host.execute(request("request-0000000006", "nonce-replay-00001")).await),
		"replay"
	);

	let revoked = FakeHost::default();
	revoked.grant("festival", &["attestation:schema_by_id"]).unwrap();
	let revoked_request = authorize(&revoked, request("request-0000000007", "nonce-00000000007"));
	revoked.revoke("festival");
	assert_eq!(code(revoked.execute(revoked_request).await), "permission_revoked");

	let cancelled = FakeHost::default();
	cancelled.grant("festival", &["attestation:schema_by_id"]).unwrap();
	let cancelled_request =
		authorize(&cancelled, request("request-0000000008", "nonce-00000000008"));
	cancelled.cancel("request-0000000008");
	assert_eq!(code(cancelled.execute(cancelled_request).await), "cancelled");

	let timeout = FakeHost::new(
		Arc::new(|| 1_000),
		Arc::new(TestSigner::immediate()),
		Arc::new(TimeoutTransport),
		Arc::new(RecordingObserver::default()),
	);
	timeout.grant("festival", &["attestation:schema_by_id"]).unwrap();
	assert_eq!(
		code(
			timeout
				.execute(authorize(&timeout, request("request-0000000009", "nonce-00000000009")))
				.await
		),
		"timeout"
	);

	let mut drift = request("request-0000000010", "nonce-00000000010");
	drift.network.spec_version = 30;
	assert_eq!(code(host.execute(drift).await), "unsupported_runtime");
	assert_eq!(code(assert_composite_snapshot(&["0xaa", "0xbb"])), "inconsistent_snapshot");

	let mut abi = request("request-0000000011", "nonce-00000000011");
	abi.payload = payload(json!({"subject_id":"alice", "nested":{"contract_address":"0x01"}}));
	assert_eq!(code(host.execute(abi).await), "unsupported_surface");
}

#[tokio::test]
async fn every_authoritative_native_route_constructs_validates_and_dispatches_in_rust() {
	let contract: Value =
		serde_json::from_str(&load("docs/sdk/native-route-contract.json")).unwrap();
	let routes = contract["routes"].as_array().unwrap();
	assert_eq!(routes.len(), 132);
	let selected = Arc::new(Mutex::new(Vec::new()));
	let host = FakeHost::new(
		Arc::new(|| 1_000),
		Arc::new(TestSigner::immediate()),
		Arc::new(RouteRecordingTransport { selected: selected.clone() }),
		Arc::new(RecordingObserver::default()),
	);
	for (index, route) in routes.iter().enumerate() {
		instantiate_native_route(route).unwrap().validate_and_prepare().unwrap();
		let scope = route["id"].as_str().unwrap();
		host.grant("route-harness", &[scope]).unwrap();
		let consent = Consent {
			scope: vec![scope.into()],
			expires_at: 2_000,
			nonce: format!("route-consent-{index:04}"),
		};
		host.issue_consent("route-harness", &consent).unwrap();
		let request: HostRequest = serde_json::from_value(json!({
			"version": 1,
			"request_id": format!("route-request-{index:04}"),
			"application_id": "route-harness",
			"capability": route["capability"],
			"method": route["method"],
			"network": NetworkIdentity::orbis_candidate(),
			"finality": route["finality"],
			"payload": route["sample_payload"],
			"consent": consent,
		}))
		.unwrap();
		request.validate().unwrap();
		host.execute(request).await.unwrap();
		assert!(route["runtime"]["target"].as_str().is_some_and(|target| !target.is_empty()));
	}
	assert_eq!(selected.lock().unwrap().len(), routes.len());
}

#[test]
fn shared_descriptor_and_identity_are_exact_and_fail_closed() {
	let descriptor: DescriptorContract = serde_json::from_str(&load(
		"product-sdk/packages/descriptors/generated/orbis-descriptor.json",
	))
	.unwrap();
	validate_descriptor_contract(&descriptor).unwrap();
	assert_eq!(ORBIS_DESCRIPTOR_CONTRACT_SHA256.len(), 64);

	for (field, expected) in [
		("spec_version", NativeErrorCode::UnsupportedRuntime),
		("transaction_version", NativeErrorCode::UnsupportedRuntime),
		("metadata_hash", NativeErrorCode::MetadataMismatch),
		("descriptor_contract_sha256", NativeErrorCode::DescriptorMismatch),
		("genesis_hash", NativeErrorCode::UnsupportedRuntime),
		("chain_spec_source_sha256", NativeErrorCode::UnsupportedRuntime),
	] {
		let mut identity = NetworkIdentity::orbis_candidate();
		match field {
			"spec_version" => identity.spec_version += 1,
			"transaction_version" => identity.transaction_version += 1,
			"metadata_hash" => identity.metadata_hash = "0x00".into(),
			"descriptor_contract_sha256" => identity.descriptor_contract_sha256 = "00".repeat(32),
			"genesis_hash" => identity.genesis_hash = "0x00".into(),
			_ => identity.chain_spec_source_sha256 = "00".repeat(32),
		}
		assert_eq!(identity.validate().unwrap_err().code, expected, "{field}");
	}

	let manifest: Value =
		serde_json::from_str(&load("docs/sdk/compatibility-manifest.json")).unwrap();
	assert_eq!(manifest["network"]["orbis_spec_version"], 29);
	assert_eq!(manifest["network"]["orbis_transaction_version"], 8);
	assert_eq!(manifest["client_policy"]["unknown_spec"], "reject");
	assert_eq!(manifest["client_policy"]["metadata_hash_mismatch"], "reject");
	assert_eq!(manifest["client_policy"]["descriptor_hash_mismatch"], "reject");
}

#[test]
fn recursive_contract_abi_scale_and_address_surfaces_are_rejected() {
	for value in [
		json!({"nested": [{"raw_SCALE": "0x00"}]}),
		json!({"nested": {"ContractABI": []}}),
		json!({"nested": {"deployment_contract_address": "0x01"}}),
		json!({"safe": ["revive contract address"]}),
	] {
		assert_eq!(
			assert_no_contract_surface(&value, "payload").unwrap_err().code,
			NativeErrorCode::UnsupportedSurface
		);
	}
	assert_no_contract_surface(&json!({"account_address":"5Alice"}), "payload").unwrap();
}

#[test]
fn lifecycle_envelope_enforces_exact_terminal_evidence() {
	let cancelled = NativeError::new(NativeErrorCode::Cancelled, "cancelled");
	let lifecycle = NativeLifecycle {
		version: 1,
		intent_id: "intent-0000000001".into(),
		state: NativeLifecycleState::Cancelled,
		block_hash: None,
		extrinsic_hash: None,
		error: Some(cancelled),
	};
	lifecycle.validate().unwrap();
	let mut invalid = lifecycle.clone();
	invalid.error = Some(NativeError::new(NativeErrorCode::Timeout, "timeout"));
	assert_eq!(invalid.validate().unwrap_err().code, NativeErrorCode::InvalidInput);

	let finalized = NativeLifecycle {
		version: 1,
		intent_id: "intent-0000000002".into(),
		state: NativeLifecycleState::Finalized,
		block_hash: Some(format!("0x{}", "ab".repeat(32))),
		extrinsic_hash: Some(format!("0x{}", "cd".repeat(32))),
		error: None,
	};
	finalized.validate().unwrap();
	let mut invalid = finalized;
	invalid.extrinsic_hash = None;
	assert_eq!(invalid.validate().unwrap_err().code, NativeErrorCode::InvalidInput);
}

#[test]
fn shared_eqc_targets_and_schema_only_results_are_strictly_validated() {
	let manifest: SloManifest =
		serde_json::from_str(&load("docs/evidence/performance/service-slo-manifest.json")).unwrap();
	let payload: Value = serde_json::from_str(&load(
		"docs/evidence/verification/p5/sdk-freeze-ratification.payload.json",
	))
	.unwrap();
	validate_slo_manifest(&manifest, &payload).unwrap();

	for class in ["E", "Q", "C"] {
		let result: EqcResult = serde_json::from_str(&load(&format!(
			"product-sdk/tests/eqc/fixtures/{class}.schema-only.json"
		)))
		.unwrap();
		validate_eqc_result(&result, &"0".repeat(64)).unwrap();
		let mut claim = result.clone();
		claim.campaign_executed = true;
		assert_eq!(
			validate_eqc_result(&claim, &"0".repeat(64)).unwrap_err().code,
			NativeErrorCode::InvalidInput
		);
	}

	let mut weakened = manifest.clone();
	weakened.campaign.interleaved_runs = 4;
	assert_eq!(
		validate_slo_manifest(&weakened, &payload).unwrap_err().code,
		NativeErrorCode::InvalidInput
	);
	let mut dishonest = payload;
	dishonest["performance_claim"] = json!("false");
	assert_eq!(
		validate_slo_manifest(&manifest, &dishonest).unwrap_err().code,
		NativeErrorCode::InvalidInput
	);

	let malformed = load("product-sdk/tests/eqc/fixtures/E.schema-only.json")
		.replace("\"samples\": 0", "\"samples\": \"0\"");
	assert!(serde_json::from_str::<EqcResult>(&malformed).is_err());
	let unknown = load("product-sdk/tests/eqc/fixtures/E.schema-only.json")
		.replace("\"schema_version\": 1,", "\"schema_version\": 1, \"unknown\": false,");
	assert!(serde_json::from_str::<EqcResult>(&unknown).is_err());
}

#[derive(Default)]
struct RecordingObserver {
	outcomes: Mutex<Vec<NativeErrorCodeOrSuccess>>,
}

impl TerminalObserver for RecordingObserver {
	fn terminal(&self, _request_id: &str, outcome: NativeErrorCodeOrSuccess) {
		self.outcomes.lock().unwrap().push(outcome);
	}
}

struct TestSigner {
	entered: Arc<Notify>,
	release: Option<Arc<Notify>>,
	calls: Arc<AtomicUsize>,
}

impl TestSigner {
	fn immediate() -> Self {
		Self {
			entered: Arc::new(Notify::new()),
			release: None,
			calls: Arc::new(AtomicUsize::new(0)),
		}
	}
}

#[async_trait]
impl HostSigner for TestSigner {
	async fn sign(&self, _request: &HostRequest) -> Result<String, NativeError> {
		self.calls.fetch_add(1, Ordering::SeqCst);
		self.entered.notify_one();
		if let Some(release) = &self.release {
			release.notified().await;
		}
		Ok("signature".into())
	}
}

struct TimeoutTransport;

#[async_trait]
impl HostTransport for TimeoutTransport {
	async fn submit(
		&self,
		_request: SignedRequest,
		_cancelled: watch::Receiver<bool>,
	) -> Result<HostResponse, NativeError> {
		Err(NativeError::new(NativeErrorCode::Timeout, "transport timeout").retryable())
	}
}

struct RouteRecordingTransport {
	selected: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl HostTransport for RouteRecordingTransport {
	async fn submit(
		&self,
		request: SignedRequest,
		_cancelled: watch::Receiver<bool>,
	) -> Result<HostResponse, NativeError> {
		self.selected.lock().unwrap().push(format!(
			"{:?}:{}:{}",
			request.request.finality,
			request.request.scope_name()?,
			request.signature
		));
		Ok(HostResponse { finalized_hash: format!("0x{}", "ab".repeat(32)) })
	}
}

struct BlockingTransport {
	entered: Arc<Notify>,
	release: Arc<Notify>,
	calls: Arc<AtomicUsize>,
}

#[async_trait]
impl HostTransport for BlockingTransport {
	async fn submit(
		&self,
		_request: SignedRequest,
		_cancelled: watch::Receiver<bool>,
	) -> Result<HostResponse, NativeError> {
		self.calls.fetch_add(1, Ordering::SeqCst);
		self.entered.notify_one();
		self.release.notified().await;
		Ok(HostResponse { finalized_hash: format!("0x{}", "aa".repeat(32)) })
	}
}

#[tokio::test]
async fn cancellation_is_exactly_once_and_late_transport_cannot_win() {
	let observer = Arc::new(RecordingObserver::default());
	let entered = Arc::new(Notify::new());
	let release = Arc::new(Notify::new());
	let calls = Arc::new(AtomicUsize::new(0));
	let host = Arc::new(FakeHost::new(
		Arc::new(|| 1_000),
		Arc::new(TestSigner::immediate()),
		Arc::new(BlockingTransport {
			entered: entered.clone(),
			release: release.clone(),
			calls: calls.clone(),
		}),
		observer.clone(),
	));
	host.grant("festival", &["attestation:schema_by_id"]).unwrap();
	let cancel_request = authorize(&host, request("request-cancel-0001", "nonce-cancel-00001"));
	let running_host = host.clone();
	let task = tokio::spawn(async move { running_host.execute(cancel_request).await });
	entered.notified().await;
	host.cancel("request-cancel-0001");
	host.cancel("request-cancel-0001");
	assert_eq!(task.await.unwrap().unwrap_err().code, NativeErrorCode::Cancelled);
	release.notify_waiters();
	tokio::task::yield_now().await;
	assert_eq!(calls.load(Ordering::SeqCst), 1);
	assert_eq!(
		observer.outcomes.lock().unwrap().as_slice(),
		[NativeErrorCodeOrSuccess::Error(NativeErrorCode::Cancelled)]
	);
}

#[tokio::test]
async fn revocation_is_checked_before_signing_and_again_before_transport() {
	let calls = Arc::new(AtomicUsize::new(0));
	let transports = Arc::new(AtomicUsize::new(0));
	let host = FakeHost::new(
		Arc::new(|| 1_000),
		Arc::new(TestSigner {
			entered: Arc::new(Notify::new()),
			release: None,
			calls: calls.clone(),
		}),
		Arc::new(BlockingTransport {
			entered: Arc::new(Notify::new()),
			release: Arc::new(Notify::new()),
			calls: transports.clone(),
		}),
		Arc::new(RecordingObserver::default()),
	);
	host.grant("festival", &["attestation:schema_by_id"]).unwrap();
	let before_request = authorize(&host, request("request-revoke-001", "nonce-revoke-00001"));
	host.revoke("festival");
	assert_eq!(
		host.execute(before_request).await.unwrap_err().code,
		NativeErrorCode::PermissionRevoked
	);
	assert_eq!((calls.load(Ordering::SeqCst), transports.load(Ordering::SeqCst)), (0, 0));

	let entered = Arc::new(Notify::new());
	let release = Arc::new(Notify::new());
	let signer = Arc::new(TestSigner {
		entered: entered.clone(),
		release: Some(release.clone()),
		calls: calls.clone(),
	});
	let host = Arc::new(FakeHost::new(
		Arc::new(|| 1_000),
		signer,
		Arc::new(BlockingTransport {
			entered: Arc::new(Notify::new()),
			release: Arc::new(Notify::new()),
			calls: transports.clone(),
		}),
		Arc::new(RecordingObserver::default()),
	));
	host.grant("festival", &["attestation:schema_by_id"]).unwrap();
	let during_request = authorize(&host, request("request-revoke-002", "nonce-revoke-00002"));
	let running_host = host.clone();
	let task = tokio::spawn(async move { running_host.execute(during_request).await });
	entered.notified().await;
	host.revoke("festival");
	release.notify_one();
	assert_eq!(task.await.unwrap().unwrap_err().code, NativeErrorCode::PermissionRevoked);
	assert_eq!(transports.load(Ordering::SeqCst), 0);

	let consent_entered = Arc::new(Notify::new());
	let consent_release = Arc::new(Notify::new());
	let host = Arc::new(FakeHost::new(
		Arc::new(|| 1_000),
		Arc::new(TestSigner {
			entered: consent_entered.clone(),
			release: Some(consent_release.clone()),
			calls,
		}),
		Arc::new(BlockingTransport {
			entered: Arc::new(Notify::new()),
			release: Arc::new(Notify::new()),
			calls: transports.clone(),
		}),
		Arc::new(RecordingObserver::default()),
	));
	host.grant("festival", &["attestation:schema_by_id"]).unwrap();
	let consent_request = authorize(&host, request("request-consent-delay", "nonce-consent-delay"));
	let nonce = consent_request.consent.nonce.clone();
	let running_host = host.clone();
	let task = tokio::spawn(async move { running_host.execute(consent_request).await });
	consent_entered.notified().await;
	host.revoke_consent(&nonce);
	consent_release.notify_one();
	assert_eq!(task.await.unwrap().unwrap_err().code, NativeErrorCode::PermissionRevoked);
	assert_eq!(transports.load(Ordering::SeqCst), 0);
	assert_eq!(
		host.execute(request("request-consent-replay", &nonce)).await.unwrap_err().code,
		NativeErrorCode::Replay
	);
}

#[test]
fn candidate_network_requires_explicit_mode_and_production_rejects_pending_activation() {
	NetworkIdentity::orbis_candidate().validate().unwrap();
	assert_eq!(
		NetworkIdentity::orbis_production().validate().unwrap_err().code,
		NativeErrorCode::UnsupportedRuntime
	);
}
