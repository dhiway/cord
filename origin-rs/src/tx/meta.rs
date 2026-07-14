use std::marker::PhantomData;

use codec::{Decode, Encode, Input, Output};
use scale_value::{scale, Value};
use sp_core::H256;
use sp_crypto_hashing::blake2_256;
use sp_runtime::{
	generic::{Era, ExtensionVersion},
	MultiSignature, MultiSigner,
};
use subxt::{
	tx::{DynamicPayload, Payload},
	Metadata, OnlineClient,
};

use crate::{
	client::{signer::Signer, Client},
	config::OriginConfig,
	types::error::OriginSdkError,
};

/// Meta transaction extension version used by the runtime.
pub const META_TX_VERSION: ExtensionVersion = 0;
const META_TAG: [u8; 8] = *b"_meta_tx";
const SPONSORED_INTENT_DOMAIN: &[u8] = b"orbis/meta-intent/v7";
/// Maximum encoded inner MetaTx accepted by the Orbis runtime.
pub const MAX_META_ENCODED_BYTES: usize = 65_536;

type Result<T> = std::result::Result<T, OriginSdkError>;

/// Strategy for supplying the meta transaction nonce.
#[derive(Clone, Copy)]
pub enum MetaNonce {
	Auto,
	Manual(u32),
}

impl Default for MetaNonce {
	fn default() -> Self {
		Self::Auto
	}
}

/// Controls whether the metadata hash extension participates in the payload.
#[derive(Clone, Copy)]
pub enum MetadataMode {
	Disabled,
	Custom([u8; 32]),
}

impl Default for MetadataMode {
	fn default() -> Self {
		Self::Disabled
	}
}

/// Era configuration for the meta transaction.
#[derive(Clone, Copy)]
pub enum MetaEra {
	Immortal,
}

impl Default for MetaEra {
	fn default() -> Self {
		Self::Immortal
	}
}

/// Tunables used while composing the meta transaction payload.
#[derive(Clone, Copy, Default)]
pub struct MetaTxOptions {
	pub nonce: MetaNonce,
	pub era: MetaEra,
	pub metadata: MetadataMode,
	pub extension_version: u8,
}

/// Debug-friendly parts of a meta transaction (available when we authored it locally).
#[derive(Clone)]
pub struct MetaTxDebug {
	pub call: Vec<u8>,
	pub bare: BareExtension,
	pub signer: MultiSigner,
	pub signature: MultiSignature,
}

/// Signed meta transaction blob (suitable for offline relay).
#[derive(Clone)]
pub struct SignedMetaTx {
	raw: Vec<u8>,
	extension_version: u8,
	debug: Option<MetaTxDebug>,
}

/// Runtime bindings captured by an Orbis sponsored intent.
///
/// The product operation deliberately supports the closed, ordinary-account lane only: score,
/// personhood, people-lite, resources and honour proofs are all disabled. Policy-bearing intents
/// require their own typed proof builders and must not be smuggled through this API as SCALE.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SponsoredIntentBindings {
	pub nonce: u32,
	pub era: Era,
	pub spec_version: u32,
	pub transaction_version: u32,
	pub genesis_hash: H256,
	/// Hash of the mortal era birth block committed by `CheckMortality`.
	pub mortality_hash: H256,
	pub metadata_hash: Option<[u8; 32]>,
}

/// A participant-authorized current-wire Orbis MetaTx, ready for a distinct sponsor to submit.
#[derive(Clone)]
pub struct SponsoredIntent {
	signed: SignedMetaTx,
	participant: origin_primitives::AccountId,
	intent_commitment: H256,
	bindings: SponsoredIntentBindings,
}

impl SponsoredIntent {
	pub fn participant(&self) -> &origin_primitives::AccountId {
		&self.participant
	}

	pub fn intent_commitment(&self) -> H256 {
		self.intent_commitment
	}

	pub fn bindings(&self) -> SponsoredIntentBindings {
		self.bindings
	}

	pub fn encoded_meta_tx(&self) -> &[u8] {
		self.signed.raw()
	}

	pub(crate) fn signed(&self) -> &SignedMetaTx {
		&self.signed
	}
}

