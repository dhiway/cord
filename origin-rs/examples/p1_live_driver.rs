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

//! Hash-bound live AC7/AC8 driver used by `zombienet/p1-control-broker/run.py`.
//!
//! The driver uses `OriginClient` for both chains. This is deliberate: `OriginConfig` encodes the
//! exact current Origin/Orbis transaction-extension tuple and the runtime exposes the Orbis
//! fee/storage/revive extensions under the same metadata identifiers. Dynamic calls remain bound
//! to the metadata fetched from each live endpoint.

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use codec::{Decode, Encode};
use oc::{
	client::{signer::OriginSigner, EventEnvelope, OriginClient},
	p1_campaign::{
		expected_cases, provider_submit_call, AssertionRecord, CaseRecord, CaseStatus,
		DriverEvidence, EventRecord, FailureRecord, FinalizedBlock, ScenarioManifest,
	},
	types::OriginAccount,
};
use scale_value::{Composite, Value};
use serde_json::json;
use sp_core::crypto::AccountId32;
use std::{
	collections::{BTreeMap, BTreeSet},
	fs,
	path::PathBuf,
	str::FromStr,
	time::{Duration, Instant},
};
use subxt::{backend::rpc::RpcClient, dynamic, ext::subxt_rpcs::rpc_params, utils::H256};

const EXTENSIONS: &[&str] = &[
	"AuthorizeCall",
	"CheckNonZeroSender",
	"CheckSpecVersion",
	"CheckTxVersion",
	"CheckGenesis",
	"CheckMortality",
	"CheckNonce",
	"CheckWeight",
	"ChargeTransactionPayment",
	"ValidateStorageCalls",
	"CheckMetadataHash",
	"ReviveSetOrigin",
	"WeightReclaim",
];

#[derive(Debug, Parser)]
struct Args {
	#[arg(long)]
	phase: String,
	#[arg(long)]
	manifest: PathBuf,
	#[arg(long)]
	relay: String,
	#[arg(long)]
	orbis: String,
	#[arg(long)]
	relay_upgrade_wasm: PathBuf,
	#[arg(long)]
	orbis_upgrade_wasm: PathBuf,
	#[arg(long)]
	output: PathBuf,
	#[arg(long)]
	evidence_dir: PathBuf,
	#[arg(long, default_value_t = 5_100)]
	wait_seconds: u64,
}

#[derive(Clone)]
struct Chain {
	name: &'static str,
	endpoint: String,
	client: OriginClient,
	rpc: RpcClient,
}

#[derive(Clone, Debug)]
struct ObservedBlock {
	chain: String,
	number: u32,
	hash: H256,
	events: Vec<EventRecord>,
}

#[derive(Clone, Debug)]
struct TxTrace {
	block: ObservedBlock,
	tx_hash: String,
}

#[derive(Clone, Debug)]
struct AuraAuthorProof {
	block_hash: H256,
	authority_state_hash: H256,
	slot: u64,
	authority_index: usize,
	authority: Vec<u8>,
	digest_log: String,
}

#[derive(Debug, Decode)]
struct BrokerStatus {
	_core_count: u16,
	_private_pool_size: u32,
	_system_pool_size: u32,
	_last_committed_timeslice: u32,
	last_timeslice: u32,
}

impl Chain {
	async fn connect(name: &'static str, endpoint: String) -> Result<Self> {
		let client = OriginClient::connect(&endpoint).await?;
		let rpc = RpcClient::from_insecure_url(&endpoint).await?;
		Ok(Self { name, endpoint, client, rpc })
	}

	async fn reconnect(&mut self) -> Result<()> {
		self.client = OriginClient::connect(&self.endpoint).await?;
		self.rpc = RpcClient::from_insecure_url(&self.endpoint).await?;
		Ok(())
	}

	fn assert_runtime_contract(&self) -> Result<()> {
		let metadata = self.client.metadata();
		let control = metadata
			.pallet_by_name("CoretimeControl")
			.ok_or_else(|| anyhow!("{} metadata lacks CoretimeControl", self.name))?;
		if control.index() != 221 {
			bail!("{} CoretimeControl index is {}, expected 221", self.name, control.index())
		}
		for call in [
			"request_core_count",
			"submit_request",
			"retry_request",
			"acknowledge",
			"set_transport_hold",
			"release_held",
		] {
			if control.call_variant_by_name(call).is_none() {
				// Each side exposes the same pallet type even though policy permits only its role.
				bail!("{} CoretimeControl lacks call {call}", self.name)
			}
		}
		let version = metadata.extrinsic().transaction_extension_version_to_use_for_encoding();
		let observed = metadata
			.extrinsic()
			.transaction_extensions_by_version(version)
			.ok_or_else(|| anyhow!("{} has no extension layout for version {version}", self.name))?
			.map(|item| item.identifier())
			.collect::<Vec<_>>();
		if observed != EXTENSIONS {
			bail!("{} signed-extension layout mismatch: {observed:?}", self.name)
		}
		Ok(())
	}

	async fn finalized_hash(&self) -> Result<H256> {
		let value: String = self.rpc.request("chain_getFinalizedHead", rpc_params![]).await?;
		H256::from_str(&value).map_err(|error| anyhow!("bad finalized hash {value}: {error}"))
	}

	async fn header_number(&self, hash: H256) -> Result<u32> {
		let header = self.header_value(hash).await?;
		let number = header["number"].as_str().ok_or_else(|| anyhow!("header lacks number"))?;
		Ok(u32::from_str_radix(number.trim_start_matches("0x"), 16)?)
	}

	async fn header_value(&self, hash: H256) -> Result<serde_json::Value> {
		self.rpc
			.request("chain_getHeader", rpc_params![format!("{hash:#x}")])
			.await
			.map_err(Into::into)
	}

	async fn hash_at(&self, number: u32) -> Result<H256> {
		let value: String = self.rpc.request("chain_getBlockHash", rpc_params![number]).await?;
		H256::from_str(&value).map_err(|error| anyhow!("bad block hash {value}: {error}"))
	}

	async fn events_at(&self, hash: H256) -> Result<Vec<EventRecord>> {
		let events = self.client.online().events().at(hash).await?;
		let mut records = Vec::new();
		for event in events.iter() {
			let event = event?;
			records.push(EventRecord {
				chain: self.name.into(),
				block_hash: format!("{hash:#x}"),
				pallet: event.pallet_name().into(),
				variant: event.variant_name().into(),
				fields: format!("{:?}", event.field_values()?),
			});
		}
		Ok(records)
	}

	async fn observed_block(&self, hash: H256) -> Result<ObservedBlock> {
		Ok(ObservedBlock {
			chain: self.name.into(),
			number: self.header_number(hash).await?,
			hash,
			events: self.events_at(hash).await?,
		})
	}

	async fn storage_raw_at(
		&self,
		hash: H256,
		pallet: &str,
		entry: &str,
		keys: Vec<Value<()>>,
	) -> Result<Vec<u8>> {
		let address = dynamic::storage(pallet, entry, keys);
		Ok(self
			.client
			.online()
			.storage()
			.at(hash)
			.fetch(&address)
			.await?
			.map(|value| value.into_encoded())
			.unwrap_or_default())
	}

	async fn storage_raw(
		&self,
		pallet: &str,
		entry: &str,
		keys: Vec<Value<()>>,
	) -> Result<(H256, Vec<u8>)> {
		let hash = self.finalized_hash().await?;
		Ok((hash, self.storage_raw_at(hash, pallet, entry, keys).await?))
	}

	async fn submit(
		&self,
		signer: &OriginSigner,
		call: subxt::tx::DynamicPayload,
	) -> Result<TxTrace> {
		let outcome = self
			.client
			.tx()
			.using(signer.clone())
			.submit(call)
			.await?
			.wait_finalized()
			.await?;
		let hash = outcome.block.ok_or_else(|| anyhow!("finalized transaction has no block"))?;
		let block = ObservedBlock {
			chain: self.name.into(),
			number: self.header_number(hash).await?,
			hash,
			events: envelopes(self.name, hash, outcome.events),
		};
		Ok(TxTrace { block, tx_hash: format!("{:#x}", outcome.hash) })
	}

	async fn sudo(
		&self,
		signer: &OriginSigner,
		inner: subxt::tx::DynamicPayload,
	) -> Result<TxTrace> {
		self.submit(signer, dynamic::tx("Sudo", "sudo", vec![inner.into_value()])).await
	}

	async fn session_index_at(&self, hash: H256) -> Result<u32> {
		let raw = self.storage_raw_at(hash, "Session", "CurrentIndex", vec![]).await?;
		u32::decode(&mut &raw[..]).context("decode Session.CurrentIndex")
	}

