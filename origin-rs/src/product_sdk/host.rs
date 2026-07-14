use std::{
	collections::{HashMap, HashSet},
	sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::contract::{validate_method_scope, Consent, HostRequest, NativeError, NativeErrorCode};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignedRequest {
	pub request: HostRequest,
	pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostResponse {
	pub finalized_hash: String,
}

#[async_trait]
pub trait HostSigner: Send + Sync {
	async fn sign(&self, request: &HostRequest) -> Result<String, NativeError>;
}

#[async_trait]
pub trait HostTransport: Send + Sync {
	async fn submit(
		&self,
		request: SignedRequest,
		cancelled: watch::Receiver<bool>,
	) -> Result<HostResponse, NativeError>;
}

pub trait TerminalObserver: Send + Sync {
	fn terminal(&self, request_id: &str, outcome: NativeErrorCodeOrSuccess);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeErrorCodeOrSuccess {
	Success,
	Error(NativeErrorCode),
}

struct DefaultSigner;

#[async_trait]
impl HostSigner for DefaultSigner {
	async fn sign(&self, _request: &HostRequest) -> Result<String, NativeError> {
		Ok("native-fake-host-signature".into())
	}
}

struct DefaultTransport;

#[async_trait]
impl HostTransport for DefaultTransport {
	async fn submit(
		&self,
		_request: SignedRequest,
		_cancelled: watch::Receiver<bool>,
	) -> Result<HostResponse, NativeError> {
		Ok(HostResponse { finalized_hash: format!("0x{}", "ab".repeat(32)) })
	}
}

struct NoopObserver;

impl TerminalObserver for NoopObserver {
	fn terminal(&self, _request_id: &str, _outcome: NativeErrorCodeOrSuccess) {}
}

#[derive(Default)]
struct State {
	permissions: HashMap<String, HashSet<String>>,
	revoked: HashSet<String>,
	consents: HashMap<String, ConsentRecord>,
	revoked_consents: HashSet<String>,
	consumed_nonces: HashSet<String>,
	pre_cancelled: HashSet<String>,
	inflight: HashMap<String, watch::Sender<bool>>,
	terminal: HashSet<String>,
	seen: HashSet<String>,
}

#[derive(Clone)]
struct ConsentRecord {
	application_id: String,
	scope: Vec<String>,
	expires_at: u64,
	claimed_by: Option<String>,
}

/// A deterministic, transport-neutral host used to prove the shared host contract.
///
/// Permission, consent, cancellation, replay and signer ordering live here. Runtime semantics do
/// not: production clients must still derive those from runtime metadata and runtime APIs.
pub struct FakeHost {
	state: Mutex<State>,
	now: Arc<dyn Fn() -> u64 + Send + Sync>,
	signer: Arc<dyn HostSigner>,
	transport: Arc<dyn HostTransport>,
	observer: Arc<dyn TerminalObserver>,
}

impl Default for FakeHost {
	fn default() -> Self {
		Self::new(
			Arc::new(|| 1_000),
			Arc::new(DefaultSigner),
			Arc::new(DefaultTransport),
			Arc::new(NoopObserver),
		)
	}
}

impl FakeHost {
	pub fn new(
		now: Arc<dyn Fn() -> u64 + Send + Sync>,
		signer: Arc<dyn HostSigner>,
		transport: Arc<dyn HostTransport>,
		observer: Arc<dyn TerminalObserver>,
	) -> Self {
		Self { state: Mutex::new(State::default()), now, signer, transport, observer }
	}

	pub fn grant<S: AsRef<str>>(
		&self,
		application_id: impl Into<String>,
		method_scopes: &[S],
	) -> Result<(), NativeError> {
		let application_id = application_id.into();
		if application_id.is_empty() || method_scopes.is_empty() {
			return Err(NativeError::new(
				NativeErrorCode::InvalidInput,
				"host permissions require an application and exact method scopes",
			));
		}
		let mut scopes = HashSet::new();
		for scope in method_scopes {
			let scope = scope.as_ref();
			validate_method_scope(scope).map_err(|_| {
				NativeError::new(NativeErrorCode::InvalidInput, "unrecognized host method scope")
			})?;
			if !scopes.insert(scope.to_owned()) {
				return Err(NativeError::new(
					NativeErrorCode::InvalidInput,
					"duplicate host method scope",
				));
			}
		}
		self.state
			.lock()
			.expect("fake-host lock is not poisoned")
			.permissions
			.insert(application_id, scopes);
		Ok(())
	}

	pub fn issue_consent(
		&self,
		application_id: impl Into<String>,
		consent: &Consent,
	) -> Result<(), NativeError> {
		let application_id = application_id.into();
		if application_id.is_empty()
			|| consent.scope.is_empty()
			|| consent.expires_at == 0
			|| !(16..=128).contains(&consent.nonce.len())
			|| consent.scope.iter().any(|scope| validate_method_scope(scope).is_err())
			|| consent.scope.iter().collect::<HashSet<_>>().len() != consent.scope.len()
		{
			return Err(NativeError::new(
				NativeErrorCode::InvalidInput,
				"invalid host-issued consent",
			));
		}
		let mut state = self.state.lock().expect("fake-host lock is not poisoned");
		if state.consents.contains_key(&consent.nonce)
			|| state.consumed_nonces.contains(&consent.nonce)
		{
			return Err(NativeError::new(
				NativeErrorCode::Conflict,
				"consent nonce already issued or consumed",
			));
		}
		state.consents.insert(
			consent.nonce.clone(),
			ConsentRecord {
				application_id,
				scope: consent.scope.clone(),
				expires_at: consent.expires_at,
				claimed_by: None,
			},
		);
		Ok(())
	}

	pub fn revoke_consent(&self, nonce: &str) {
		self.state
			.lock()
			.expect("fake-host lock is not poisoned")
			.revoked_consents
			.insert(nonce.into());
	}

	pub fn revoke(&self, application_id: &str) {
		self.state
			.lock()
			.expect("fake-host lock is not poisoned")
			.revoked
			.insert(application_id.into());
	}

	pub fn cancel(&self, request_id: &str) {
		let mut state = self.state.lock().expect("fake-host lock is not poisoned");
		if let Some(sender) = state.inflight.get(request_id) {
			if !*sender.borrow() {
				let _ = sender.send(true);
			}
		} else {
			state.pre_cancelled.insert(request_id.into());
		}
	}

	pub async fn execute(&self, request: HostRequest) -> Result<HostResponse, NativeError> {
		request.validate()?;
		let request_id = request.request_id.clone();
		let claimed = {
			let mut state = self.state.lock().expect("fake-host lock is not poisoned");
			if !state.seen.insert(request_id.clone()) {
				return Err(NativeError::new(
					NativeErrorCode::Conflict,
					"duplicate or in-flight request_id",
				));
			}
			true
		};

		let result = self.execute_claimed(&request).await;
		if claimed {
			let outcome = match &result {
				Ok(_) => NativeErrorCodeOrSuccess::Success,
				Err(error) => NativeErrorCodeOrSuccess::Error(error.code),
			};
			self.finish(&request, outcome);
		}
		result
	}

	async fn execute_claimed(&self, request: &HostRequest) -> Result<HostResponse, NativeError> {
		let (sender, mut receiver) = {
			let mut state = self.state.lock().expect("fake-host lock is not poisoned");
			if state.pre_cancelled.remove(&request.request_id) {
				return Err(cancelled("request cancelled before signing"));
			}
			if state.revoked.contains(&request.application_id) {
				return Err(NativeError::new(
					NativeErrorCode::PermissionRevoked,
					"permission revoked before signing",
				));
			}
			let scope = request.scope_name()?;
			if !state
				.permissions
				.get(&request.application_id)
				.is_some_and(|permissions| permissions.contains(&scope))
			{
				return Err(NativeError::new(
					NativeErrorCode::PermissionDenied,
					"method not granted by host",
				));
			}
			if state.consumed_nonces.contains(&request.consent.nonce) {
				return Err(NativeError::new(
					NativeErrorCode::Replay,
					"consent nonce already used",
				));
			}
			let Some(consent) = state.consents.get(&request.consent.nonce).cloned() else {
				return Err(NativeError::new(
					NativeErrorCode::PermissionDenied,
					"host-issued consent is missing",
				));
			};
			if state.revoked_consents.contains(&request.consent.nonce) {
				return Err(NativeError::new(
					NativeErrorCode::PermissionRevoked,
					"host-issued consent was revoked",
				));
			}
			if consent.application_id != request.application_id
				|| consent.scope != request.consent.scope
				|| consent.expires_at != request.consent.expires_at
				|| !consent.scope.contains(&scope)
			{
				return Err(NativeError::new(
					NativeErrorCode::PermissionDenied,
					"request consent does not match the host-issued record",
				));
			}
			if consent.expires_at <= (self.now)() {
				return Err(NativeError::new(NativeErrorCode::ConsentExpired, "consent expired"));
			}
			if consent.claimed_by.is_some() {
				return Err(NativeError::new(
					NativeErrorCode::Replay,
					"consent nonce is in flight",
				));
			}
			state
				.consents
				.get_mut(&request.consent.nonce)
				.expect("consent was read while holding the same lock")
				.claimed_by = Some(request.request_id.clone());
			let (sender, receiver) = watch::channel(false);
			state.inflight.insert(request.request_id.clone(), sender.clone());
			(sender, receiver)
		};

		let signature = tokio::select! {
			biased;
			changed = receiver.changed() => {
				let _ = changed;
				return Err(cancelled("request cancelled while signing"));
			},
			result = self.signer.sign(request) => result?,
		};
		if *receiver.borrow() {
			return Err(cancelled("request cancelled while signing"));
		}
		{
			let mut state = self.state.lock().expect("fake-host lock is not poisoned");
			let scope = request.scope_name()?;
			if state.revoked.contains(&request.application_id)
				|| !state
					.permissions
					.get(&request.application_id)
					.is_some_and(|permissions| permissions.contains(&scope))
			{
				return Err(NativeError::new(
					NativeErrorCode::PermissionRevoked,
					"application grant was revoked during signing",
				));
			}
			if state.consumed_nonces.contains(&request.consent.nonce) {
				return Err(NativeError::new(
					NativeErrorCode::Replay,
					"consent nonce already used",
				));
			}
			let consent = state.consents.get(&request.consent.nonce).cloned().ok_or_else(|| {
				NativeError::new(
					NativeErrorCode::PermissionRevoked,
					"host-issued consent was invalidated",
				)
			})?;
			if state.revoked_consents.contains(&request.consent.nonce) {
				return Err(NativeError::new(
					NativeErrorCode::PermissionRevoked,
					"host-issued consent was revoked during signing",
				));
			}
			if consent.application_id != request.application_id
				|| consent.scope != request.consent.scope
				|| consent.expires_at != request.consent.expires_at
				|| consent.claimed_by.as_deref() != Some(request.request_id.as_str())
				|| !consent.scope.contains(&scope)
			{
				return Err(NativeError::new(
					NativeErrorCode::PermissionRevoked,
					"host-issued consent changed during signing",
				));
			}
			if consent.expires_at <= (self.now)() {
				return Err(NativeError::new(
					NativeErrorCode::ConsentExpired,
					"consent expired during signing",
				));
			}
			state.consents.remove(&request.consent.nonce);
			state.consumed_nonces.insert(request.consent.nonce.clone());
		}
		if *receiver.borrow() {
			return Err(cancelled("request cancelled before transport"));
		}

		let signed = SignedRequest { request: request.clone(), signature };
		let transport_cancelled = receiver.clone();
		let result = tokio::select! {
			biased;
			changed = receiver.changed() => {
				let _ = changed;
				Err(cancelled("in-flight request cancelled"))
			},
			result = self.transport.submit(signed, transport_cancelled) => result,
		};
		drop(sender);
		result
	}

	fn finish(&self, request: &HostRequest, outcome: NativeErrorCodeOrSuccess) {
		let notify = {
			let mut state = self.state.lock().expect("fake-host lock is not poisoned");
			state.inflight.remove(&request.request_id);
			if state.consents.get(&request.consent.nonce).is_some_and(|consent| {
				consent.claimed_by.as_deref() == Some(request.request_id.as_str())
			}) {
				state.consents.remove(&request.consent.nonce);
				state.consumed_nonces.insert(request.consent.nonce.clone());
			}
			state.terminal.insert(request.request_id.clone())
		};
		if notify {
			self.observer.terminal(&request.request_id, outcome);
		}
	}
}

fn cancelled(message: &str) -> NativeError {
	NativeError::new(NativeErrorCode::Cancelled, message)
}
