use codec::{Compact, Decode, Encode, Output};
use scale_value::{Composite, Value, ValueDef};
use sp_runtime::{generic::ExtensionVersion, MultiSignature};
use subxt::Metadata;

use crate::{config::OriginConfig, types::error::OriginSdkError};

/// Meta transaction extension version used by the runtime.
pub const META_TX_VERSION: ExtensionVersion = 0;

/// Mode used by the metadata hash signed extension.
#[derive(Clone, Copy, Debug, Encode, Decode, PartialEq, Eq)]
pub enum MetadataHashMode {
	Disabled,
	Enabled,
}

/// Mortality data carried by the meta-tx bare extension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mortality {
	pub era: sp_runtime::generic::Era,
	pub hash: subxt::utils::H256,
}

/// Bare extensions for meta-transactions (excludes VerifySignature).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetaTxBareExt {
	pub spec_version: u32,
	pub tx_version: u32,
	pub genesis_hash: subxt::utils::H256,
	pub mortality: Mortality,
	pub nonce: u32,
	pub metadata: MetadataHashMode,
	pub metadata_hash: Option<[u8; 32]>,
}

impl Encode for MetaTxBareExt {
	fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
		().encode_to(dest); // MetaTxMarker value
		().encode_to(dest); // CheckNonZeroSender value
		().encode_to(dest); // CheckSpecVersion value
		().encode_to(dest); // CheckTxVersion value
		().encode_to(dest); // CheckGenesis value
		self.mortality.era.encode_to(dest); // CheckMortality value
		Compact(self.nonce as u32).encode_to(dest); // CheckNonce value (Compact<u32>)
		self.metadata.encode_to(dest); // CheckMetadataHash mode
	}
}

impl Decode for MetaTxBareExt {
	fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
		let _: () = Decode::decode(input)?; // marker
		let _: () = Decode::decode(input)?; // non-zero sender
		let _: () = Decode::decode(input)?; // spec version
		let _: () = Decode::decode(input)?; // tx version
		let _: () = Decode::decode(input)?; // genesis
		let era = sp_runtime::generic::Era::decode(input)?;
		let Compact(nonce) = Compact::<u32>::decode(input)?;
		let metadata = MetadataHashMode::decode(input)?;

		Ok(Self {
			spec_version: 0,
			tx_version: 0,
			genesis_hash: subxt::utils::H256::default(),
			mortality: Mortality { era, hash: subxt::utils::H256::default() },
			nonce,
			metadata,
			metadata_hash: None,
		})
	}
}

impl MetaTxBareExt {
	/// Encoded `AdditionalSigned` payload for the bare extension tuple.
	pub fn implicit_bytes(&self) -> Vec<u8> {
		(
			*b"_meta_tx",
			(),                  // CheckNonZeroSender additional signed
			self.spec_version,   // CheckSpecVersion additional signed
			self.tx_version,     // CheckTxVersion additional signed
			self.genesis_hash,   // CheckGenesis additional signed
			self.mortality.hash, // CheckMortality additional signed (block hash at era birth)
			(),                  // CheckNonce additional signed
			self.metadata_hash,  // CheckMetadataHash additional signed
		)
			.encode()
	}
}

/// Wire format shared between signer and relayer (call is SCALE-encoded bytes).
#[derive(Clone, Encode, Decode)]
pub struct SignedMetaTxWire {
	pub call: Vec<u8>,
	pub extension_version: ExtensionVersion,
	pub verify: raw_meta::VerifySignature,
	pub bare: MetaTxBareExt,
}

/// Meta transaction wrapper with decoded call value for relayer usage.
pub struct SignedMetaTx {
	pub wire: SignedMetaTxWire,
	pub call_value: Value,
}

impl Clone for SignedMetaTx {
	fn clone(&self) -> Self {
		Self { wire: self.wire.clone(), call_value: self.call_value.clone() }
	}
}

impl SignedMetaTx {
	/// Encode the wire format for transport.
	pub fn encode(&self) -> Vec<u8> {
		self.wire.encode()
	}

	/// Decode from wire bytes using runtime metadata to recover the call value.
	pub fn decode_with_metadata(
		metadata: &Metadata,
		bytes: Vec<u8>,
	) -> Result<Self, OriginSdkError> {
		let wire = SignedMetaTxWire::decode(&mut &*bytes)
			.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
		let call_value = decode_call_value(metadata, &wire.call)?;
		Ok(Self { wire, call_value })
	}
}