	async fn wait_session(&self, after: H256, timeout: Duration) -> Result<Vec<ObservedBlock>> {
		let start_index = self.session_index_at(after).await?;
		let mut next = self.header_number(after).await?.saturating_add(1);
		let deadline = Instant::now() + timeout;
		let mut blocks = Vec::new();
		while Instant::now() < deadline {
			let head = self.finalized_hash().await?;
			let head_number = self.header_number(head).await?;
			while next <= head_number {
				let hash = self.hash_at(next).await?;
				blocks.push(self.observed_block(hash).await?);
				next += 1;
			}
			if self.session_index_at(head).await? > start_index {
				if !blocks
					.iter()
					.flat_map(|block| &block.events)
					.any(|event| event.pallet == "Session" && event.variant == "NewSession")
				{
					bail!("Session index changed without finalized Session.NewSession evidence")
				}
				return Ok(blocks);
			}
			tokio::time::sleep(Duration::from_secs(3)).await;
		}
		bail!("{} session did not rotate within {timeout:?}", self.name)
	}

	async fn wait_event(
		&self,
		after: H256,
		pallet: &str,
		variant: &str,
		timeout: Duration,
	) -> Result<Vec<ObservedBlock>> {
		let mut next = self.header_number(after).await?.saturating_add(1);
		let deadline = Instant::now() + timeout;
		let mut blocks = Vec::new();
		while Instant::now() < deadline {
			let head = self.finalized_hash().await?;
			let head_number = self.header_number(head).await?;
			while next <= head_number {
				let block = self.observed_block(self.hash_at(next).await?).await?;
				let found = block
					.events
					.iter()
					.any(|event| event.pallet == pallet && event.variant == variant);
				blocks.push(block);
				next += 1;
				if found {
					return Ok(blocks);
				}
			}
			tokio::time::sleep(Duration::from_secs(2)).await;
		}
		bail!("{} did not finalize {pallet}.{variant} within {timeout:?}", self.name)
	}

	async fn wait_finalized_after(&self, after: H256, timeout: Duration) -> Result<ObservedBlock> {
		let number = self.header_number(after).await?;
		let deadline = Instant::now() + timeout;
		while Instant::now() < deadline {
			let head = self.finalized_hash().await?;
			if self.header_number(head).await? > number {
				return self.observed_block(head).await;
			}
			tokio::time::sleep(Duration::from_secs(2)).await;
		}
		bail!("{} finality did not advance within {timeout:?}", self.name)
	}

	async fn wait_aura_author(
		&self,
		after: H256,
		expected_key: &[u8],
		timeout: Duration,
	) -> Result<(Vec<ObservedBlock>, AuraAuthorProof)> {
		let mut next = self.header_number(after).await?.saturating_add(1);
		let deadline = Instant::now() + timeout;
		let mut blocks = Vec::new();
		while Instant::now() < deadline {
			let head = self.finalized_hash().await?;
			let head_number = self.header_number(head).await?;
			while next <= head_number {
				let hash = self.hash_at(next).await?;
				let header = self.header_value(hash).await?;
				let block = self.observed_block(hash).await?;
				blocks.push(block);
				next += 1;
				let Some((slot, digest_log)) = aura_slot_from_header(&header)? else { continue };
				let parent = header["parentHash"]
					.as_str()
					.ok_or_else(|| anyhow!("header lacks parentHash"))?;
				let authority_state_hash = H256::from_str(parent)
					.map_err(|error| anyhow!("bad parent hash {parent}: {error}"))?;
				let authorities = decode_aura_authorities(
					&self
						.storage_raw_at(authority_state_hash, "Aura", "Authorities", vec![])
						.await?,
				)?;
				if authorities.is_empty() {
					bail!("{} finalized Aura authority set is empty", self.name)
				}
				let authority_index = (slot % authorities.len() as u64) as usize;
				let authority = authorities[authority_index].clone();
				if authority == expected_key {
					return Ok((
						blocks,
						AuraAuthorProof {
							block_hash: hash,
							authority_state_hash,
							slot,
							authority_index,
							authority,
							digest_log,
						},
					));
				}
			}
			tokio::time::sleep(Duration::from_secs(2)).await;
		}
		bail!("{} rotated Aura key did not author a finalized block within {timeout:?}", self.name)
	}
}

fn envelopes(chain: &str, hash: H256, events: Vec<EventEnvelope>) -> Vec<EventRecord> {
	events
		.into_iter()
		.map(|event| EventRecord {
			chain: chain.into(),
			block_hash: format!("{hash:#x}"),
			pallet: event.pallet,
			variant: event.variant,
			fields: format!("{:?}", event.fields),
		})
		.collect()
}

fn hash_bytes(bytes: &[u8]) -> String {
	format!("0x{}", hex::encode(sp_crypto_hashing::sha2_256(bytes)))
}

fn account(uri: &str) -> Result<(AccountId32, OriginSigner)> {
	let account = OriginAccount::from_uri(uri, None).map_err(|error| anyhow!("{error:?}"))?;
	let id = account.account_id();
	let signer = OriginSigner::from_account(&account).map_err(|error| anyhow!(error))?;
	Ok((id, signer))
}

fn account_bytes(account: &AccountId32) -> &[u8] {
	<AccountId32 as AsRef<[u8]>>::as_ref(account)
}

fn account_value(account: &AccountId32) -> Value<()> {
	Value::from_bytes(account_bytes(account))
}

fn multi_address(account: &AccountId32) -> Value<()> {
	Value::variant("Id", Composite::unnamed(vec![account_value(account)]))
}

fn has_event(events: &[EventRecord], pallet: &str, variant: &str) -> bool {
	events.iter().any(|event| event.pallet == pallet && event.variant == variant)
}

fn flatten_blocks(
	traces: &[TxTrace],
	extra: &[ObservedBlock],
) -> (Vec<FinalizedBlock>, Vec<EventRecord>) {
	let mut blocks = Vec::new();
	let mut events = Vec::new();
	for block in traces.iter().map(|trace| &trace.block).chain(extra) {
		blocks.push(FinalizedBlock {
			chain: block.chain.clone(),
			number: block.number,
			hash: format!("{:#x}", block.hash),
		});
		events.extend(block.events.clone());
	}
	blocks.sort_by(|a, b| (&a.chain, a.number, &a.hash).cmp(&(&b.chain, b.number, &b.hash)));
	blocks.dedup_by(|a, b| a.chain == b.chain && a.hash == b.hash);
	(blocks, events)
}

fn pass_record(
	input: BTreeMap<String, String>,
	output: BTreeMap<String, String>,
	traces: Vec<TxTrace>,
	extra: Vec<ObservedBlock>,
	assertions: Vec<AssertionRecord>,
) -> Result<CaseRecord> {
	if assertions.iter().any(|item| !item.passed) {
		bail!("case contains failed assertion")
	}
	let (finalized_blocks, events) = flatten_blocks(&traces, &extra);
	let record = CaseRecord {
		status: CaseStatus::Pass,
		input_hashes: input,
		output_hashes: output,
		finalized_blocks,
		events,
		assertions,
		failure: None,
	};
	record.validate_pass().map_err(|error| anyhow!(error))?;
	Ok(record)
}

fn failed_record(stage: &str, error: &anyhow::Error) -> CaseRecord {
	CaseRecord {
		status: CaseStatus::Failed,
		input_hashes: BTreeMap::new(),
		output_hashes: BTreeMap::new(),
		finalized_blocks: Vec::new(),
		events: Vec::new(),
		assertions: vec![AssertionRecord {
			name: stage.into(),
			passed: false,
			observed: format!("{error:#}"),
		}],
		failure: Some(FailureRecord {
			kind: "execution-failure".into(),
			stage: stage.into(),
			message: format!("{error:#}"),
			missing_runtime_surface: None,
		}),
	}
}

fn map_hash(name: &str, bytes: &[u8]) -> BTreeMap<String, String> {
	BTreeMap::from([(name.into(), hash_bytes(bytes))])
}

fn provider_assignment(para_id: u32) -> Value<()> {
	Value::unnamed_composite(vec![Value::unnamed_composite(vec![
		Value::variant("Task", Composite::unnamed(vec![Value::u128(para_id.into())])),
		Value::unnamed_composite(vec![Value::u128(57_600)]),
	])])
}

fn broker_schedule(para_id: u32) -> Value<()> {
	Value::unnamed_composite(vec![Value::named_composite([
		("mask", Value::from_bytes([0xff; 10])),
		(
			"assignment",
			Value::variant("Task", Composite::unnamed(vec![Value::u128(para_id.into())])),
		),
	])])
}

fn option_none() -> Value<()> {
	Value::variant("None", Composite::unnamed(vec![]))
}

fn option_some(value: Value<()>) -> Value<()> {
	Value::variant("Some", Composite::unnamed(vec![value]))
}

