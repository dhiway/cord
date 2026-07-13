use std::{
	collections::{HashMap, HashSet},
	sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::contract::{Capability, HostRequest, NativeError, NativeErrorCode};

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
		Ok("p0-fake-host-signature".into())
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
	permissions: HashMap<String, HashSet<Capability>>,
	revoked: HashSet<String>,
	consumed_nonces: HashSet<String>,
	pre_cancelled: HashSet<String>,
	inflight: HashMap<String, watch::Sender<bool>>,
	terminal: HashSet<String>,
	seen: HashSet<String>,
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

	pub fn grant(&self, application_id: impl Into<String>, capabilities: &[Capability]) {
		self.state
			.lock()
			.expect("fake-host lock is not poisoned")
			.permissions
			.insert(application_id.into(), capabilities.iter().copied().collect());
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
			self.finish(&request_id, outcome);
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
			if request.capability == Capability::Unsupported
				|| !state
					.permissions
					.get(&request.application_id)
					.is_some_and(|permissions| permissions.contains(&request.capability))
			{
				return Err(NativeError::new(
					NativeErrorCode::PermissionDenied,
					"capability not granted by host",
				));
			}
			let scope = request.scope_name()?;
			if !request.consent.scope.iter().any(|item| item == scope) {
				return Err(NativeError::new(
					NativeErrorCode::PermissionDenied,
					"consent does not cover method",
				));
			}
			if request.consent.expires_at <= (self.now)() {
				return Err(NativeError::new(NativeErrorCode::ConsentExpired, "consent expired"));
			}
			if !state.consumed_nonces.insert(request.consent.nonce.clone()) {
				return Err(NativeError::new(
					NativeErrorCode::Replay,
					"consent nonce already used",
				));
			}
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
		if self
			.state
			.lock()
			.expect("fake-host lock is not poisoned")
			.revoked
			.contains(&request.application_id)
		{
			return Err(NativeError::new(
				NativeErrorCode::PermissionRevoked,
				"permission revoked during signing",
			));
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

	fn finish(&self, request_id: &str, outcome: NativeErrorCodeOrSuccess) {
		let notify = {
			let mut state = self.state.lock().expect("fake-host lock is not poisoned");
			state.inflight.remove(request_id);
			state.terminal.insert(request_id.into())
		};
		if notify {
			self.observer.terminal(request_id, outcome);
		}
	}
}

fn cancelled(message: &str) -> NativeError {
	NativeError::new(NativeErrorCode::Cancelled, message)
}