/// Prepare and participant-sign the active Orbis v8/v7-intent MetaTx wire.
///
/// This is intentionally not the legacy bare MetaTx helper below. It mirrors the active runtime
/// extension ordering, including paid-ingress intent commitment, policy/storage hashes and the
/// metadata implicit. The resulting bytes are decoded against live metadata before they leave the
/// SDK, so runtime wire drift fails closed.
pub(crate) async fn prepare_sponsored_intent<S: Signer + ?Sized>(
	metadata: &Metadata,
	call_bytes: &[u8],
	participant: &S,
	bindings: SponsoredIntentBindings,
) -> Result<SponsoredIntent> {
	if bindings.era.is_immortal() {
		return Err(OriginSdkError::InvalidInput(
			"sponsored intents require a finite mortal era".into(),
		));
	}

	let account = participant.account_id();
	let policy_proofs = raw::PolicyProofsV6::default();
	let metadata_extension = raw::CheckMetadataHash {
		mode: if bindings.metadata_hash.is_some() {
			raw::MetadataMode::Enabled
		} else {
			raw::MetadataMode::Disabled
		},
	};
	let preimage = raw::IntentPreimageV7 {
		domain: SPONSORED_INTENT_DOMAIN.to_vec(),
		extension_version: META_TX_VERSION,
		genesis_hash: bindings.genesis_hash,
		spec_version: bindings.spec_version,
		transaction_version: bindings.transaction_version,
		inner_signer: account.clone(),
		call_hash: H256(blake2_256(call_bytes)),
		mortality: bindings.era,
		nonce: bindings.nonce,
		policy_proofs_hash: H256(blake2_256(&policy_proofs.encode())),
		storage_extension_hash: H256(blake2_256(&[])),
		metadata_extension_hash: H256(blake2_256(&metadata_extension.encode())),
		metadata_implicit: bindings.metadata_hash,
	};
	let intent_commitment = H256(blake2_256(&preimage.encode()));
	let bare = raw::SponsoredBareExtension {
		consume: raw::ConsumePaidMetaIngress(preimage),
		marker: raw::MetaTxMarker(PhantomData),
		non_zero_sender: raw::CheckNonZeroSender(PhantomData),
		check_spec_version: raw::CheckSpecVersion(PhantomData),
		check_tx_version: raw::CheckTxVersion(PhantomData),
		check_genesis: raw::CheckGenesis(PhantomData),
		mortality: raw::CheckMortality(bindings.era),
		nonce: raw::CheckNonce(bindings.nonce),
		identity_policies: raw::MetaIdentityBoundPolicies::default(),
		storage: raw::ValidateStorageCalls(PhantomData),
		metadata: metadata_extension,
	};
	let implicit = raw::SponsoredImplicit {
		consume: (),
		meta_tag: META_TAG,
		non_zero_sender: (),
		spec_version: bindings.spec_version,
		transaction_version: bindings.transaction_version,
		genesis_hash: bindings.genesis_hash,
		mortality_hash: bindings.mortality_hash,
		nonce: (),
		identity_policies: ((), (), ()),
		storage: (),
		metadata_hash: bindings.metadata_hash,
	};
	let mut payload = Vec::new();
	META_TX_VERSION.encode_to(&mut payload);
	payload.extend_from_slice(call_bytes);
	bare.encode_to(&mut payload);
	implicit.encode_to(&mut payload);
	let signature = participant.sign_payload(&blake2_256(&payload)).await;
	let raw = raw::SponsoredMetaTx {
		call: RawRuntimeCall(call_bytes),
		extension_version: META_TX_VERSION,
		extension: raw::SponsoredExtension {
			verify: raw::ActiveVerifySignature::Signed { signature, account: account.clone() },
			bare,
		},
	}
	.encode();
	ensure_meta_encoded_len(raw.len())?;
	decode_meta_value(metadata, &raw)?;

	Ok(SponsoredIntent {
		signed: SignedMetaTx::from_parts(raw, META_TX_VERSION, None),
		participant: account,
		intent_commitment,
		bindings,
	})
}

impl SignedMetaTx {
	fn from_parts(raw: Vec<u8>, extension_version: u8, debug: Option<MetaTxDebug>) -> Self {
		Self { raw, extension_version, debug }
	}

	/// Raw encoded meta transaction (no length prefix).
	pub fn encode(&self) -> Vec<u8> {
		if let Some(debug) = &self.debug {
			let raw = encode_meta_tx(
				self.extension_version,
				&debug.call,
				&debug.bare.as_tuple(),
				&debug.signer,
				&debug.signature,
			);
			return raw;
		}
		self.raw.clone()
	}

	/// Access the optional debug bundle (present when we built the payload locally).
	pub fn debug(&self) -> Option<&MetaTxDebug> {
		self.debug.as_ref()
	}