fn versioned_parent() -> Value<()> {
	Value::variant(
		"V5",
		Composite::unnamed(vec![Value::named_composite([
			("parents", Value::u128(1)),
			("interior", Value::variant("Here", Composite::unnamed(vec![]))),
		])]),
	)
}

fn versioned_orbis() -> Value<()> {
	Value::variant(
		"V5",
		Composite::unnamed(vec![Value::named_composite([
			("parents", Value::u128(0)),
			(
				"interior",
				Value::variant(
					"X1",
					Composite::unnamed(vec![Value::unnamed_composite(vec![Value::variant(
						"Parachain",
						Composite::unnamed(vec![Value::u128(1006)]),
					)])]),
				),
			),
		])]),
	)
}

fn versioned_transact(call: Vec<u8>, origin_kind: &str) -> Value<()> {
	let unpaid = Value::variant(
		"UnpaidExecution",
		Composite::named([
			("weight_limit", Value::variant("Unlimited", Composite::unnamed(vec![]))),
			("check_origin", option_none()),
		]),
	);
	let transact = Value::variant(
		"Transact",
		Composite::named([
			("origin_kind", Value::variant(origin_kind, Composite::unnamed(vec![]))),
			(
				"fallback_max_weight",
				option_some(Value::named_composite([
					("ref_time", Value::u128(250_000_000)),
					("proof_size", Value::u128(24_576)),
				])),
			),
			("call", Value::named_composite([("encoded", Value::from_bytes(call))])),
		]),
	);
	Value::variant("V5", Composite::unnamed(vec![Value::unnamed_composite(vec![unpaid, transact])]))
}