/// Build a bare meta-tx extension using live chain values.
pub async fn build_meta_tx_bare_ext(
	client: &subxt::OnlineClient<OriginConfig>,
	nonce: u64,
	metadata_hash: Option<[u8; 32]>,
) -> Result<MetaTxBareExt, OriginSdkError> {
	let at = client.at_current_block().await.map_err(|e| OriginSdkError::Tx(e.to_string()))?;
	let spec_version = at.spec_version();
	let tx_version = at.transaction_version();
	let genesis_hash = client.genesis_hash();

	Ok(MetaTxBareExt {
		spec_version,
		tx_version,
		genesis_hash,
		mortality: Mortality { era: sp_runtime::generic::Era::Immortal, hash: genesis_hash },
		nonce: nonce as u32,
		metadata: if metadata_hash.is_some() {
			MetadataHashMode::Enabled
		} else {
			MetadataHashMode::Disabled
		},
		metadata_hash,
	})
}

/// Build the signing preimage used by pallet-meta-tx tests and runtime.
pub fn meta_tx_sign_payload(
	meta_tx_version: ExtensionVersion,
	call_bytes: &[u8],
	bare: &MetaTxBareExt,
) -> Vec<u8> {
	let mut encoded = Vec::new();

	// (META_TX_VERSION, call, ext.clone(), ext.implicit())
	meta_tx_version.encode_to(&mut encoded);
	encoded.extend_from_slice(call_bytes);
	bare.encode_to(&mut encoded);
	// `implicit()` encodes as a tuple of implicit values; do not wrap in a Vec prefix.
	encoded.extend_from_slice(&bare.implicit_bytes());

	sp_core::blake2_256(&encoded).to_vec()
}

/// Assemble the SignedMetaTx bundle (wire + decoded call value).
pub fn assemble_meta_tx(
	meta_tx_version: ExtensionVersion,
	call_bytes: &[u8],
	call_value: Value,
	bare: MetaTxBareExt,
	signer: &origin_primitives::AccountId,
	signature: &MultiSignature,
) -> SignedMetaTx {
	let verify =
		raw_meta::VerifySignature::Signed { signature: signature.clone(), account: signer.clone() };
	let wire = SignedMetaTxWire {
		call: call_bytes.to_vec(),
		extension_version: meta_tx_version,
		verify,
		bare,
	};
	SignedMetaTx { wire, call_value }
}

/// Turn a signed meta-tx into a dynamic `Value` accepted by pallet-meta-tx::dispatch.
pub fn meta_tx_value_from_signed(
	metadata: &Metadata,
	signed: &SignedMetaTx,
) -> Result<Value, OriginSdkError> {
	// Encode a raw MetaTx payload matching the runtime SCALE layout, then decode it via metadata
	// to obtain the exact Value shape expected by MetaTx::dispatch.
	let raw_bytes = encode_raw_meta_tx(signed);

	let meta_ty = find_meta_tx_type(metadata)?;
	let mut cursor: &[u8] = &raw_bytes;
	let value = scale_value::scale::decode_as_type(&mut cursor, meta_ty.id, metadata.types())
		.map_err(|e| OriginSdkError::Decode(e.to_string()))?
		.remove_context();
	if !cursor.is_empty() {
		return Err(OriginSdkError::Decode("meta-tx decode: bytes not fully consumed".into()));
	}

	// Validate by re-encoding as the MetaTx::dispatch argument type.
	let param_ty = meta_tx_dispatch_arg_type(metadata)?;
	let mut buf = Vec::new();
	scale_value::scale::encode_as_type(&value, param_ty, metadata.types(), &mut buf)?;

	Ok(value)
}

fn decode_call_value(metadata: &Metadata, call_bytes: &[u8]) -> Result<Value, OriginSdkError> {
	let call_ty = metadata.outer_enums().call_enum_ty();
	let value = scale_value::scale::decode_as_type(&mut &*call_bytes, call_ty, metadata.types())
		.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
	Ok(value.remove_context())
}

fn meta_tx_dispatch_arg_type(metadata: &Metadata) -> Result<u32, OriginSdkError> {
	let pallet = metadata
		.pallet_by_name("MetaTx")
		.ok_or_else(|| OriginSdkError::Metadata("MetaTx pallet not found".into()))?;
	let call = pallet
		.call_variants()
		.ok_or_else(|| OriginSdkError::Metadata("MetaTx pallet has no calls".into()))?
		.iter()
		.find(|c| c.name == "dispatch")
		.ok_or_else(|| OriginSdkError::Metadata("MetaTx.dispatch not found".into()))?;
	let param_ty = call
		.fields
		.first()
		.ok_or_else(|| OriginSdkError::Metadata("MetaTx.dispatch arg missing".into()))?
		.ty
		.id;
	Ok(param_ty)
}

