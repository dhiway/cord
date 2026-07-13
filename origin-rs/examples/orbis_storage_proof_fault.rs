//! Dev-network transaction-storage proof setup and fault injector.
//!
//! This tool is deliberately restricted to an explicit `--allow-dev-faults` flag, a loopback
//! endpoint, and the exact development runtime version. It does not add a runtime or node backdoor:
//! local Sudo uses the existing `System::set_storage` call to shorten the otherwise 201,600-block
//! retention window. The unsigned actions document that ordinary RPC submission is rejected by the
//! pool because this pallet call has mandatory-inherent dispatch class; they are not an inherent
//! provider fault-injection seam.

use clap::{Parser, Subcommand};
use codec::{Compact, Decode, Encode};
use oc::{
	client::{signer::SubxtSignerAdapter, OriginSigner},
	config::{build_orbis_params, OrbisClient, OrbisConfig},
	types::OriginAccount,
};
use scale_value::Composite;
use serde::Serialize;
use serde_json::Value as JsonValue;
use sp_crypto_hashing::twox_128;
use std::sync::Arc;
use subxt::{config::DefaultExtrinsicParamsBuilder, dynamic::Value};

#[derive(Debug, Parser)]
struct Args {
	#[clap(long, default_value = "ws://127.0.0.1:10810")]
	endpoint: String,
	#[clap(long, default_value = "//Alice")]
	seed: String,
	/// Required acknowledgement that the target is a disposable local chain.
	#[clap(long)]
	allow_dev_faults: bool,
	#[clap(subcommand)]
	action: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
	/// Read the live runtime metadata constants used by the proof-capacity gate.
	Inspect,
	/// Decode finalized System.BlockWeight and block bytes against live metadata constants.
	MeasureBlock {
		/// Exact finalized block hash to measure.
		#[clap(long)]
		block_hash: String,
	},
	/// Shorten retention and authorize the signer through the existing local Sudo key.
	Setup {
		#[clap(long, default_value_t = 12)]
		retention: u32,
		#[clap(long, default_value_t = 32)]
		transactions: u32,
		#[clap(long, default_value_t = 1_048_576)]
		bytes: u64,
	},
	/// Submit deterministic indexed content through the regular signed production call path.
	Store {
		#[clap(long, default_value_t = 2_048)]
		bytes: usize,
		#[clap(long, default_value_t = 0x5a)]
		fill: u8,
	},
	/// Submit an unsigned composite inherent with no proof.
	InjectNone,
	/// Submit an unsigned malformed proof through the normal pool and dispatch path.
	InjectInvalid,
}

#[derive(Debug, Serialize)]
struct Output {
	status: &'static str,
	action: &'static str,
	chain: String,
	block_hash: Option<String>,
	extrinsic_hash: Option<String>,
	events: Vec<String>,
	error: Option<String>,
}

#[derive(Debug, Serialize)]
struct InspectOutput {
	status: &'static str,
	chain: String,
	spec_version: u32,
	transaction_version: u32,
	max_block_transactions: u32,
	max_transaction_size: u32,
	block_length_scale: String,
	block_weights_scale: String,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
struct WeightParts {
	ref_time: u64,
	proof_size: u64,
}

impl WeightParts {
	fn checked_add(self, other: Self) -> Result<Self, Box<dyn std::error::Error>> {
		Ok(Self {
			ref_time: self.ref_time.checked_add(other.ref_time).ok_or("ref_time overflow")?,
			proof_size: self
				.proof_size
				.checked_add(other.proof_size)
				.ok_or("proof_size overflow")?,
		})
	}