async fn run_control(
	args: &Args,
	relay: &mut Chain,
	orbis: &mut Chain,
) -> Result<BTreeMap<String, CaseRecord>> {
	let mut records = BTreeMap::new();
	let (alice_id, alice) = account("//Alice")?;
	let (bob_id, bob) = account("//Bob")?;
	let (ferdie_id, _) = account("//Ferdie")?;
	let timeout = Duration::from_secs(args.wait_seconds);

	// Validator removal.
	let before = relay.storage_raw("Session", "Validators", vec![]).await?;
	let tx = relay
		.sudo(&alice, dynamic::tx("AuthorityManager", "remove", vec![account_value(&ferdie_id)]))
		.await?;
	let session = relay.wait_session(tx.block.hash, timeout).await?;
	let after = relay.storage_raw("Session", "Validators", vec![]).await?;
	let assertions = vec![
		AssertionRecord {
			name: "queued removal finalized".into(),
			passed: has_event(&tx.block.events, "AuthorityManager", "QueuedRemoval"),
			observed: format!("{:?}", tx.block.events),
		},
		AssertionRecord {
			name: "session enacted".into(),
			passed: session
				.iter()
				.flat_map(|b| &b.events)
				.any(|e| e.pallet == "AuthorityManager" && e.variant == "Enacted"),
			observed: "AuthorityManager.Enacted".into(),
		},
		AssertionRecord {
			name: "Ferdie excluded".into(),
			passed: !after.1.windows(32).any(|value| value == account_bytes(&ferdie_id)),
			observed: hex::encode(&after.1),
		},
	];
	records.insert(
		"AC7-VALIDATOR-REMOVE".into(),
		pass_record(
			map_hash("validators-before", &before.1),
			map_hash("validators-after", &after.1),
			vec![tx],
			session,
			assertions,
		)?,
	);

	// Validator re-admission uses the still-staged session keys.
	let tx = relay
		.sudo(&alice, dynamic::tx("AuthorityManager", "nominate", vec![account_value(&ferdie_id)]))
		.await?;
	let session = relay.wait_session(tx.block.hash, timeout).await?;
	let after_admit = relay.storage_raw("Session", "Validators", vec![]).await?;
	records.insert(
		"AC7-VALIDATOR-ADMIT".into(),
		pass_record(
			map_hash("validators-removed", &after.1),
			map_hash("validators-restored", &after_admit.1),
			vec![tx.clone()],
			session.clone(),
			vec![
				AssertionRecord {
					name: "queued admission finalized".into(),
					passed: has_event(&tx.block.events, "AuthorityManager", "QueuedAdd"),
					observed: format!("{:?}", tx.block.events),
				},
				AssertionRecord {
					name: "Ferdie restored".into(),
					passed: after_admit
						.1
						.windows(32)
						.any(|value| value == account_bytes(&ferdie_id)),
					observed: hex::encode(&after_admit.1),
				},
			],
		)?,
	);

	// Remove Bob, rotate the key in Bob's local keystore, bind it with Session.set_keys, then
	// restore him.
	let old_aura = orbis.storage_raw("Aura", "Authorities", vec![]).await?;
	let remove = orbis
		.sudo(
			&alice,
			dynamic::tx(
				"CollatorSelection",
				"set_invulnerables",
				vec![Value::unnamed_composite(vec![account_value(&alice_id)])],
			),
		)
		.await?;
	let removed_session = orbis.wait_session(remove.block.hash, timeout).await?;
	let removed = orbis.storage_raw("Session", "Validators", vec![]).await?;
	records.insert(
		"AC7-COLLATOR-REMOVE".into(),
		pass_record(
			map_hash("invulnerables-before", account_bytes(&bob_id)),
			map_hash("validators-after", &removed.1),
			vec![remove.clone()],
			removed_session.clone(),
			vec![
				AssertionRecord {
					name: "NewInvulnerables finalized".into(),
					passed: has_event(
						&remove.block.events,
						"CollatorSelection",
						"NewInvulnerables",
					),
					observed: format!("{:?}", remove.block.events),
				},
				AssertionRecord {
					name: "Bob excluded".into(),
					passed: !removed.1.windows(32).any(|value| value == account_bytes(&bob_id)),
					observed: hex::encode(&removed.1),
				},
			],
		)?,
	);

	let bob_rpc = RpcClient::from_insecure_url("ws://127.0.0.1:11872").await?;
	let rotated: String = bob_rpc.request("author_rotateKeys", rpc_params![]).await?;
	let keys = hex::decode(rotated.trim_start_matches("0x"))?;
	let proof = Vec::<u8>::new();
	if keys.len() != 32 {
		bail!("Orbis SessionKeys must encode exactly one 32-byte Aura key, got {}", keys.len())
	}
	let set_keys = orbis
		.submit(
			&bob,
			dynamic::tx(
				"Session",
				"set_keys",
				vec![
					Value::named_composite([("aura", Value::from_bytes(&keys))]),
					Value::from_bytes(&proof),
				],
			),
		)
		.await?;
	let restore = orbis
		.sudo(
			&alice,
			dynamic::tx(
				"CollatorSelection",
				"set_invulnerables",
				vec![Value::unnamed_composite(vec![
					account_value(&alice_id),
					account_value(&bob_id),
				])],
			),
		)
		.await?;
	let restored_session = orbis.wait_session(restore.block.hash, timeout).await?;
	let validators = orbis.storage_raw("Session", "Validators", vec![]).await?;
	let aura = orbis.storage_raw("Aura", "Authorities", vec![]).await?;
	let author_scan_start = orbis.finalized_hash().await?;
	let (author_blocks, author_proof) = orbis
		.wait_aura_author(author_scan_start, &keys, Duration::from_secs(300))
		.await?;
	let author_binding = format!(
		"block={:#x};authority_state={:#x};slot={};index={};authority=0x{};digest={}",
		author_proof.block_hash,
		author_proof.authority_state_hash,
		author_proof.slot,
		author_proof.authority_index,
		hex::encode(&author_proof.authority),
		author_proof.digest_log,
	);
	records.insert(
		"AC7-COLLATOR-ADMIT".into(),
		pass_record(
			map_hash("validators-removed", &removed.1),
			map_hash("validators-restored", &validators.1),
			vec![restore.clone()],
			restored_session.clone().into_iter().chain(author_blocks.clone()).collect(),
			vec![
				AssertionRecord {
					name: "NewInvulnerables finalized".into(),
					passed: has_event(
						&restore.block.events,
						"CollatorSelection",
						"NewInvulnerables",
					),
					observed: format!("{:?}", restore.block.events),
				},
				AssertionRecord {
					name: "Bob restored".into(),
					passed: validators.1.windows(32).any(|value| value == account_bytes(&bob_id)),
					observed: hex::encode(&validators.1),
				},
				AssertionRecord {
					name: "restored rotated Aura authority authored a finalized block".into(),
					passed: author_proof.authority == keys,
					observed: author_binding.clone(),
				},
			],
		)?,
	);
	let rotation_input = BTreeMap::from([
		("old-aura-authorities".into(), hash_bytes(&old_aura.1)),
		("rotated-key".into(), hash_bytes(&keys)),
	]);
	let rotation_output = BTreeMap::from([
		("active-aura".into(), hash_bytes(&aura.1)),
		("finalized-author-binding".into(), hash_bytes(author_binding.as_bytes())),
	]);
	let rotation_assertions = vec![
		AssertionRecord {
			name: "rotated key differs from the old active set".into(),
			passed: !old_aura.1.windows(32).any(|value| value == keys.as_slice()),
			observed: hex::encode(&old_aura.1),
		},
		AssertionRecord {
			name: "rotated key is active and the authority set changed".into(),
			passed: old_aura.1 != aura.1
				&& aura.1.windows(32).any(|value| value == keys.as_slice()),
			observed: hex::encode(&aura.1),
		},
		AssertionRecord {
			name: "Aura PreRuntime slot maps to rotated authority index".into(),
			passed: author_proof.authority == keys,
			observed: author_binding.clone(),
		},
	];
	records.insert(
		"AC7-KEY-ROTATION".into(),
		pass_record(
			rotation_input.clone(),
			rotation_output.clone(),
			vec![set_keys.clone(), restore.clone()],
			restored_session.clone().into_iter().chain(author_blocks.clone()).collect(),
			rotation_assertions.clone(),
		)?,
	);
	records.insert(
		"AC7-COMPROMISE-RECOVERY".into(),
		pass_record(
			rotation_input,
			rotation_output,
			vec![remove, set_keys, restore],
			removed_session
				.into_iter()
				.chain(restored_session)
				.chain(author_blocks)
				.collect(),
			rotation_assertions,
		)?,
	);

	// Hash-pinned, increasing runtime upgrades. Reconnect after each metadata change.
	for (id, chain, path) in [
		("AC7-ORIGIN-UPGRADE", &mut *relay, &args.relay_upgrade_wasm),
		("AC7-ORBIS-UPGRADE", &mut *orbis, &args.orbis_upgrade_wasm),
	] {
		let wasm = fs::read(path)?;
		let before_hash = chain.finalized_hash().await?;
		let before_version: serde_json::Value = chain
			.rpc
			.request("state_getRuntimeVersion", rpc_params![format!("{before_hash:#x}")])
			.await?;
		let before_code: String = chain
			.rpc
			.request("state_getStorage", rpc_params!["0x3a636f6465", format!("{before_hash:#x}")])
			.await?;
		if hash_bytes(&wasm) == hash_bytes(&hex::decode(before_code.trim_start_matches("0x"))?) {
			bail!("{id} candidate Wasm equals live :code")
		}
		let tx = chain
			.sudo(&alice, dynamic::tx("System", "set_code", vec![Value::from_bytes(&wasm)]))
			.await?;
		chain.reconnect().await?;
		chain.assert_runtime_contract()?;
		let after_hash = chain.finalized_hash().await?;
		let after_version: serde_json::Value = chain
			.rpc
			.request("state_getRuntimeVersion", rpc_params![format!("{after_hash:#x}")])
			.await?;
		let after_code: String = chain
			.rpc
			.request("state_getStorage", rpc_params!["0x3a636f6465", format!("{after_hash:#x}")])
			.await?;
		let candidate_hash = hash_bytes(&wasm);
		let installed_hash = hash_bytes(&hex::decode(after_code.trim_start_matches("0x"))?);
		let increased =
			after_version["specVersion"].as_u64() > before_version["specVersion"].as_u64();
		let mut extra = Vec::new();
		let mut assertions = vec![
			AssertionRecord {
				name: "System.CodeUpdated".into(),
				passed: has_event(&tx.block.events, "System", "CodeUpdated"),
				observed: format!("{:?}", tx.block.events),
			},
			AssertionRecord {
				name: "finalized :code equals candidate".into(),
				passed: installed_hash == candidate_hash,
				observed: format!("candidate={candidate_hash}, installed={installed_hash}"),
			},
			AssertionRecord {
				name: "specVersion increased".into(),
				passed: increased,
				observed: format!(
					"{before_hash:#x}:{before_version} -> {after_hash:#x}:{after_version}"
				),
			},
		];
		if id == "AC7-ORBIS-UPGRADE" {
			// Production is six hours. A finalized rotation within this bound proves that the
			// candidate and live Orbis node are the separately hash-pinned fast-runtime build.
			let started = Instant::now();
			extra = chain.wait_session(tx.block.hash, Duration::from_secs(300)).await?;
			let elapsed = started.elapsed();
			assertions.push(AssertionRecord {
				name: "fast-runtime session finalized within five minutes".into(),
				passed: elapsed <= Duration::from_secs(300),
				observed: format!("elapsed={elapsed:?}"),
			});
		}
		records.insert(
			id.into(),
			pass_record(
				map_hash("live-code", before_code.as_bytes()),
				map_hash("candidate-wasm", &wasm),
				vec![tx.clone()],
				extra,
				assertions,
			)?,
		);
	}

	// TxPause and SafeMode use an ordinary Balances transfer as the blocked probe.
	let call_name = Value::unnamed_composite(vec![
		Value::from_bytes(b"Balances"),
		Value::from_bytes(b"transfer_allow_death"),
	]);
	let transfer = || {
		dynamic::tx(
			"Balances",
			"transfer_allow_death",
			vec![multi_address(&bob_id), Value::u128(1)],
		)
	};
	let balance_key = vec![account_value(&bob_id)];
	let paused = orbis
		.sudo(&alice, dynamic::tx("TxPause", "pause", vec![call_name.clone()]))
		.await?;
	let paused_before = orbis.storage_raw("System", "Account", balance_key.clone()).await?;
	let rejected = orbis.submit(&alice, transfer()).await;
	let paused_after = orbis.storage_raw("System", "Account", balance_key.clone()).await?;
	let unpaused = orbis.sudo(&alice, dynamic::tx("TxPause", "unpause", vec![call_name])).await?;
	let recovered = orbis.submit(&alice, transfer()).await?;
	records.insert(
		"AC7-TX-PAUSE-RECOVERY".into(),
		pass_record(
			map_hash("account-before", &paused_before.1),
			map_hash(
				"account-after",
				&orbis.storage_raw("System", "Account", balance_key.clone()).await?.1,
			),
			vec![paused, unpaused, recovered],
			vec![],
			vec![
				AssertionRecord {
					name: "paused call rejected".into(),
					passed: rejected.is_err(),
					observed: format!("{rejected:?}"),
				},
				AssertionRecord {
					name: "rejection preserved state".into(),
					passed: paused_before.1 == paused_after.1,
					observed: format!(
						"{} == {}",
						hex::encode(paused_before.1),
						hex::encode(paused_after.1)
					),
				},
			],
		)?,
	);

	let entered = orbis
		.sudo(&alice, dynamic::tx("SafeMode", "force_enter", Vec::<Value>::new()))
		.await?;
	let safe_before = orbis.storage_raw("System", "Account", balance_key.clone()).await?;
	let rejected = orbis.submit(&alice, transfer()).await;
	let safe_after = orbis.storage_raw("System", "Account", balance_key.clone()).await?;
	let exited = orbis
		.sudo(&alice, dynamic::tx("SafeMode", "force_exit", Vec::<Value>::new()))
		.await?;
	let recovered = orbis.submit(&alice, transfer()).await?;
	records.insert(
		"AC7-SAFE-MODE-RECOVERY".into(),
		pass_record(
			map_hash("account-before", &safe_before.1),
			map_hash(
				"account-after",
				&orbis.storage_raw("System", "Account", balance_key).await?.1,
			),
			vec![entered, exited, recovered],
			vec![],
			vec![
				AssertionRecord {
					name: "safe-mode call rejected".into(),
					passed: rejected.is_err(),
					observed: format!("{rejected:?}"),
				},
				AssertionRecord {
					name: "rejection preserved state".into(),
					passed: safe_before.1 == safe_after.1,
					observed: format!(
						"{} == {}",
						hex::encode(safe_before.1),
						hex::encode(safe_after.1)
					),
				},
			],
		)?,
	);
	Ok(records)
}

async fn request_and_receipt(
	orbis: &Chain,
	alice: &OriginSigner,
	count: u16,
	timeout: Duration,
) -> Result<(u64, TxTrace, Vec<ObservedBlock>)> {
	let before = orbis.storage_raw("CoretimeControl", "NextRequestId", vec![]).await?;
	let id = if before.1.is_empty() { 0 } else { u64::decode(&mut &before.1[..])? };
	let tx = orbis
		.sudo(
			alice,
			dynamic::tx("CoretimeControl", "request_core_count", vec![Value::u128(count.into())]),
		)
		.await?;
	let receipt = orbis
		.wait_event(tx.block.hash, "CoretimeControl", "ReceiptRecorded", timeout)
		.await?;
	Ok((id, tx, receipt))
}

