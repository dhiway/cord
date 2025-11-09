use crate::{
	context::ExampleContext,
	cord,
	formatting::token_to_string,
	view_types::{scale, PacketSnapshotView, RegistryInfoView},
};
use codec::{Compact, Decode, Encode, Output};
use color_eyre::eyre::{eyre, Result, WrapErr};
use cord::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use hex::encode as hex_encode;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use scale_value::{scale as scale_decoder, Value};
use sp_runtime::MultiSignature;
use std::{
	sync::atomic::{AtomicU64, Ordering},
	time::{SystemTime, UNIX_EPOCH},
};
use subxt::{
	dynamic::{view_function_call, DecodedValueThunk},
	ext::subxt_core::Metadata,
	utils::AccountId32,
	view_functions::DynamicPayload,
};

type Ss58Identifier = cord::runtime_types::cord_primitives::identifier::Ss58Identifier;

pub struct RegistryViewData {
	pub info: RegistryInfoView,
}

pub struct PacketViewData {
	pub snapshot: PacketSnapshotView,
}

pub struct RegisterViewClient<'a> {
	metadata: Metadata,
	ctx: &'a ExampleContext,
}

impl<'a> RegisterViewClient<'a> {
	pub async fn new(ctx: &'a ExampleContext, _block_hash: subxt::utils::H256) -> Result<Self> {
		let metadata = ctx.client.metadata();
		Ok(Self { metadata, ctx })
	}

	pub async fn registry_overview(&self, registry: &Ss58Identifier) -> Result<RegistryViewData> {
		let (info_auth, info_bytes) = self.auth_arg("register-info");
		let info_args = vec![info_bytes, Self::encoded_identifier(registry)];
		let payload = self.call_view_payload("info", info_args).await?;
		let info_bytes = match self.split_option_payload("info", &payload)? {
			Some(bytes) => bytes,
			None => {
				#[cfg(debug_assertions)]
				if let Err(err) = self.debug_authorization_failure("info", &info_auth).await {
					tracing::warn!(target: "anchor", "Register::info debug probe failed: {err:?}");
				}
				return Err(eyre!(
					"registry {} not found via view function",
					token_to_string(registry)
				));
			},
		};
		let info = self.decode_registry_info_bytes(info_bytes)?;

		Ok(RegistryViewData { info })
	}

	pub async fn packet_snapshot(
		&self,
		registry: &Ss58Identifier,
		packet: &Ss58Identifier,
	) -> Result<PacketViewData> {
		let (packet_auth, packet_auth_bytes) = self.auth_arg("register-packet");
		let packet_args = vec![
			packet_auth_bytes,
			Self::encoded_identifier(registry),
			Self::encoded_identifier(packet),
			Self::encoded_option_u32(None),
		];
		let payload = self.call_view_payload("packet", packet_args).await?;
		let snapshot_bytes = match self.split_option_payload("packet", &payload)? {
			Some(bytes) => bytes,
			None => {
				#[cfg(debug_assertions)]
				if let Err(err) = self.debug_authorization_failure("packet", &packet_auth).await {
					tracing::warn!(target: "anchor", "Register::packet debug probe failed: {err:?}");
				}
				return Err(eyre!(
					"packet {} not found via view function",
					token_to_string(packet)
				));
			},
		};
		let snapshot = self.decode_packet_snapshot_bytes(snapshot_bytes)?;
		Ok(PacketViewData { snapshot })
	}

	async fn call_view_payload(&self, name: &str, args: Vec<Vec<u8>>) -> Result<Vec<u8>> {
		let thunk = self.call_view_raw(name, args).await?;
		let mut cursor = thunk.encoded();
		match self.read_result_tag(name, &mut cursor)? {
			ResultTag::Ok => {
				self.log_payload_head(name, "payload-after-result", cursor);
				Ok(cursor.to_vec())
			},
			ResultTag::Err => {
				let err_hex = hex::encode(cursor);
				Err(eyre!(
					"Register::{name} runtime error (DispatchError SCALE bytes: 0x{err_hex})"
				))
			},
		}
	}