	fn checked_sub(self, other: Self) -> Result<Self, Box<dyn std::error::Error>> {
		Ok(Self {
			ref_time: self
				.ref_time
				.checked_sub(other.ref_time)
				.ok_or("event ref_time exceeds System.BlockWeight for its dispatch class")?,
			proof_size: self
				.proof_size
				.checked_sub(other.proof_size)
				.ok_or("event proof_size exceeds System.BlockWeight for its dispatch class")?,
		})
	}
}

#[derive(Debug, Serialize)]
struct ExactRatio {
	numerator: u64,
	denominator: u64,
	fraction: f64,
}

#[derive(Debug, Serialize)]
struct WeightRatio {
	ref_time: ExactRatio,
	proof_size: ExactRatio,
}

#[derive(Debug, Serialize)]
struct ClassWeightMeasurement {
	consumed: WeightParts,
	corrected_extrinsic_event_total: WeightParts,
	block_weight_minus_corrected_event_total: WeightParts,
	configured_max_total: Option<WeightParts>,
	configured_max_ratio: Option<WeightRatio>,
}

#[derive(Debug, Serialize)]
struct ExtrinsicLengthMeasurement {
	index: u32,
	raw_bytes: u64,
	scale_encoded_bytes: u64,
}

#[derive(Debug, Serialize)]
struct MeasureBlockOutput {
	status: &'static str,
	chain: String,
	block_hash: String,
	block_number: u32,
	spec_version: u32,
	transaction_version: u32,
	metadata_hash: String,
	block_weight_scale: String,
	block_weights_constant_scale: String,
	block_length_constant_scale: String,
	normal: ClassWeightMeasurement,
	operational: ClassWeightMeasurement,
	mandatory: ClassWeightMeasurement,
	total_consumed: WeightParts,
	max_block: WeightParts,
	total_max_block_ratio: WeightRatio,
	corrected_extrinsic_event_count: u32,
	extrinsic_count: u32,
	header_encoded_bytes: u64,
	extrinsics_vector_prefix_bytes: u64,
	extrinsics_encoded_bytes: u64,
	block_encoded_bytes: u64,
	extrinsics: Vec<ExtrinsicLengthMeasurement>,
	block_length_limits: [u64; 3],
	max_block_length_bytes: u64,
	block_length_ratio: ExactRatio,
}

fn json_at<'a>(
	value: &'a JsonValue,
	path: &[&str],
) -> Result<&'a JsonValue, Box<dyn std::error::Error>> {
	let mut current = value;
	for field in path {
		current = current
			.get(*field)
			.ok_or_else(|| format!("metadata-decoded value lacks {}", path.join(".")))?;
	}
	Ok(current)
}

fn json_u64(value: &JsonValue, path: &[&str]) -> Result<u64, Box<dyn std::error::Error>> {
	json_at(value, path)?
		.as_u64()
		.ok_or_else(|| format!("metadata-decoded {} is not u64", path.join(".")).into())
}

fn weight_at(value: &JsonValue, path: &[&str]) -> Result<WeightParts, Box<dyn std::error::Error>> {
	let weight = json_at(value, path)?;
	Ok(WeightParts {
		ref_time: json_u64(weight, &["ref_time"])?,
		proof_size: json_u64(weight, &["proof_size"])?,
	})
}

fn optional_weight_at(
	value: &JsonValue,
	path: &[&str],
) -> Result<Option<WeightParts>, Box<dyn std::error::Error>> {
	let option = json_at(value, path)?;
	if option.is_null() {
		return Ok(None);
	}
	let name = option.get("name").and_then(JsonValue::as_str);
	match name {
		Some("None") => Ok(None),
		Some("Some") => {
			let inner = option
				.get("values")
				.and_then(JsonValue::as_array)
				.and_then(|values| values.first())
				.ok_or("metadata-decoded Option<Weight>::Some has no value")?;
			Ok(Some(WeightParts {
				ref_time: json_u64(inner, &["ref_time"])?,
				proof_size: json_u64(inner, &["proof_size"])?,
			}))
		},
		_ => Err(format!("metadata-decoded {} is not Option<Weight>", path.join(".")).into()),
	}
}

fn exact_ratio(numerator: u64, denominator: u64) -> Result<ExactRatio, Box<dyn std::error::Error>> {
	if denominator == 0 {
		return Err("runtime resource limit is zero".into());
	}
	Ok(ExactRatio { numerator, denominator, fraction: numerator as f64 / denominator as f64 })
}