async fn run_broker_pre(
	args: &Args,
	relay: &Chain,
	orbis: &Chain,
) -> Result<BTreeMap<String, CaseRecord>> {
	let mut records = BTreeMap::new();
	let (_, alice) = account("//Alice")?;
	let timeout = Duration::from_secs(args.wait_seconds);
	let relay_head = relay.finalized_hash().await?;
	let begin = relay.header_number(relay_head).await?.saturating_add(2);
	let bootstrap = relay
		.sudo(
			&alice,
			dynamic::tx(
				"Coretime",
				"assign_core",
				vec![
					Value::u128(0),
					Value::u128(begin.into()),
					provider_assignment(1006),
					option_none(),
				],
			),
		)
		.await?;
	let queue_hash = relay.finalized_hash().await?;
	let queue_after = claim_queue_at(relay, queue_hash).await?;
	records.insert(
		"AC8-BOOTSTRAP".into(),
		pass_record(
			map_hash("assignment-input", &1006u32.to_le_bytes()),
			map_hash("claim-queue", queue_after.as_bytes()),
			vec![bootstrap],
			vec![relay.observed_block(queue_hash).await?],
			vec![AssertionRecord {
				name: "claim queue contains para 1006".into(),
				passed: queue_core_count(&queue_after, 1006) >= 1,
				observed: format!("hash={queue_hash:#x}, queue={queue_after}"),
			}],
		)?,
	);

	let (id, request, receipt) = request_and_receipt(orbis, &alice, 3, timeout).await?;
	let origin_applied = relay
		.storage_raw("CoretimeControl", "Applied", vec![Value::u128(id.into())])
		.await?;
	records.insert(
		"AC8-REQUEST".into(),
		pass_record(
			map_hash("request", &provider_submit_call(id, 3)),
			map_hash("origin-applied", &origin_applied.1),
			vec![request.clone()],
			receipt.clone(),
			vec![
				AssertionRecord {
					name: "RequestSent finalized".into(),
					passed: has_event(&request.block.events, "CoretimeControl", "RequestSent"),
					observed: format!("{:?}", request.block.events),
				},
				AssertionRecord {
					name: "receipt finalized".into(),
					passed: receipt
						.iter()
						.flat_map(|b| &b.events)
						.any(|e| e.pallet == "CoretimeControl" && e.variant == "ReceiptRecorded"),
					observed: format!("{receipt:?}"),
				},
				AssertionRecord {
					name: "provider applied count".into(),
					passed: origin_applied.1 == 3u16.to_le_bytes(),
					observed: hex::encode(&origin_applied.1),
				},
			],
		)?,
	);

	let mut reserves = Vec::new();
	for _ in 0..3 {
		reserves.push(
			orbis
				.sudo(&alice, dynamic::tx("Broker", "reserve", vec![broker_schedule(1006)]))
				.await?,
		);
	}
	let reservations = orbis.storage_raw("Broker", "Reservations", vec![]).await?;
	records.insert(
		"AC8-RESERVE".into(),
		pass_record(
			map_hash("schedule", &1006u32.to_le_bytes()),
			map_hash("reservations", &reservations.1),
			reserves.clone(),
			vec![],
			vec![AssertionRecord {
				name: "three complete reservation schedules finalized".into(),
				passed: reserves.len() == 3
					&& reservations.1.first() == Some(&12)
					&& !reservations.1.is_empty(),
				observed: format!(
					"dispatches={}, storage={}",
					reserves.len(),
					hex::encode(&reservations.1)
				),
			}],
		)?,
	);

	let config = Value::named_composite([
		("advance_notice", Value::u128(2)),
		("interlude_length", Value::u128(2)),
		("leadin_length", Value::u128(2)),
		("region_length", Value::u128(5)),
		("ideal_bulk_proportion", Value::u128(1_000_000_000)),
		("limit_cores_offered", option_some(Value::u128(0))),
		("renewal_bump", Value::u128(100_000_000)),
		("contribution_timeout", Value::u128(5)),
	]);
	let configure = orbis.sudo(&alice, dynamic::tx("Broker", "configure", vec![config])).await?;
	let start = orbis
		.sudo(&alice, dynamic::tx("Broker", "start_sales", vec![Value::u128(1), Value::u128(0)]))
		.await?;
	let assigned = match orbis.wait_event(start.block.hash, "Broker", "CoreAssigned", timeout).await
	{
		Ok(blocks) => blocks,
		Err(_) => {
			orbis
				.wait_event(start.block.hash, "Broker", "HistoryInitialized", timeout)
				.await?
		},
	};
	let (queue, queue_blocks) = wait_claim_count(relay, 1006, 3, true, timeout).await?;
	records.insert(
		"AC8-ASSIGN".into(),
		pass_record(
			map_hash("reservations", &reservations.1),
			map_hash("claim-queue", queue.as_bytes()),
			vec![configure, start],
			assigned.into_iter().chain(queue_blocks).collect(),
			vec![AssertionRecord {
				name: "three claim-queue cores".into(),
				passed: queue_core_count(&queue, 1006) >= 3,
				observed: queue,
			}],
		)?,
	);

	// A bounded lease is the enterprise-safe renewable path; it avoids a public offer.
	let status = orbis.storage_raw("Broker", "Status", vec![]).await?;
	let broker_status = BrokerStatus::decode(&mut &status.1[..]).context("decode Broker.Status")?;
	let lease_until = broker_status
		.last_timeslice
		.checked_add(5)
		.ok_or_else(|| anyhow!("lease timeslice overflow"))?;
	let lease = orbis
		.sudo(
			&alice,
			dynamic::tx(
				"Broker",
				"set_lease",
				vec![Value::u128(1006), Value::u128(lease_until.into())],
			),
		)
		.await;
	match lease {
		Ok(lease) => {
			let rotation =
				orbis.wait_event(lease.block.hash, "Broker", "Renewable", timeout).await?;
			let renew = orbis
				.submit(&alice, dynamic::tx("Broker", "renew", vec![Value::u128(3)]))
				.await?;
			records.insert(
				"AC8-RENEW".into(),
				pass_record(
					map_hash("broker-status", &status.1),
					map_hash("renew-tx", renew.tx_hash.as_bytes()),
					vec![lease, renew.clone()],
					rotation,
					vec![AssertionRecord {
						name: "renew finalized".into(),
						passed: has_event(&renew.block.events, "Broker", "Renewed"),
						observed: format!(
							"status={broker_status:?}, until={lease_until}, events={:?}",
							renew.block.events
						),
					}],
				)?,
			);
		},
		Err(error) => {
			records.insert("AC8-RENEW".into(), failed_record("enterprise lease renewal", &error));
		},
	}

	let session_before = relay.finalized_hash().await?;
	let (down_id, down, down_receipt) = request_and_receipt(orbis, &alice, 1, timeout).await?;
	let relay_session = relay.wait_session(session_before, timeout).await?;
	let (queue_down, queue_down_blocks) = wait_claim_count(relay, 1006, 1, false, timeout).await?;
	records.insert(
		"AC8-RESIZE-DOWN".into(),
		pass_record(
			map_hash("request", &provider_submit_call(down_id, 1)),
			map_hash("claim-queue", queue_down.as_bytes()),
			vec![down.clone()],
			down_receipt
				.clone()
				.into_iter()
				.chain(relay_session.clone())
				.chain(queue_down_blocks)
				.collect(),
			vec![AssertionRecord {
				name: "one active core".into(),
				passed: queue_core_count(&queue_down, 1006) == 1,
				observed: queue_down.clone(),
			}],
		)?,
	);
	let (up_id, up, up_receipt) = request_and_receipt(orbis, &alice, 3, timeout).await?;
	let relay_session_up = relay.wait_session(relay.finalized_hash().await?, timeout).await?;
	let (queue_up, queue_up_blocks) = wait_claim_count(relay, 1006, 3, true, timeout).await?;
	records.insert(
		"AC8-RESIZE-UP".into(),
		pass_record(
			map_hash("request", &provider_submit_call(up_id, 3)),
			map_hash("claim-queue", queue_up.as_bytes()),
			vec![up.clone()],
			up_receipt
				.clone()
				.into_iter()
				.chain(relay_session_up.clone())
				.chain(queue_up_blocks)
				.collect(),
			vec![AssertionRecord {
				name: "three active cores".into(),
				passed: queue_core_count(&queue_up, 1006) >= 3,
				observed: queue_up.clone(),
			}],
		)?,
	);
	records.insert(
		"AC8-SESSION".into(),
		pass_record(
			map_hash("pre-session", queue_down.as_bytes()),
			map_hash("post-session", queue_up.as_bytes()),
			vec![down, up],
			relay_session.into_iter().chain(relay_session_up).collect(),
			vec![AssertionRecord {
				name: "scheduling changes crossed relay sessions".into(),
				passed: queue_core_count(&queue_down, 1006) == 1
					&& queue_core_count(&queue_up, 1006) >= 3,
				observed: format!("{queue_down} -> {queue_up}"),
			}],
		)?,
	);

	let applied_before = relay
		.storage_raw("CoretimeControl", "Applied", vec![Value::u128(up_id.into())])
		.await?;
	let relay_before_retry = relay.finalized_hash().await?;
	let retry = orbis
		.sudo(
			&alice,
			dynamic::tx("CoretimeControl", "retry_request", vec![Value::u128(up_id.into())]),
		)
		.await?;
	let duplicate = orbis
		.wait_event(retry.block.hash, "CoretimeControl", "ReceiptRecorded", timeout)
		.await?;
	let provider_duplicate = relay
		.wait_event(relay_before_retry, "CoretimeControl", "RequestStatus", timeout)
		.await?;
	let applied = relay
		.storage_raw("CoretimeControl", "Applied", vec![Value::u128(up_id.into())])
		.await?;
	records.insert(
		"AC8-DUPLICATE".into(),
		pass_record(
			map_hash("request", &provider_submit_call(up_id, 3)),
			map_hash("applied", &applied.1),
			vec![retry],
			duplicate.clone().into_iter().chain(provider_duplicate.clone()).collect(),
			vec![
				AssertionRecord {
					name: "provider emitted Duplicate".into(),
					passed: provider_duplicate.iter().flat_map(|block| &block.events).any(
						|event| {
							event.pallet == "CoretimeControl"
								&& event.variant == "RequestStatus"
								&& event.fields.contains("Duplicate")
						},
					),
					observed: format!("{provider_duplicate:?}"),
				},
				AssertionRecord {
					name: "single provider ledger value".into(),
					passed: applied_before.1 == applied.1 && applied.1 == 3u16.to_le_bytes(),
					observed: format!(
						"{} -> {}",
						hex::encode(&applied_before.1),
						hex::encode(&applied.1)
					),
				},
			],
		)?,
	);

	let next = orbis.storage_raw("CoretimeControl", "NextRequestId", vec![]).await?;
	let next_id = u64::decode(&mut &next.1[..])?;
	let gap = next_id.saturating_add(1);
	let relay_before_gap = relay.finalized_hash().await?;
	let inject = orbis
		.sudo(
			&alice,
			dynamic::tx(
				"PolkadotXcm",
				"send",
				vec![
					versioned_parent(),
					versioned_transact(provider_submit_call(gap, 2), "Native"),
				],
			),
		)
		.await?;
	let out_of_order = relay
		.wait_event(relay_before_gap, "CoretimeControl", "RequestStatus", timeout)
		.await?;
	let (predecessor_id, predecessor, predecessor_receipt) =
		request_and_receipt(orbis, &alice, 2, timeout).await?;
	let (gap_id, gap_normal, gap_receipt) = request_and_receipt(orbis, &alice, 2, timeout).await?;
	let last = relay.storage_raw("CoretimeControl", "LastApplied", vec![]).await?;
	records.insert(
		"AC8-OUT-OF-ORDER".into(),
		pass_record(
			map_hash("gap", &provider_submit_call(gap, 2)),
			map_hash("last-applied", &last.1),
			vec![inject, predecessor, gap_normal],
			out_of_order
				.clone()
				.into_iter()
				.chain(predecessor_receipt)
				.chain(gap_receipt)
				.collect(),
			vec![
				AssertionRecord {
					name: "provider rejected injected future id".into(),
					passed: out_of_order.iter().flat_map(|block| &block.events).any(|event| {
						event.pallet == "CoretimeControl"
							&& event.variant == "RequestStatus"
							&& event.fields.contains("OutOfOrder")
					}),
					observed: format!("{out_of_order:?}"),
				},
				AssertionRecord {
					name: "gap recovered in sequence".into(),
					passed: predecessor_id == next_id
						&& gap_id == gap && last.1 == (gap_id, 2u16).encode(),
					observed: format!(
						"predecessor={predecessor_id}, gap={gap_id}, last={}",
						hex::encode(&last.1)
					),
				},
			],
		)?,
	);

	// A late negative receipt is injected from Origin with Parent/Superuser semantics. Capture
	// the Orbis finalized head and terminal record before sending so a fast DMP cannot race past
	// the evidence scan.
	let orbis_before_late = orbis.finalized_hash().await?;
	let outbound_before_late = orbis
		.storage_raw_at(
			orbis_before_late,
			"CoretimeControl",
			"Outbound",
			vec![Value::u128(gap_id.into())],
		)
		.await?;
	let late = relay
		.sudo(
			&alice,
			dynamic::tx(
				"PolkadotXcm",
				"send",
				vec![
					versioned_orbis(),
					versioned_transact(
						oc::p1_campaign::orbis_acknowledge_call(gap_id, 2, 2),
						"Superuser",
					),
				],
			),
		)
		.await?;
	let late_blocks = orbis
		.wait_event(orbis_before_late, "MessageQueue", "Processed", Duration::from_secs(60))
		.await?;
	let outbound = orbis
		.storage_raw("CoretimeControl", "Outbound", vec![Value::u128(gap_id.into())])
		.await?;
	records.insert(
		"AC8-RECEIPT-REORDER".into(),
		pass_record(
			map_hash("late-negative", &oc::p1_campaign::orbis_acknowledge_call(gap_id, 2, 2)),
			map_hash("outbound", &outbound.1),
			vec![late],
			late_blocks.clone(),
			vec![
				AssertionRecord {
					name: "late negative was processed but not recorded".into(),
					passed: !late_blocks.iter().flat_map(|block| &block.events).any(|event| {
						event.pallet == "CoretimeControl" && event.variant == "ReceiptRecorded"
					}),
					observed: format!("{:?}", late_blocks),
				},
				AssertionRecord {
					name: "terminal outbound retained".into(),
					passed: !outbound.1.is_empty() && outbound.1 == outbound_before_late,
					observed: format!(
						"{} -> {}",
						hex::encode(outbound_before_late),
						hex::encode(&outbound.1)
					),
				},
			],
		)?,
	);

	// The fast-runtime-only control gate holds exact outbound envelopes before XCM routing.
	// A later finalized relay block proves the provider still has no ledger entries. Explicit
	// releases then use the normal runtime-owned sender in retained request-ID order.
	let held_next = orbis.storage_raw("CoretimeControl", "NextRequestId", vec![]).await?;
	let held_id = u64::decode(&mut &held_next.1[..])?;
	let held_second_id = held_id.checked_add(1).ok_or_else(|| anyhow!("held ID overflow"))?;
	let relay_before_hold = relay.finalized_hash().await?;
	let hold = orbis
		.sudo(&alice, dynamic::tx("CoretimeControl", "set_transport_hold", vec![Value::bool(true)]))
		.await?;
	let held_request = orbis
		.sudo(&alice, dynamic::tx("CoretimeControl", "request_core_count", vec![Value::u128(2)]))
		.await?;
	let held_second = orbis
		.sudo(&alice, dynamic::tx("CoretimeControl", "request_core_count", vec![Value::u128(3)]))
		.await?;
	let marker = orbis
		.submit(
			&alice,
			dynamic::tx("System", "remark", vec![Value::from_bytes(b"p1-held-request")]),
		)
		.await?;
	let relay_delay_block =
		relay.wait_finalized_after(relay_before_hold, Duration::from_secs(60)).await?;
	let pending = orbis
		.storage_raw("CoretimeControl", "Outbound", vec![Value::u128(held_id.into())])
		.await?;
	let pending_second = orbis
		.storage_raw("CoretimeControl", "Outbound", vec![Value::u128(held_second_id.into())])
		.await?;
	let held_flag = orbis.storage_raw("CoretimeControl", "TransportHeld", vec![]).await?;
	let provider_before_release = relay
		.storage_raw_at(
			relay_delay_block.hash,
			"CoretimeControl",
			"Applied",
			vec![Value::u128(held_id.into())],
		)
		.await?;
	let provider_second_before_release = relay
		.storage_raw_at(
			relay_delay_block.hash,
			"CoretimeControl",
			"Applied",
			vec![Value::u128(held_second_id.into())],
		)
		.await?;
	let release_held = orbis
		.sudo(
			&alice,
			dynamic::tx("CoretimeControl", "release_held", vec![Value::u128(held_id.into())]),
		)
		.await?;
	let held_receipt = orbis
		.wait_event(release_held.block.hash, "CoretimeControl", "ReceiptRecorded", timeout)
		.await?;
	let provider_after_first = relay.storage_raw("CoretimeControl", "LastApplied", vec![]).await?;
	let release_second = orbis
		.sudo(
			&alice,
			dynamic::tx(
				"CoretimeControl",
				"release_held",
				vec![Value::u128(held_second_id.into())],
			),
		)
		.await?;
	let held_second_receipt = orbis
		.wait_event(release_second.block.hash, "CoretimeControl", "ReceiptRecorded", timeout)
		.await?;
	let provider_after_release =
		relay.storage_raw("CoretimeControl", "LastApplied", vec![]).await?;
	let unhold = orbis
		.sudo(
			&alice,
			dynamic::tx("CoretimeControl", "set_transport_hold", vec![Value::bool(false)]),
		)
		.await?;
	let held_receipt_hash = held_second_receipt
		.last()
		.map(|block| format!("{:#x}", block.hash))
		.ok_or_else(|| anyhow!("held request receipt scan returned no finalized block"))?;
	records.insert(
		"AC8-DELAYED".into(),
		pass_record(
			BTreeMap::from([
				("first-request-id".into(), hash_bytes(&held_id.to_le_bytes())),
				("second-request-id".into(), hash_bytes(&held_second_id.to_le_bytes())),
				("pending-outbound".into(), hash_bytes(&pending.1)),
				("pending-second-outbound".into(), hash_bytes(&pending_second.1)),
			]),
			BTreeMap::from([
				("provider-last-applied".into(), hash_bytes(&provider_after_release.1)),
				("receipt-block".into(), hash_bytes(held_receipt_hash.as_bytes())),
			]),
			vec![
				hold,
				held_request.clone(),
				held_second.clone(),
				marker,
				release_held.clone(),
				release_second.clone(),
				unhold,
			],
			std::iter::once(relay_delay_block)
				.chain(held_receipt.clone())
				.chain(held_second_receipt.clone())
				.collect(),
			vec![
				AssertionRecord {
					name: "requests finalized in held pending state".into(),
					passed: has_event(&held_request.block.events, "CoretimeControl", "RequestHeld")
						&& has_event(&held_second.block.events, "CoretimeControl", "RequestHeld")
						&& held_flag.1 == [1]
						&& pending.1 == [2, 0, 0]
						&& pending_second.1 == [3, 0, 0],
					observed: format!(
						"held={}, first={}, second={}, events={:?}/{:?}",
						hex::encode(&held_flag.1),
						hex::encode(&pending.1),
						hex::encode(&pending_second.1),
						held_request.block.events,
						held_second.block.events,
					),
				},
				AssertionRecord {
					name: "provider remained unapplied across later relay finality".into(),
					passed: provider_before_release.is_empty()
						&& provider_second_before_release.is_empty(),
					observed: format!(
						"first={}, second={}",
						hex::encode(&provider_before_release),
						hex::encode(&provider_second_before_release),
					),
				},
				AssertionRecord {
					name: "explicit releases preserved order and finalized Accepted receipts"
						.into(),
					passed: has_event(
						&release_held.block.events,
						"CoretimeControl",
						"HeldRequestReleased",
					) && has_event(
						&release_second.block.events,
						"CoretimeControl",
						"HeldRequestReleased",
					) && provider_after_first.1 == (held_id, 2u16).encode()
						&& provider_after_release.1 == (held_second_id, 3u16).encode()
						&& held_receipt.iter().flat_map(|block| &block.events).any(|event| {
							event.pallet == "CoretimeControl"
								&& event.variant == "ReceiptRecorded"
								&& event.fields.contains("Accepted")
						}) && held_second_receipt.iter().flat_map(|block| &block.events).any(
						|event| {
							event.pallet == "CoretimeControl"
								&& event.variant == "ReceiptRecorded"
								&& event.fields.contains("Accepted")
						},
					),
					observed: format!(
						"first={}, second={}, receipts={held_receipt:?}/{held_second_receipt:?}",
						hex::encode(&provider_after_first.1),
						hex::encode(&provider_after_release.1),
					),
				},
			],
		)?,
	);
	let reservations_before = orbis.storage_raw("Broker", "Reservations", vec![]).await?;
	let relay_before_release = relay.finalized_hash().await?;
	let release = orbis
		.sudo(&alice, dynamic::tx("Broker", "unreserve", vec![Value::u128(0)]))
		.await?;
	let (release_id, release_request, release_receipt) =
		request_and_receipt(orbis, &alice, 2, timeout).await?;
	let release_session = relay.wait_session(relay_before_release, timeout).await?;
	let (release_queue, release_queue_blocks) =
		wait_claim_count(relay, 1006, 2, false, timeout).await?;
	let reservations_after = orbis.storage_raw("Broker", "Reservations", vec![]).await?;
	records.insert(
		"AC8-RELEASE".into(),
		pass_record(
			map_hash("reservations-before", &reservations_before.1),
			map_hash("reservations-after", &reservations_after.1),
			vec![release, release_request],
			release_receipt
				.into_iter()
				.chain(release_session)
				.chain(release_queue_blocks)
				.collect(),
			vec![
				AssertionRecord {
					name: "reservation state changed".into(),
					passed: reservations_before.1 != reservations_after.1,
					observed: format!("request={release_id}"),
				},
				AssertionRecord {
					name: "released core absent after provider session".into(),
					passed: queue_core_count(&release_queue, 1006) == 2,
					observed: release_queue,
				},
			],
		)?,
	);

	let checkpoint_applied = relay
		.storage_raw("CoretimeControl", "Applied", vec![Value::u128(gap_id.into())])
		.await?;
	let checkpoint = json!({
		"retained_request_id": gap_id,
		"retained_count": 2,
		"relay_applied": hex::encode(checkpoint_applied.1),
		"orbis_outbound": hex::encode(outbound.1),
	});
	fs::write(
		args.evidence_dir.join("p1-driver-restart-checkpoint.json"),
		serde_json::to_vec_pretty(&checkpoint)?,
	)?;
	Ok(records)
}