#[allow(dead_code)]
fn multi_signature_value(sig: &MultiSignature) -> Value {
	use sp_runtime::MultiSignature::*;
	match sig {
		Ed25519(s) =>
			Value::variant("Ed25519", Composite::Unnamed(vec![Value::from_bytes(s.0.to_vec())])),
		Sr25519(s) =>
			Value::variant("Sr25519", Composite::Unnamed(vec![Value::from_bytes(s.0.to_vec())])),
		Ecdsa(s) =>
			Value::variant("Ecdsa", Composite::Unnamed(vec![Value::from_bytes(s.0.to_vec())])),
		Eth(s) =>
			Value::variant("Ecdsa", Composite::Unnamed(vec![Value::from_bytes(s.0.to_vec())])),
	}
}

#[allow(dead_code)]
fn mortality_value(era: &sp_runtime::generic::Era) -> Value {
	match era {
		sp_runtime::generic::Era::Immortal =>
			Value::variant("Immortal", Composite::Unnamed(vec![])),
		sp_runtime::generic::Era::Mortal(period, phase) => Value::variant(
			"Mortal",
			Composite::Unnamed(vec![Value::u128(*period as u128), Value::u128(*phase as u128)]),
		),
	}
}

#[allow(dead_code)]
fn metadata_hash_value(mode: MetadataHashMode) -> Value {
	match mode {
		MetadataHashMode::Disabled => Value::variant("Disabled", Composite::Unnamed(vec![])),
		MetadataHashMode::Enabled => Value::variant("Enabled", Composite::Unnamed(vec![])),
	}
}

#[allow(dead_code)]
fn unit_value() -> Value {
	Value { value: ValueDef::Composite(Composite::Unnamed(vec![])), context: () }
}

fn find_meta_tx_type(metadata: &Metadata) -> Result<scale_info::PortableType, OriginSdkError> {
	metadata
		.types()
		.types
		.iter()
		.find(|ty| {
			let segments = &ty.ty.path.segments;
			segments.len() == 2 &&
				segments[0].as_str() == "pallet_meta_tx" &&
				segments[1].as_str() == "MetaTx"
		})
		.cloned()
		.ok_or_else(|| OriginSdkError::Metadata("pallet_meta_tx::MetaTx type not found".into()))
}

// Raw SCALE helpers to mirror the runtime MetaTx encoding.
mod raw_meta {
	use super::*;

	#[derive(Clone, Encode, Decode)]
	pub enum VerifySignature {
		Signed { signature: MultiSignature, account: origin_primitives::AccountId },
		Disabled,
	}

	#[derive(Clone, Encode)]
	pub struct MetaTxMarker(#[codec(skip)] pub core::marker::PhantomData<()>);
	#[derive(Clone, Encode)]
	pub struct CheckNonZeroSender(#[codec(skip)] pub core::marker::PhantomData<()>);
	#[derive(Clone, Encode)]
	pub struct CheckSpecVersion(#[codec(skip)] pub core::marker::PhantomData<()>);
	#[derive(Clone, Encode)]
	pub struct CheckTxVersion(#[codec(skip)] pub core::marker::PhantomData<()>);
	#[derive(Clone, Encode)]
	pub struct CheckGenesis(#[codec(skip)] pub core::marker::PhantomData<()>);
	#[derive(Clone, Encode)]
	pub struct CheckMortality(pub sp_runtime::generic::Era);
	#[derive(Clone, Encode)]
	pub struct CheckNonce(#[codec(compact)] pub u32);
	#[derive(Clone, Encode)]
	pub struct CheckMetadataHash {
		pub mode: RawMetadataHashMode,
	}

	#[derive(Clone, Encode)]
	pub enum RawMetadataHashMode {
		Disabled,
		Enabled,
	}

	#[derive(Clone)]
	pub struct RawCall<'a>(pub &'a [u8]);
	impl<'a> Encode for RawCall<'a> {
		fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
			dest.write(self.0);
		}
	}
}

fn encode_raw_meta_tx(signed: &SignedMetaTx) -> Vec<u8> {
	use raw_meta::*;

	let bare = &signed.wire.bare;
	let md_mode = match bare.metadata {
		MetadataHashMode::Disabled => raw_meta::RawMetadataHashMode::Disabled,
		MetadataHashMode::Enabled => raw_meta::RawMetadataHashMode::Enabled,
	};

	let ext = (
		signed.wire.verify.clone(),
		MetaTxMarker(Default::default()),
		CheckNonZeroSender(Default::default()),
		CheckSpecVersion(Default::default()),
		CheckTxVersion(Default::default()),
		CheckGenesis(Default::default()),
		CheckMortality(bare.mortality.era),
		CheckNonce(bare.nonce),
		CheckMetadataHash { mode: md_mode },
	);

	let raw = (RawCall(&signed.wire.call), signed.wire.extension_version, ext);
	raw.encode()
}