fn weight_ratio(
	consumed: WeightParts,
	limit: WeightParts,
) -> Result<WeightRatio, Box<dyn std::error::Error>> {
	Ok(WeightRatio {
		ref_time: exact_ratio(consumed.ref_time, limit.ref_time)?,
		proof_size: exact_ratio(consumed.proof_size, limit.proof_size)?,
	})
}

fn event_class(value: &JsonValue) -> Result<&str, Box<dyn std::error::Error>> {
	let class = json_at(value, &["dispatch_info", "class"])?;
	class
		.get("name")
		.and_then(JsonValue::as_str)
		.or_else(|| class.as_str())
		.ok_or_else(|| "metadata-decoded dispatch class has no variant name".into())
}

fn metadata_constant<'a>(
	metadata: &'a subxt::Metadata,
	pallet: &str,
	constant: &str,
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
	Ok(metadata
		.pallet_by_name(pallet)
		.ok_or_else(|| format!("metadata pallet {pallet} is absent"))?
		.constant_by_name(constant)
		.ok_or_else(|| format!("metadata constant {pallet}.{constant} is absent"))?
		.value())
}

async fn measure_finalized_block(
	client: &OrbisClient,
	block_hash_text: &str,
	chain: String,
) -> Result<MeasureBlockOutput, Box<dyn std::error::Error>> {
	let block_hash = block_hash_text
		.parse::<subxt::config::substrate::H256>()
		.map_err(|error| format!("invalid --block-hash: {error}"))?;
	let block = client.blocks().at(block_hash).await?;
	let canonical_hash = format!("{:#x}", block.hash());
	if !canonical_hash.eq_ignore_ascii_case(block_hash_text) {
		return Err("fetched block hash differs from requested block hash".into());
	}

	let block_weight_address =
		subxt::dynamic::storage("System", "BlockWeight", Vec::<Value<()>>::new());
	let block_weight = block
		.storage()
		.fetch(&block_weight_address)
		.await?
		.ok_or("System.BlockWeight is absent at the requested block state")?;
	let block_weight_scale = format!("0x{}", hex::encode(block_weight.encoded()));
	let block_weight_json = serde_json::to_value(block_weight.to_value()?)?;

	let block_weights =
		client.constants().at(&subxt::dynamic::constant("System", "BlockWeights"))?;
	let block_weights_constant_scale = format!("0x{}", hex::encode(block_weights.encoded()));
	let block_weights_json = serde_json::to_value(block_weights.to_value()?)?;
	let block_length = client.constants().at(&subxt::dynamic::constant("System", "BlockLength"))?;
	let block_length_constant_scale = format!("0x{}", hex::encode(block_length.encoded()));
	let block_length_json = serde_json::to_value(block_length.to_value()?)?;

	let consumed = [
		weight_at(&block_weight_json, &["normal"])?,
		weight_at(&block_weight_json, &["operational"])?,
		weight_at(&block_weight_json, &["mandatory"])?,
	];
	let configured_max_total = [
		optional_weight_at(&block_weights_json, &["per_class", "normal", "max_total"])?,
		optional_weight_at(&block_weights_json, &["per_class", "operational", "max_total"])?,
		optional_weight_at(&block_weights_json, &["per_class", "mandatory", "max_total"])?,
	];
	let max_block = weight_at(&block_weights_json, &["max_block"])?;

	let extrinsics = block.extrinsics().await?;
	let mut extrinsic_lengths = Vec::with_capacity(extrinsics.len());
	let mut extrinsics_encoded_bytes = Compact(extrinsics.len() as u32).encoded_size() as u64;
	for details in extrinsics.iter() {
		let raw_bytes = details.bytes().len() as u64;
		let scale_encoded_bytes = raw_bytes
			.checked_add(Compact(raw_bytes as u32).encoded_size() as u64)
			.ok_or("extrinsic length overflow")?;
		extrinsics_encoded_bytes = extrinsics_encoded_bytes
			.checked_add(scale_encoded_bytes)
			.ok_or("block extrinsic length overflow")?;
		extrinsic_lengths.push(ExtrinsicLengthMeasurement {
			index: details.index(),
			raw_bytes,
			scale_encoded_bytes,
		});
	}
	let extrinsics_vector_prefix_bytes = Compact(extrinsics.len() as u32).encoded_size() as u64;
	let header_encoded_bytes = block.header().encoded_size() as u64;
	let block_encoded_bytes = header_encoded_bytes
		.checked_add(extrinsics_encoded_bytes)
		.ok_or("encoded block length overflow")?;

	let mut corrected_event_total = [WeightParts::default(); 3];
	let mut corrected_event_count = 0u32;
	for event in block.events().await?.iter() {
		let event = event?;
		if event.pallet_name() != "System"
			|| !matches!(event.variant_name(), "ExtrinsicSuccess" | "ExtrinsicFailed")
		{
			continue;
		}
		let fields = serde_json::to_value(event.field_values()?)?;
		let index = match event_class(&fields)? {
			"Normal" => 0,
			"Operational" => 1,
			"Mandatory" => 2,
			other => return Err(format!("unknown metadata-decoded dispatch class {other}").into()),
		};
		let weight = weight_at(&fields, &["dispatch_info", "weight"])?;
		corrected_event_total[index] = corrected_event_total[index].checked_add(weight)?;
		corrected_event_count = corrected_event_count
			.checked_add(1)
			.ok_or("corrected extrinsic event count overflow")?;
	}
	if corrected_event_count as usize != extrinsics.len() {
		return Err(format!(
			"System corrected-weight event count {} differs from block extrinsic count {}",
			corrected_event_count,
			extrinsics.len()
		)
		.into());
	}

	let class_measurement = |index: usize| -> Result<_, Box<dyn std::error::Error>> {
		Ok(ClassWeightMeasurement {
			consumed: consumed[index],
			corrected_extrinsic_event_total: corrected_event_total[index],
			block_weight_minus_corrected_event_total: consumed[index]
				.checked_sub(corrected_event_total[index])?,
			configured_max_total: configured_max_total[index],
			configured_max_ratio: configured_max_total[index]
				.map(|limit| weight_ratio(consumed[index], limit))
				.transpose()?,
		})
	};
	let total_consumed = consumed[0].checked_add(consumed[1])?.checked_add(consumed[2])?;
	let block_length_limits = [
		json_u64(&block_length_json, &["max", "normal"])?,
		json_u64(&block_length_json, &["max", "operational"])?,
		json_u64(&block_length_json, &["max", "mandatory"])?,
	];
	let max_block_length_bytes = *block_length_limits
		.iter()
		.max()
		.ok_or("runtime BlockLength has no dispatch classes")?;
	let runtime = client.runtime_version();

	Ok(MeasureBlockOutput {
		status: "ok",
		chain,
		block_hash: canonical_hash,
		block_number: block.number(),
		spec_version: runtime.spec_version,
		transaction_version: runtime.transaction_version,
		metadata_hash: format!("0x{}", hex::encode(client.metadata().hasher().hash())),
		block_weight_scale,
		block_weights_constant_scale,
		block_length_constant_scale,
		normal: class_measurement(0)?,
		operational: class_measurement(1)?,
		mandatory: class_measurement(2)?,
		total_consumed,
		max_block,
		total_max_block_ratio: weight_ratio(total_consumed, max_block)?,
		corrected_extrinsic_event_count: corrected_event_count,
		extrinsic_count: extrinsics.len() as u32,
		header_encoded_bytes,
		extrinsics_vector_prefix_bytes,
		extrinsics_encoded_bytes,
		block_encoded_bytes,
		extrinsics: extrinsic_lengths,
		block_length_limits,
		max_block_length_bytes,
		block_length_ratio: exact_ratio(block_encoded_bytes, max_block_length_bytes)?,
	})
}