	fn split_option_payload<'a>(&self, name: &str, payload: &'a [u8]) -> Result<Option<&'a [u8]>> {
		self.log_payload_head(name, "option-bytes", payload);
		if payload.is_empty() {
			return Err(eyre!("Register::{name} payload empty while decoding Option"));
		}
		match payload.first().copied() {
			Some(0) => Ok(None),
			Some(1) => Ok(Some(&payload[1..])),
			Some(other) => {
				tracing::warn!(
					target: "anchor",
					"Register::{name} Option payload missing discriminant; first byte=0x{other:02x}, treating as Some(..)"
				);
				Ok(Some(payload))
			},
			None => Err(eyre!("Register::{name} Option payload missing discriminant byte")),
		}
	}

	fn decode_registry_info_bytes(&self, bytes: &[u8]) -> Result<RegistryInfoView> {
		self.decode_as("scale::RegistryInfo", bytes, RegistryInfoView::from)
			.or_else(|err| {
				tracing::warn!(
					target: "anchor",
					"Register::info canonical decode failed: {err:?}; attempting legacy schema"
				);
				self.decode_as("scale::legacy::RegistryInfo", bytes, RegistryInfoView::from)
					.map_err(|legacy_err| {
						eyre!(
							"Register::info payload decode failed; canonical_err={:?}; legacy_err={:?}",
							err,
							legacy_err
						)
					})
			})
	}

	fn decode_packet_snapshot_bytes(&self, bytes: &[u8]) -> Result<PacketSnapshotView> {
		self.decode_as("scale::PacketSnapshot", bytes, PacketSnapshotView::from)
	}

	fn decode_as<T, U, F>(&self, label: &str, bytes: &[u8], map: F) -> Result<U>
	where
		T: Decode,
		F: FnOnce(T) -> U,
	{
		let mut cursor = bytes;
		match T::decode(&mut cursor) {
			Ok(value) => Ok(map(value)),
			Err(err) => {
				self.log_payload_head(label, "decode-error-bytes", bytes);
				Err(eyre!("failed to decode {label}: {err:?}"))
			},
		}
	}

	async fn call_view_raw(&self, name: &str, args: Vec<Vec<u8>>) -> Result<DecodedValueThunk> {
		let payload = self.build_payload(name, args)?;
		let api = self
			.ctx
			.client
			.view_functions()
			.at_latest()
			.await
			.wrap_err("failed to query latest block for view function")?;
		let thunk = api
			.call(payload)
			.await
			.wrap_err_with(|| format!("view function Register::{name} failed"))?;
		let encoded = thunk.encoded();
		match encoded.first().copied() {
			Some(0x00) => tracing::debug!(
				target: "anchor",
				"Register::{name} SCALE wrapper → Result::Ok(..)"
			),
			Some(0x01) => tracing::debug!(
				target: "anchor",
				"Register::{name} SCALE wrapper → Result::Err(..)"
			),
			Some(tag) => tracing::debug!(
				target: "anchor",
				"Register::{name} SCALE wrapper → unknown leading byte 0x{tag:02x}"
			),
			None => {
				tracing::debug!(target: "anchor", "Register::{name} SCALE wrapper → empty payload")
			},
		}
		tracing::debug!(
			target: "anchor",
			"Register::{name} raw view bytes: 0x{} ({} bytes)",
			hex::encode(encoded),
			encoded.len()
		);
		Ok(thunk)
	}

	fn build_payload(&self, name: &str, args: Vec<Vec<u8>>) -> Result<DynamicPayload> {
		let pallet = self
			.metadata
			.pallet_by_name("Register")
			.ok_or_else(|| eyre!("Register pallet missing from metadata"))?;
		let vf = match pallet.view_function_by_name(name) {
			Some(vf) => vf,
			None => {
				let available =
					pallet.view_functions().map(|vf| vf.name()).collect::<Vec<_>>().join(", ");
				return Err(eyre!(
					"view function Register::{name} not found in metadata (available: {available})"
				));
			},
		};
		let inputs: Vec<_> = vf.inputs().collect();
		if inputs.len() != args.len() {
			return Err(eyre!(
				"Register::{name} expects {} args but {} were provided",
				inputs.len(),
				args.len()
			));
		}
		let mut values = Vec::with_capacity(inputs.len());
		for (data, meta) in args.into_iter().zip(inputs.iter()) {
			values.push(self.decode_arg(&data, meta.ty).wrap_err_with(|| {
				format!("failed to map argument for Register::{name} to scale_value")
			})?);
		}

		if let Some(output_ty) = self.metadata.types().resolve(vf.output_ty()) {
			let path_segments: Vec<_> =
				output_ty.path.segments.iter().map(|seg| seg.as_str()).collect();
			tracing::debug!(
				target: "anchor",
				"Register::{name} output type id={} path={:?} def={:?}",
				vf.output_ty(),
				path_segments,
				output_ty.type_def
			);
		}

		Ok(view_function_call(*vf.query_id(), values))
	}

	fn decode_arg(&self, bytes: &[u8], type_id: u32) -> Result<Value> {
		let mut cursor = bytes;
		let value = scale_decoder::decode_as_type(&mut cursor, type_id, self.metadata.types())
			.wrap_err("scale_value decode failed")?
			.remove_context();
		Ok(value)
	}

	fn read_result_tag(&self, name: &str, cursor: &mut &[u8]) -> Result<ResultTag> {
		if cursor.is_empty() {
			return Err(eyre!("Register::{name} missing SCALE Result discriminant: payload empty"));
		}
		let (first, rest) = cursor.split_first().unwrap();
		*cursor = rest;
		match first {
			0 => Ok(ResultTag::Ok),
			1 => Ok(ResultTag::Err),
			other => Err(eyre!("Register::{name} returned unknown SCALE Result tag 0x{other:02x}")),
		}
	}

	fn log_payload_head(&self, name: &str, label: &str, bytes: &[u8]) {
		let preview_len = bytes.len().min(32);
		let head = hex::encode(&bytes[..preview_len]);
		tracing::debug!(
			target: "anchor",
			"Register::{name} {label} len={} head=0x{}",
			bytes.len(),
			head
		);
	}

	#[cfg(debug_assertions)]
	async fn debug_authorization_failure(
		&self,
		name: &str,
		auth: &RawViewAuthorization,
	) -> Result<()> {
		use sp_core::blake2_128;
		let mut data = auth.account.encode();
		data.extend_from_slice(&auth.payload.0);
		data.extend(auth.signature.encode());
		let hash = blake2_128(&data);
		let snapshot = self.ctx.client.storage().at_latest().await?;
		let key = cord::storage().register().view_signature_uses(hash);
		let used = snapshot.fetch(&key).await?.is_some();
		tracing::warn!(
			target: "anchor",
			"Register::{name} authorization rejected; replay_entry_present={used}"
		);
		Ok(())
	}

	fn auth_arg(&self, purpose: &str) -> (RawViewAuthorization, Vec<u8>) {
		let auth = self.new_authorization(purpose);
		self.log_authorization_details(purpose, &auth);
		let mut buf = Vec::new();
		auth.encode_to(&mut buf);
		(auth, buf)
	}

	fn new_authorization(&self, purpose: &str) -> RawViewAuthorization {
		const MAX_VIEW_AUTH_LEN: usize = 128;
		let payload_raw = self.random_payload(purpose);
		assert!(
			payload_raw.len() <= MAX_VIEW_AUTH_LEN,
			"view authorization payload exceeded MaxViewAuthorizationLen; purpose={purpose}"
		);
		let payload = BoundedVec(payload_raw.clone());
		let signature = self.ctx.signer.sign_view_payload(&payload_raw);
		#[cfg(debug_assertions)]
		{
			use sp_runtime::{traits::Verify, AccountId32 as RuntimeAccountId};
			let mut raw = [0u8; 32];
			raw.copy_from_slice(self.ctx.account_id.as_ref());
			let account = RuntimeAccountId::from(raw);
			if !signature.verify(payload_raw.as_slice(), &account) {
				tracing::warn!(target: "anchor", "local view authorization signature failed verification");
			}
		}
		RawViewAuthorization { account: self.ctx.account_id.clone(), payload, signature }
	}

	fn log_authorization_details(&self, purpose: &str, auth: &RawViewAuthorization) {
		let payload_preview = String::from_utf8_lossy(&auth.payload.0);
		let (sig_scheme, sig_bytes) = match &auth.signature {
			MultiSignature::Sr25519(sig) => ("sr25519", hex_encode(sig.0)),
			MultiSignature::Ed25519(sig) => ("ed25519", hex_encode(sig.0)),
			MultiSignature::Ecdsa(sig) => ("ecdsa", hex_encode(sig.0)),
		};
		let signature_hex = hex_encode(auth.signature.encode());
		tracing::debug!(
			target: "anchor",
			"Register view auth → purpose={purpose}, account={:?}, payload=\"{payload_preview}\", signature_scheme={sig_scheme}, signature_raw=0x{sig_bytes}, encoded=0x{signature_hex}",
			auth.account,
		);
	}

	fn random_payload(&self, purpose: &str) -> Vec<u8> {
		static AUTH_NONCE: AtomicU64 = AtomicU64::new(1);
		let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
		let mut rng = thread_rng();
		let suffix: String =
			(&mut rng).sample_iter(&Alphanumeric).take(6).map(char::from).collect();
		let nonce = AUTH_NONCE.fetch_add(1, Ordering::Relaxed);
		format!("{purpose}:{ts}:{nonce}:{suffix}").into_bytes()
	}

	fn encode_identifier(buf: &mut Vec<u8>, token: &Ss58Identifier) {
		let raw = &(token.0).0;
		let len = u32::try_from(raw.len()).expect("identifier under 2^32 bytes");
		Compact(len).encode_to(buf);
		buf.extend_from_slice(raw);
	}

	fn encoded_identifier(token: &Ss58Identifier) -> Vec<u8> {
		let mut buf = Vec::new();
		Self::encode_identifier(&mut buf, token);
		buf
	}

	fn encode_option_u32(buf: &mut Vec<u8>, value: Option<u32>) {
		match value {
			Some(val) => {
				buf.push(1);
				buf.extend_from_slice(&val.to_le_bytes());
			},
			None => buf.push(0),
		}
	}

	fn encoded_option_u32(value: Option<u32>) -> Vec<u8> {
		let mut buf = Vec::new();
		Self::encode_option_u32(&mut buf, value);
		buf
	}
}

enum ResultTag {
	Ok,
	Err,
}

#[derive(Clone)]
struct RawViewAuthorization {
	account: AccountId32,
	payload: BoundedVec<u8>,
	signature: MultiSignature,
}

impl Encode for RawViewAuthorization {
	fn size_hint(&self) -> usize {
		self.account.size_hint() + self.payload.0.size_hint() + self.signature.size_hint()
	}

	fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
		self.account.encode_to(dest);
		self.payload.0.encode_to(dest);
		self.signature.encode_to(dest);
	}
}