	/// Decode a wire blob into `SignedMetaTx`, validating shape against metadata.
	pub fn decode_with_metadata(metadata: &Metadata, bytes: impl AsRef<[u8]>) -> Result<Self> {
		let raw = bytes.as_ref().to_vec();
		ensure_meta_encoded_len(raw.len())?;
		decode_meta_value(metadata, &raw)?;
		Ok(Self::from_parts(raw, META_TX_VERSION, None))
	}

	pub fn raw(&self) -> &[u8] {
		&self.raw
	}
}

impl Encode for SignedMetaTx {
	fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
		dest.write(&self.encode());
	}
}

impl Decode for SignedMetaTx {
	fn decode<I: Input>(input: &mut I) -> std::result::Result<Self, codec::Error> {
		let capacity = input.remaining_len()?.unwrap_or(0);
		let mut bytes = Vec::with_capacity(capacity);
		while let Ok(b) = input.read_byte() {
			bytes.push(b);
		}
		Ok(SignedMetaTx::from_parts(bytes, META_TX_VERSION, None))
	}
}

/// Build, sign, and wrap a call for meta dispatch. The returned payload should be
/// submitted by the relayer as a normal extrinsic.
pub async fn dispatch_call_with_meta<S: Signer>(
	client: &Client,
	call: DynamicPayload,
	meta_signer: &S,
	opts: MetaTxOptions,
) -> Result<DynamicPayload> {
	let metadata = client.metadata();
	let call_bytes = call
		.encode_call_data(&metadata)
		.map_err(|e| OriginSdkError::Encode(e.to_string()))?;

	let account = meta_signer.account_id();
	let meta_identifier = meta_signer.account_identifier();
	let nonce = match opts.nonce {
		MetaNonce::Auto => client
			.online()
			.tx()
			.account_nonce(&account)
			.await
			.map_err(|e| OriginSdkError::Nonce(e.to_string()))?,
		MetaNonce::Manual(value) => value as u64,
	};

	let metadata_hash = match opts.metadata {
		MetadataMode::Disabled => None,
		MetadataMode::Custom(hash) => Some(hash),
	};

	let mut bare_extension = build_meta_tx_bare_ext(client.online(), nonce, metadata_hash).await?;
	bare_extension.era = match opts.era {
		MetaEra::Immortal => Era::Immortal,
	};
	let version =
		if opts.extension_version == 0 { META_TX_VERSION } else { opts.extension_version };

	let preimage = meta_tx_sign_payload(version, &call_bytes, &bare_extension);
	let signature: MultiSignature = meta_signer.sign_payload(&preimage).await;

	let signed =
		assemble_meta_tx(version, &call_bytes, bare_extension, &meta_identifier, &signature);
	let encoded_len = u32::try_from(signed.raw().len())
		.map_err(|_| OriginSdkError::InvalidInput("meta-tx exceeds u32 length".into()))?;
	let meta_value = meta_tx_value_from_signed(&metadata, &signed)?;
	Ok(subxt::dynamic::tx("MetaTx", "dispatch", vec![meta_value, Value::u128(encoded_len.into())]))
}

/// Build a bare meta-tx extension using on-chain runtime information.
pub async fn build_meta_tx_bare_ext(
	client: &OnlineClient<OriginConfig>,
	nonce: u64,
	metadata_hash: Option<[u8; 32]>,
) -> Result<BareExtension> {
	let runtime_version = client.runtime_version();
	let nonce_u32 = u32::try_from(nonce)
		.map_err(|_| OriginSdkError::InvalidInput("meta-tx nonce overflow".into()))?;

	Ok(BareExtension {
		era: Era::Immortal,
		nonce: nonce_u32,
		spec_version: runtime_version.spec_version,
		tx_version: runtime_version.transaction_version,
		genesis_hash: H256(client.genesis_hash().0),
		metadata_hash,
	})
}

/// Build the signing preimage: (version, call_bytes, bare_ext, bare_ext.implicit()).
pub fn meta_tx_sign_payload(
	version: u8,
	call_bytes: &[u8],
	bare_extension: &BareExtension,
) -> [u8; 32] {
	let bare_tuple = bare_extension.as_tuple();
	let implicit = bare_extension.implicit_payload();
	signature_message(version, call_bytes, &bare_tuple, &implicit)
}