async fn run_broker_post(
	args: &Args,
	relay: &Chain,
	orbis: &Chain,
) -> Result<BTreeMap<String, CaseRecord>> {
	let mut records = BTreeMap::new();
	let (_, alice) = account("//Alice")?;
	let checkpoint_path = args.evidence_dir.join("p1-driver-restart-checkpoint.json");
	let checkpoint: serde_json::Value = serde_json::from_slice(
		&fs::read(&checkpoint_path).context("missing pre-restart checkpoint")?,
	)?;
	let id = checkpoint["retained_request_id"]
		.as_u64()
		.ok_or_else(|| anyhow!("checkpoint lacks request id"))?;
	let relay_head = relay.finalized_hash().await?;
	let orbis_head = orbis.finalized_hash().await?;
	let applied = relay
		.storage_raw_at(relay_head, "CoretimeControl", "Applied", vec![Value::u128(id.into())])
		.await?;
	let outbound = orbis
		.storage_raw_at(orbis_head, "CoretimeControl", "Outbound", vec![Value::u128(id.into())])
		.await?;
	let restart_evidence = fs::read(args.evidence_dir.join("full-restart.json"))
		.context("runner full-restart evidence missing")?;
	let checkpoint_applied = checkpoint["relay_applied"]
		.as_str()
		.ok_or_else(|| anyhow!("checkpoint lacks relay applied value"))?;
	let checkpoint_outbound = checkpoint["orbis_outbound"]
		.as_str()
		.ok_or_else(|| anyhow!("checkpoint lacks Orbis outbound value"))?;
	records.insert(
		"AC8-FULL-RESTART".into(),
		pass_record(
			map_hash("checkpoint", &fs::read(&checkpoint_path)?),
			map_hash("runner-restart", &restart_evidence),
			vec![],
			vec![relay.observed_block(relay_head).await?, orbis.observed_block(orbis_head).await?],
			vec![
				AssertionRecord {
					name: "runner restart verdict pass".into(),
					passed: String::from_utf8_lossy(&restart_evidence)
						.contains("\"status\": \"pass\""),
					observed: String::from_utf8_lossy(&restart_evidence).into(),
				},
				AssertionRecord {
					name: "provider ledger persisted across restart".into(),
					passed: hex::encode(&applied) == checkpoint_applied,
					observed: format!("{checkpoint_applied} -> {}", hex::encode(&applied)),
				},
				AssertionRecord {
					name: "Orbis retained ledger persisted across restart".into(),
					passed: hex::encode(&outbound) == checkpoint_outbound,
					observed: format!("{checkpoint_outbound} -> {}", hex::encode(&outbound)),
				},
			],
		)?,
	);
	let retry = orbis
		.sudo(&alice, dynamic::tx("CoretimeControl", "retry_request", vec![Value::u128(id.into())]))
		.await?;
	let receipt = orbis
		.wait_event(
			retry.block.hash,
			"CoretimeControl",
			"ReceiptRecorded",
			Duration::from_secs(args.wait_seconds),
		)
		.await?;
	let applied_after = relay
		.storage_raw("CoretimeControl", "Applied", vec![Value::u128(id.into())])
		.await?;
	let outbound_after = orbis
		.storage_raw("CoretimeControl", "Outbound", vec![Value::u128(id.into())])
		.await?;
	records.insert(
		"AC8-RESTART-RECOVERY".into(),
		pass_record(
			map_hash("provider-before", &applied),
			map_hash("provider-after", &applied_after.1),
			vec![retry],
			receipt,
			vec![
				AssertionRecord {
					name: "provider ledger persisted".into(),
					passed: applied == applied_after.1,
					observed: hex::encode(&applied_after.1),
				},
				AssertionRecord {
					name: "Orbis outbound persisted".into(),
					passed: outbound == outbound_after.1,
					observed: hex::encode(&outbound_after.1),
				},
			],
		)?,
	);
	Ok(records)
}