fn retention_key() -> Vec<u8> {
	[twox_128(b"TransactionStorage"), twox_128(b"RetentionPeriod")].concat()
}

fn account_value(account: &sp_core::crypto::AccountId32) -> Value<()> {
	Value::from_bytes(<sp_core::crypto::AccountId32 as AsRef<[u8]>>::as_ref(account))
}

fn none_value() -> Value<()> {
	Value::variant("None", Composite::unnamed(Vec::new()))
}

fn invalid_proof_value() -> Value<()> {
	let proof = Value::named_composite([
		("chunk", Value::from_bytes([0x42u8; 32])),
		("proof", Value::unnamed_composite([Value::from_bytes([0x99u8; 32])])),
	]);
	Value::variant("Some", Composite::unnamed([proof]))
}

async fn submit_unsigned(
	client: &OrbisClient,
	proof: Value<()>,
) -> Result<Output, Box<dyn std::error::Error>> {
	let call = subxt::dynamic::tx("TransactionStorage", "apply_block_inherents", vec![proof]);
	let tx = client.tx().create_unsigned(&call)?;
	let extrinsic_hash = format!("{:?}", tx.hash());
	let progress = match tx.submit_and_watch().await {
		Ok(progress) => progress,
		Err(error) => {
			return Ok(Output {
				status: "pool-rejected",
				action: "inject",
				chain: String::new(),
				block_hash: None,
				extrinsic_hash: Some(extrinsic_hash),
				events: Vec::new(),
				error: Some(error.to_string()),
			})
		},
	};
	match progress.wait_for_finalized().await {
		Ok(in_block) => {
			let block_hash = format!("{:?}", in_block.block_hash());
			let events = in_block.fetch_events().await?;
			let mut names = Vec::new();
			for event in events.iter() {
				let event = event?;
				names.push(format!("{}.{}", event.pallet_name(), event.variant_name()));
			}
			let error = in_block.wait_for_success().await.err().map(|error| error.to_string());
			Ok(Output {
				status: if error.is_some() { "rejected" } else { "accepted" },
				action: "inject",
				chain: String::new(),
				block_hash: Some(block_hash),
				extrinsic_hash: Some(extrinsic_hash),
				events: names,
				error,
			})
		},
		Err(error) => Ok(Output {
			status: "pool-rejected",
			action: "inject",
			chain: String::new(),
			block_hash: None,
			extrinsic_hash: Some(extrinsic_hash),
			events: Vec::new(),
			error: Some(error.to_string()),
		}),
	}
}