/// Assemble a meta transaction from its components.
pub fn assemble_meta_tx(
	extension_version: u8,
	call_bytes: &[u8],
	bare_extension: BareExtension,
	meta_signer: &MultiSigner,
	signature: &MultiSignature,
) -> SignedMetaTx {
	let raw = encode_meta_tx(
		extension_version,
		call_bytes,
		&bare_extension.as_tuple(),
		meta_signer,
		signature,
	);

	SignedMetaTx::from_parts(
		raw,
		extension_version,
		Some(MetaTxDebug {
			call: call_bytes.to_vec(),
			bare: bare_extension,
			signer: meta_signer.clone(),
			signature: signature.clone(),
		}),
	)
}

/// Convert a signed meta transaction to a dynamic value using runtime metadata.
pub fn meta_tx_value_from_signed(metadata: &Metadata, signed: &SignedMetaTx) -> Result<Value> {
	ensure_meta_encoded_len(signed.raw().len())?;
	decode_meta_value(metadata, signed.raw())
}

fn ensure_meta_encoded_len(len: usize) -> Result<()> {
	if len > MAX_META_ENCODED_BYTES {
		return Err(OriginSdkError::InvalidInput(format!(
			"encoded MetaTx exceeds runtime limit of {MAX_META_ENCODED_BYTES} bytes"
		)));
	}
	Ok(())
}

/// Build the signing preimage used by pallet-meta-tx tests and runtime.
fn signature_message(
	version: u8,
	call_bytes: &[u8],
	bare_extension: &RawBareExtension,
	implicit: &ImplicitPayload,
) -> [u8; 32] {
	let mut buffer = Vec::new();
	version.encode_to(&mut buffer);
	buffer.extend_from_slice(call_bytes);
	bare_extension.encode_to(&mut buffer);
	implicit.encode_to(&mut buffer);
	blake2_256(&buffer)
}

fn decode_meta_value(metadata: &Metadata, bytes: &[u8]) -> Result<Value<()>> {
	let ty = find_type(metadata, &["pallet_meta_tx", "MetaTx"])
		.ok_or_else(|| OriginSdkError::Metadata("pallet_meta_tx::MetaTx type missing".into()))?;
	let mut cursor = bytes;
	let value = scale::decode_as_type(&mut cursor, ty.id, metadata.types())
		.map_err(|e| OriginSdkError::Decode(format!("meta-tx decode: {e}")))?;
	if !cursor.is_empty() {
		return Err(OriginSdkError::Decode("meta-tx arg not fully consumed".into()));
	}
	Ok(value.remove_context())
}

fn find_type(metadata: &Metadata, path: &[&str]) -> Option<scale_info::PortableType> {
	metadata
		.types()
		.types
		.iter()
		.find(|ty| {
			let segments = &ty.ty.path.segments;
			segments.len() == path.len() &&
				segments.iter().map(|seg| seg.as_str()).zip(path.iter()).all(|(a, b)| a == *b)
		})
		.cloned()
}

/// In-memory representation of the bare meta-tx extension (without VerifySignature).
#[derive(Clone)]
pub struct BareExtension {
	pub era: Era,
	pub nonce: u32,
	pub spec_version: u32,
	pub tx_version: u32,
	pub genesis_hash: H256,
	pub metadata_hash: Option<[u8; 32]>,
}

impl BareExtension {
	/// Convert to the raw extension tuple used in MetaTxExtension.
	pub fn as_tuple(&self) -> RawBareExtension {
		(
			raw::MetaTxMarker(PhantomData),
			raw::CheckNonZeroSender(PhantomData),
			raw::CheckSpecVersion(PhantomData),
			raw::CheckTxVersion(PhantomData),
			raw::CheckGenesis(PhantomData),
			raw::CheckMortality(self.era),
			raw::CheckNonce(self.nonce),
			raw::CheckMetadataHash {
				mode: match self.metadata_hash {
					Some(_) => raw::MetadataMode::Enabled,
					None => raw::MetadataMode::Disabled,
				},
			},
		)
	}

	/// AdditionalSigned payload for the bare extension tuple.
	fn implicit_payload(&self) -> ImplicitPayload {
		ImplicitPayload {
			meta_tag: META_TAG,
			non_zero_sender: (),
			spec_version: self.spec_version,
			tx_version: self.tx_version,
			genesis_hash: self.genesis_hash,
			// For immortal era, frame_system::CheckMortality additional_signed is default hash.
			era_hash: H256::default(),
			nonce_placeholder: (),
			metadata_hash: self.metadata_hash,
		}
	}

	pub fn implicit_bytes(&self) -> Vec<u8> {
		self.implicit_payload().encode()
	}
}

