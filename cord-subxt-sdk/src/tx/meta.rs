use crate::{
	client::Client,
	error::{Error, Result},
	tx::dynamic,
};
use codec::Encode;
use scale_value::{scale, Value};
use sp_core::{hashing::blake2_256, sr25519, H256};
use sp_runtime::{generic::Era, MultiSignature};
use std::marker::PhantomData;
use subxt::{
	tx::{DynamicPayload, Payload},
	utils::AccountId32,
};

const META_TAG: [u8; 8] = *b"_meta_tx";
const DEFAULT_EXTENSION_VERSION: u8 = 0;

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
	Auto,
	Disabled,
	Custom([u8; 32]),
}

impl Default for MetadataMode {
	fn default() -> Self {
		Self::Auto
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

/// Minimal trait implemented by key pairs that can sign meta transaction payloads.
pub trait MetaSigner {
	fn account_id(&self) -> AccountId32;
	fn sign_meta_payload(&self, message: [u8; 32]) -> MultiSignature;
}

impl MetaSigner for subxt_signer::sr25519::Keypair {
	fn account_id(&self) -> AccountId32 {
		<subxt_signer::sr25519::Keypair as subxt::tx::Signer<crate::params::config::CordConfig>>::account_id(self)
	}

	fn sign_meta_payload(&self, message: [u8; 32]) -> MultiSignature {
		let signature = self.sign(&message);
		let signature = sr25519::Signature::from_raw(signature.0);
		MultiSignature::Sr25519(signature)
	}
}

pub async fn dispatch_call_with_meta<S: MetaSigner>(
	client: &Client,
	call: DynamicPayload,
	meta_signer: &S,
	opts: MetaTxOptions,
) -> Result<DynamicPayload> {
	let metadata = client.metadata();
	let call_bytes = call.encode_call_data(&metadata).map_err(|e| Error::Codec(e.to_string()))?;

	let meta_account = meta_signer.account_id();
	let nonce = match opts.nonce {
		MetaNonce::Auto => client
			.legacy_methods()
			.system_account_next_index(&meta_account)
			.await
			.map_err(|e| Error::Transport(e.to_string()))? as u32,
		MetaNonce::Manual(value) => value,
	};

	let runtime_version = client.runtime_version().await?;
	let spec_version = runtime_version.spec_version;
	let tx_version = runtime_version.transaction_version;
	let genesis_hash = H256(client.online().genesis_hash().0);

	let metadata_hash = match opts.metadata {
		MetadataMode::Disabled => None,
		MetadataMode::Custom(hash) => Some(hash),
		MetadataMode::Auto => match client.metadata_hash().await {
			Ok(hash) => Some(hash),
			Err(err) => {
				log::warn!(
					"metadata hash unavailable ({err}); disabling CheckMetadataHash implicit"
				);
				None
			},
		},
	};

	let extension_version = if opts.extension_version == 0 {
		DEFAULT_EXTENSION_VERSION
	} else {
		opts.extension_version
	};

	let bare_extension = BareExtension {
		era: match opts.era {
			MetaEra::Immortal => Era::Immortal,
		},
		nonce,
		spec_version,
		tx_version,
		genesis_hash,
		metadata_hash,
	};

	let bare_tuple = bare_extension.as_tuple();
	let implicit = bare_extension.implicit_payload();

	let message = signature_message(extension_version, &call_bytes, &bare_tuple, &implicit);
	let signature = meta_signer.sign_meta_payload(message);

	let extension = (
		raw::VerifySignature::Signed { signature, account: meta_account.clone() },
		bare_tuple.clone(),
	);

	let raw_meta = RawMetaTx { call: RawRuntimeCall(&call_bytes), extension_version, extension };

	let encoded_meta = raw_meta.encode();
	let meta_value = decode_meta_value(&metadata, &encoded_meta)?;
	let args = Value::named_composite([("meta_tx", meta_value)]);
	dynamic::build_call(client, "MetaTx", "dispatch", args).await
}

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

fn decode_meta_value(metadata: &subxt::Metadata, bytes: &[u8]) -> Result<Value<()>> {
	let ty = find_type(metadata, &["pallet_meta_tx", "MetaTx"])
		.ok_or_else(|| Error::Params("metadata missing pallet_meta_tx::MetaTx type".into()))?;
	let mut cursor = bytes;
	let value = scale::decode_as_type(&mut cursor, ty.id, metadata.types())
		.map_err(|e| Error::Codec(format!("meta transaction decode via metadata failed: {e}")))?;
	if !cursor.is_empty() {
		return Err(Error::Codec("meta transaction argument not fully consumed".into()));
	}
	Ok(value.remove_context())
}

fn find_type(metadata: &subxt::Metadata, path: &[&str]) -> Option<scale_info::PortableType> {
	metadata
		.types()
		.types
		.iter()
		.find(|ty| {
			let segments = &ty.ty.path.segments;
			segments.len() == path.len()
				&& segments.iter().map(|seg| seg.as_str()).zip(path.iter()).all(|(a, b)| a == *b)
		})
		.cloned()
}

#[derive(Clone)]
struct BareExtension {
	era: Era,
	nonce: u32,
	spec_version: u32,
	tx_version: u32,
	genesis_hash: H256,
	metadata_hash: Option<[u8; 32]>,
}

impl BareExtension {
	fn as_tuple(&self) -> RawBareExtension {
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

	fn implicit_payload(&self) -> ImplicitPayload {
		ImplicitPayload {
			meta_tag: META_TAG,
			non_zero_sender: (),
			spec_version: self.spec_version,
			tx_version: self.tx_version,
			genesis_hash: self.genesis_hash,
			era_hash: self.genesis_hash,
			nonce_placeholder: (),
			metadata_hash: self.metadata_hash,
		}
	}
}

#[derive(Encode)]
struct RawMetaTx<'a> {
	call: RawRuntimeCall<'a>,
	extension_version: u8,
	extension: (raw::VerifySignature, RawBareExtension),
}

#[derive(Clone)]
struct RawRuntimeCall<'a>(&'a [u8]);

impl<'a> Encode for RawRuntimeCall<'a> {
	fn encode_to<T: codec::Output + ?Sized>(&self, dest: &mut T) {
		dest.write(self.0);
	}
}

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

mod raw {
	use super::*;

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
		Signed { signature: MultiSignature, account: AccountId32 },
		Disabled,
	}
}