async fn claim_queue_at(chain: &Chain, hash: H256) -> Result<String> {
	chain
		.rpc
		.request("state_call", rpc_params!["ParachainHost_claim_queue", "0x", format!("{hash:#x}")])
		.await
		.map_err(Into::into)
}

async fn wait_claim_count(
	chain: &Chain,
	para: u32,
	expected: usize,
	at_least: bool,
	timeout: Duration,
) -> Result<(String, Vec<ObservedBlock>)> {
	let started = Instant::now();
	let mut last_number = 0;
	let mut blocks = Vec::new();
	loop {
		let head = chain.finalized_hash().await?;
		let number = chain.header_number(head).await?;
		if number != last_number {
			blocks.push(chain.observed_block(head).await?);
			last_number = number;
		}
		let queue = claim_queue_at(chain, head).await?;
		let count = queue_core_count(&queue, para);
		if (at_least && count >= expected) || (!at_least && count == expected) {
			return Ok((queue, blocks));
		}
		if started.elapsed() >= timeout {
			bail!(
				"{} claim queue for para {para} did not reach {}{} within {timeout:?}; last={queue}",
				chain.name,
				if at_least { ">=" } else { "=" },
				expected,
			)
		}
		tokio::time::sleep(Duration::from_secs(2)).await;
	}
}