/// Raw extension tuple for the bare extensions:
/// (MetaTxMarker, CheckNonZeroSender, CheckSpecVersion, CheckTxVersion,
///  CheckGenesis, CheckMortality, CheckNonce, CheckMetadataHash)
type RawBareExtension = (
	raw::MetaTxMarker,
	raw::CheckNonZeroSender,
	raw::CheckSpecVersion,
	raw::CheckTxVersion,
	raw::CheckGenesis,
	raw::CheckMortality,
	raw::CheckNonce,
	raw::CheckMetadataHash,
);

/// Full raw extension type for MetaTx:
/// (VerifySignature, MetaTxMarker, CheckNonZeroSender, CheckSpecVersion,
///  CheckTxVersion, CheckGenesis, CheckMortality, CheckNonce, CheckMetadataHash)
type RawExtension = (
	raw::VerifySignature,
	raw::MetaTxMarker,
	raw::CheckNonZeroSender,
	raw::CheckSpecVersion,
	raw::CheckTxVersion,
	raw::CheckGenesis,
	raw::CheckMortality,
	raw::CheckNonce,
	raw::CheckMetadataHash,
);

#[derive(Encode)]
struct RawMetaTx<'a> {
	call: RawRuntimeCall<'a>,
	extension_version: u8,
	extension: RawExtension,
}

#[derive(Clone)]
struct RawRuntimeCall<'a>(&'a [u8]);

impl<'a> Encode for RawRuntimeCall<'a> {
	fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
		dest.write(self.0);
	}
}

/// AdditionalSigned payload for the bare extension tuple.
#[derive(Clone, Encode)]
struct ImplicitPayload {
	meta_tag: [u8; 8],
	non_zero_sender: (),
	spec_version: u32,
	tx_version: u32,
	genesis_hash: H256,
	era_hash: H256,
	nonce_placeholder: (),
	metadata_hash: Option<[u8; 32]>,
}

fn encode_meta_tx(
	extension_version: u8,
	call: &[u8],
	bare_tuple: &RawBareExtension,
	account: &MultiSigner,
	signature: &MultiSignature,
) -> Vec<u8> {
	let extension = (
		raw::VerifySignature::Signed { signature: signature.clone(), account: account.clone() },
		bare_tuple.0.clone(),
		bare_tuple.1.clone(),
		bare_tuple.2.clone(),
		bare_tuple.3.clone(),
		bare_tuple.4.clone(),
		bare_tuple.5.clone(),
		bare_tuple.6.clone(),
		bare_tuple.7.clone(),
	);

	RawMetaTx { call: RawRuntimeCall(call), extension_version, extension }.encode()
}

mod raw {
	use super::*;

	#[derive(Clone, Decode, Encode)]
	pub struct IntentPreimageV7 {
		pub domain: Vec<u8>,
		pub extension_version: ExtensionVersion,
		pub genesis_hash: H256,
		pub spec_version: u32,
		pub transaction_version: u32,
		pub inner_signer: origin_primitives::AccountId,
		pub call_hash: H256,
		pub mortality: Era,
		pub nonce: u32,
		pub policy_proofs_hash: H256,
		pub storage_extension_hash: H256,
		pub metadata_extension_hash: H256,
		pub metadata_implicit: Option<[u8; 32]>,
	}

	#[derive(Clone, Encode)]
	pub struct ConsumePaidMetaIngress(pub IntentPreimageV7);

	#[derive(Clone, Default, Encode)]
	pub struct PolicyProofsV6 {
		pub personhood: Option<()>,
		pub people_lite: Option<()>,
		pub resources: Option<()>,
	}

	#[derive(Clone, Default, Encode)]
	pub struct MetaAccountBoundPoliciesV6(pub PolicyProofsV6);

	#[derive(Clone, Default, Encode)]
	pub struct MetaIdentityBoundPolicies(
		pub Option<()>,
		pub MetaAccountBoundPoliciesV6,
		pub Option<()>,
	);

	#[derive(Clone, Encode)]
	pub struct ValidateStorageCalls(pub PhantomData<()>);

	#[derive(Clone, Encode)]
	pub struct SponsoredBareExtension {
		pub consume: ConsumePaidMetaIngress,
		pub marker: MetaTxMarker,
		pub non_zero_sender: CheckNonZeroSender,
		pub check_spec_version: CheckSpecVersion,
		pub check_tx_version: CheckTxVersion,
		pub check_genesis: CheckGenesis,
		pub mortality: CheckMortality,
		pub nonce: CheckNonce,
		pub identity_policies: MetaIdentityBoundPolicies,
		pub storage: ValidateStorageCalls,
		pub metadata: CheckMetadataHash,
	}