async fn submit_signed(
	client: &OrbisClient,
	signer: &SubxtSignerAdapter,
	account_id: &origin_primitives::AccountId,
	call: subxt::tx::DynamicPayload,
	action: &'static str,
) -> Result<Output, Box<dyn std::error::Error>> {
	let nonce = client.tx().account_nonce(account_id).await?;
	let params =
		build_orbis_params(DefaultExtrinsicParamsBuilder::<OrbisConfig>::new().nonce(nonce));
	let tx = client.tx().create_signed(&call, signer, params).await?;
	let extrinsic_hash = format!("{:?}", tx.hash());
	let in_block = tx.submit_and_watch().await?.wait_for_finalized().await?;
	let block_hash = format!("{:?}", in_block.block_hash());
	let events = in_block.fetch_events().await?;
	let mut names = Vec::new();
	for event in events.iter() {
		let event = event?;
		names.push(format!("{}.{}", event.pallet_name(), event.variant_name()));
	}
	in_block.wait_for_success().await?;
	Ok(Output {
		status: "accepted",
		action,
		chain: String::new(),
		block_hash: Some(block_hash),
		extrinsic_hash: Some(extrinsic_hash),
		events: names,
		error: None,
	})
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();
	if !args.allow_dev_faults {
		return Err("refusing fault action without --allow-dev-faults".into());
	}
	let account =
		OriginAccount::from_uri(&args.seed, None).map_err(|error| format!("{error:?}"))?;
	let signer = OriginSigner::from_account(&account).map_err(|error| format!("{error:?}"))?;
	let account_id = signer.account_id();
	let signer = SubxtSignerAdapter::new(Arc::new(signer));
	if !(args.endpoint.starts_with("ws://127.0.0.1:") || args.endpoint.starts_with("ws://[::1]:")) {
		return Err("refusing a non-loopback endpoint".into());
	}
	let client = OrbisClient::from_url(&args.endpoint).await?;
	let runtime = client.runtime_version();
	if runtime.spec_version != 29 || runtime.transaction_version != 8 {
		return Err(format!(
			"refusing runtime {}/{}; expected Orbis 29/8",
			runtime.spec_version, runtime.transaction_version
		)
		.into());
	}
	let properties = "Orbis Local".to_owned();
	if let Action::MeasureBlock { block_hash } = &args.action {
		let output = measure_finalized_block(&client, block_hash, properties).await?;
		println!("{}", serde_json::to_string_pretty(&output)?);
		return Ok(());
	}
	if matches!(&args.action, Action::Inspect) {
		let metadata = client.metadata();
		let max_block_transactions = u32::decode(&mut metadata_constant(
			metadata,
			"TransactionStorage",
			"MaxBlockTransactions",
		)?)?;
		let max_transaction_size = u32::decode(&mut metadata_constant(
			metadata,
			"TransactionStorage",
			"MaxTransactionSize",
		)?)?;
		let output = InspectOutput {
			status: "ok",
			chain: properties,
			spec_version: runtime.spec_version,
			transaction_version: runtime.transaction_version,
			max_block_transactions,
			max_transaction_size,
			block_length_scale: format!(
				"0x{}",
				hex::encode(metadata_constant(metadata, "System", "BlockLength")?)
			),
			block_weights_scale: format!(
				"0x{}",
				hex::encode(metadata_constant(metadata, "System", "BlockWeights")?)
			),
		};
		println!("{}", serde_json::to_string_pretty(&output)?);
		return Ok(());
	}

	let mut output = match args.action {
		Action::Inspect => unreachable!("handled above"),
		Action::MeasureBlock { .. } => unreachable!("handled above"),
		Action::Setup { retention, transactions, bytes } => {
			if !(3..=128).contains(&retention) || transactions == 0 || bytes == 0 {
				return Err("retention must be 3..=128 and authorization must be non-zero".into());
			}
			let set_storage = subxt::dynamic::tx(
				"System",
				"set_storage",
				vec![Value::unnamed_composite([Value::unnamed_composite([
					Value::from_bytes(retention_key()),
					Value::from_bytes(retention.encode()),
				])])],
			);
			let authorize = subxt::dynamic::tx(
				"TransactionStorage",
				"authorize_account",
				vec![
					account_value(&account.account_id()),
					Value::u128(transactions.into()),
					Value::u128(bytes.into()),
				],
			);
			let batch = subxt::dynamic::tx(
				"Utility",
				"batch_all",
				vec![Value::unnamed_composite([set_storage.into_value(), authorize.into_value()])],
			);
			let sudo = subxt::dynamic::tx("Sudo", "sudo", vec![batch.into_value()]);
			submit_signed(&client, &signer, &account_id, sudo, "setup").await?
		},
		Action::Store { bytes, fill } => {
			if !(256..=65_536).contains(&bytes) {
				return Err("store bytes must be 256..=65536".into());
			}
			let store = subxt::dynamic::tx(
				"TransactionStorage",
				"store",
				vec![Value::from_bytes(vec![fill; bytes])],
			);
			submit_signed(&client, &signer, &account_id, store, "store").await?
		},
		Action::InjectNone => submit_unsigned(&client, none_value()).await?,
		Action::InjectInvalid => submit_unsigned(&client, invalid_proof_value()).await?,
	};
	output.chain = properties;
	println!("{}", serde_json::to_string_pretty(&output)?);
	Ok(())
}