fn decode_compact(data: &[u8], offset: &mut usize) -> Result<u64> {
	let first = *data.get(*offset).ok_or_else(|| anyhow!("compact EOF"))?;
	let mode = first & 3;
	match mode {
		0 => {
			*offset += 1;
			Ok((first >> 2) as u64)
		},
		1 => {
			let bytes = data.get(*offset..*offset + 2).ok_or_else(|| anyhow!("compact2 EOF"))?;
			*offset += 2;
			Ok((u16::from_le_bytes([bytes[0], bytes[1]]) >> 2) as u64)
		},
		2 => {
			let bytes = data.get(*offset..*offset + 4).ok_or_else(|| anyhow!("compact4 EOF"))?;
			*offset += 4;
			Ok((u32::from_le_bytes(bytes.try_into().unwrap()) >> 2) as u64)
		},
		_ => {
			let len = ((first >> 2) + 4) as usize;
			*offset += 1;
			let bytes =
				data.get(*offset..*offset + len).ok_or_else(|| anyhow!("compact big EOF"))?;
			*offset += len;
			Ok(bytes.iter().enumerate().fold(0u64, |v, (i, b)| v | ((*b as u64) << (8 * i))))
		},
	}
}

fn aura_slot_from_header(header: &serde_json::Value) -> Result<Option<(u64, String)>> {
	let logs = header["digest"]["logs"]
		.as_array()
		.ok_or_else(|| anyhow!("header lacks digest.logs"))?;
	for value in logs {
		let encoded = value.as_str().ok_or_else(|| anyhow!("digest log is not hex"))?;
		let data = hex::decode(encoded.trim_start_matches("0x"))?;
		// SCALE DigestItem::PreRuntime is variant 6, followed by the raw four-byte engine ID,
		// then the compact-length-prefixed consensus payload. Aura's pre-digest payload is Slot.
		if data.len() < 6 || data[0] != 6 || &data[1..5] != b"aura" {
			continue;
		}
		let mut offset = 5;
		let len = decode_compact(&data, &mut offset)? as usize;
		if len != 8 || offset + len != data.len() {
			bail!("malformed Aura PreRuntime digest: {encoded}")
		}
		let slot = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
		return Ok(Some((slot, encoded.into())));
	}
	Ok(None)
}

fn decode_aura_authorities(data: &[u8]) -> Result<Vec<Vec<u8>>> {
	let mut offset = 0;
	let count = decode_compact(data, &mut offset)? as usize;
	let expected = offset
		.checked_add(count.checked_mul(32).ok_or_else(|| anyhow!("Aura count overflow"))?)
		.ok_or_else(|| anyhow!("Aura storage length overflow"))?;
	if expected != data.len() {
		bail!("malformed Aura.Authorities storage: expected {expected}, got {}", data.len())
	}
	Ok((0..count)
		.map(|index| data[offset + index * 32..offset + (index + 1) * 32].to_vec())
		.collect())
}

fn queue_core_count(encoded: &str, para: u32) -> usize {
	let Ok(data) = hex::decode(encoded.trim_start_matches("0x")) else { return 0 };
	let mut offset = 0;
	let Ok(entries) = decode_compact(&data, &mut offset) else { return 0 };
	let mut count = 0;
	for _ in 0..entries {
		if offset + 4 > data.len() {
			return 0;
		}
		offset += 4;
		let Ok(items) = decode_compact(&data, &mut offset) else { return 0 };
		let mut found = false;
		for _ in 0..items {
			if offset + 4 > data.len() {
				return 0;
			}
			let id = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
			offset += 4;
			if id == para {
				found = true
			}
		}
		if found {
			count += 1
		}
	}
	if offset != data.len() {
		0
	} else {
		count
	}
}

fn validate_manifest(manifest: &ScenarioManifest, phase: &str) -> Result<()> {
	if manifest.schema != "cord.p1-control-broker-scenarios.v1"
		|| manifest.campaign_id != "origin-orbis-p1-control-broker-v1"
		|| manifest.para_id != 1006
	{
		bail!("unexpected scenario manifest identity")
	}
	let expected = expected_cases(phase)
		.ok_or_else(|| anyhow!("unknown phase {phase}"))?
		.iter()
		.copied()
		.collect::<BTreeSet<_>>();
	let phase_cases = manifest
		.phases
		.get(phase)
		.ok_or_else(|| anyhow!("manifest lacks phase {phase}"))?;
	if phase_cases.len() != expected.len()
		|| phase_cases
			.iter()
			.any(|case| case.action.is_empty() || case.requires.is_empty())
	{
		bail!("manifest contains duplicate or incomplete phase cases")
	}
	let observed = phase_cases.iter().map(|case| case.id.as_str()).collect::<BTreeSet<_>>();
	if expected != observed {
		bail!("manifest case mismatch: {observed:?} != {expected:?}")
	}
	Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
	let args = Args::parse();
	let manifest: ScenarioManifest = serde_json::from_slice(&fs::read(&args.manifest)?)?;
	validate_manifest(&manifest, &args.phase)?;
	let mut evidence = DriverEvidence {
		schema: "cord.p1-live-driver-evidence.v1",
		campaign_id: manifest.campaign_id.clone(),
		phase: args.phase.clone(),
		status: CaseStatus::Failed,
		cases: BTreeMap::new(),
	};
	let result: Result<BTreeMap<String, CaseRecord>> = async {
		let mut relay = Chain::connect("relay", args.relay.clone()).await?;
		let mut orbis = Chain::connect("orbis", args.orbis.clone()).await?;
		relay.assert_runtime_contract()?;
		orbis.assert_runtime_contract()?;
		match args.phase.as_str() {
			"control" => run_control(&args, &mut relay, &mut orbis).await,
			"broker-pre-restart" => run_broker_pre(&args, &relay, &orbis).await,
			"broker-post-restart" => run_broker_post(&args, &relay, &orbis).await,
			_ => bail!("unsupported phase {}", args.phase),
		}
	}
	.await;
	match result {
		Ok(records) => evidence.cases = records,
		Err(error) => {
			for id in expected_cases(&args.phase).unwrap() {
				evidence
					.cases
					.entry((*id).into())
					.or_insert_with(|| failed_record("phase dependency", &error));
			}
		},
	}
	for id in expected_cases(&args.phase).unwrap() {
		evidence.cases.entry((*id).into()).or_insert_with(|| {
			CaseRecord::capability_gap(
				"unimplemented case",
				"driver returned no record",
				"case-specific live executor",
			)
		});
	}
	evidence.status = if evidence.cases.values().all(|record| record.status == CaseStatus::Pass) {
		CaseStatus::Pass
	} else {
		CaseStatus::Failed
	};
	if let Some(parent) = args.output.parent() {
		fs::create_dir_all(parent)?
	}
	fs::write(&args.output, serde_json::to_vec_pretty(&evidence)?)?;
	if evidence.status != CaseStatus::Pass {
		bail!(
			"phase {} did not pass; typed failure records written to {}",
			args.phase,
			args.output.display()
		)
	}
	Ok(())
}