	#[derive(Clone, Encode)]
	#[allow(dead_code)]
	pub enum ActiveVerifySignature {
		Disabled,
		Signed { signature: MultiSignature, account: origin_primitives::AccountId },
	}

	#[derive(Clone, Encode)]
	pub struct SponsoredExtension {
		pub verify: ActiveVerifySignature,
		pub bare: SponsoredBareExtension,
	}

	#[derive(Encode)]
	pub struct SponsoredMetaTx<'a> {
		pub call: RawRuntimeCall<'a>,
		pub extension_version: ExtensionVersion,
		pub extension: SponsoredExtension,
	}

	#[derive(Clone, Encode)]
	pub struct SponsoredImplicit {
		pub consume: (),
		pub meta_tag: [u8; 8],
		pub non_zero_sender: (),
		pub spec_version: u32,
		pub transaction_version: u32,
		pub genesis_hash: H256,
		pub mortality_hash: H256,
		pub nonce: (),
		pub identity_policies: ((), (), ()),
		pub storage: (),
		pub metadata_hash: Option<[u8; 32]>,
	}

	#[derive(Clone, Encode)]
	pub struct MetaTxMarker(pub PhantomData<()>);

	#[derive(Clone, Encode)]
	pub struct CheckNonZeroSender(pub PhantomData<()>);

	#[derive(Clone, Encode)]
	pub struct CheckSpecVersion(pub PhantomData<()>);

	#[derive(Clone, Encode)]
	pub struct CheckTxVersion(pub PhantomData<()>);

	#[derive(Clone, Encode)]
	pub struct CheckGenesis(pub PhantomData<()>);

	#[derive(Clone, Encode)]
	pub struct CheckMortality(pub Era);

	#[derive(Clone, Encode)]
	pub struct CheckNonce(#[codec(compact)] pub u32);

	#[derive(Clone, Encode)]
	pub struct CheckMetadataHash {
		pub mode: MetadataMode,
	}

	#[derive(Clone, Encode)]
	pub enum MetadataMode {
		Disabled,
		Enabled,
	}

	#[derive(Clone, Encode)]
	#[allow(dead_code)]
	pub enum VerifySignature {
		Signed { signature: MultiSignature, account: MultiSigner },
		Disabled,
	}
}

#[cfg(test)]
mod sponsored_tests {
	use super::*;

	#[test]
	fn current_intent_preimage_fixture_matches_wire_shape() {
		let fixture =
			include_bytes!("../../../origin/orbis/runtime/fixtures/meta-v8/intent-preimage.scale");
		let mut cursor = &fixture[..];
		let decoded = raw::IntentPreimageV7::decode(&mut cursor).expect("current intent fixture");
		assert!(cursor.is_empty());
		assert_eq!(decoded.domain, SPONSORED_INTENT_DOMAIN);
		assert_eq!(decoded.extension_version, META_TX_VERSION);
		assert_eq!(decoded.transaction_version, 8);
		assert_eq!(decoded.mortality, Era::Immortal);
		assert_eq!(decoded.encode(), fixture);
	}

	#[test]
	fn closed_policy_lane_has_canonical_default_encoding() {
		assert_eq!(raw::PolicyProofsV6::default().encode(), [0, 0, 0]);
		assert_eq!(raw::MetaIdentityBoundPolicies::default().encode(), [0, 0, 0, 0, 0]);
		assert!(raw::ValidateStorageCalls(PhantomData).encode().is_empty());
		assert_eq!(raw::CheckMetadataHash { mode: raw::MetadataMode::Enabled }.encode(), [1]);
	}

	#[test]
	fn active_verify_signature_keeps_disabled_as_zero_and_signed_as_one() {
		let disabled = raw::ActiveVerifySignature::Disabled.encode();
		assert_eq!(disabled, [0]);
		let signed = raw::ActiveVerifySignature::Signed {
			signature: MultiSignature::Ed25519(sp_core::ed25519::Signature::from_raw([7; 64])),
			account: origin_primitives::AccountId::new([9; 32]),
		}
		.encode();
		assert_eq!(signed[0], 1);
	}

	#[test]
	fn encoded_meta_limit_matches_runtime_boundary() {
		assert!(ensure_meta_encoded_len(MAX_META_ENCODED_BYTES).is_ok());
		assert!(ensure_meta_encoded_len(MAX_META_ENCODED_BYTES + 1).is_err());
	}
}
