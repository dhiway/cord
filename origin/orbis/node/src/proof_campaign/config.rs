use std::{path::PathBuf, str::FromStr, sync::Arc};

use clap::{Parser, ValueEnum};
use polkadot_omni_node_lib::chain_spec::LoadSpec;
use sc_cli::{
	ChainSpec, CliConfiguration, DefaultConfigurationValues, ImportParams, KeystoreParams,
	NetworkParams, Result, RpcEndpoint, SharedParams, SubstrateCli,
};
use sc_service::config::{BasePath, PrometheusConfig};

use crate::chain_spec::{ChainSpecLoader, Extensions};

/// The only chain id on which the fault binary is allowed to start.
pub const ISOLATED_CHAIN_ID: &str = "orbis-proof-isolated";
/// A single fail-closed authoring fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum FaultKind {
	/// Omit a proof after proving that the canonical provider produced one.
	#[value(name = "Missing")]
	Missing,
	/// Corrupt a canonical proof.
	#[value(name = "Invalid")]
	Invalid,
	/// Supply the previous retention target's proof.
	#[value(name = "Stale")]
	Stale,
	/// Probe a second copy of the mandatory proof inherent and require `BadMandatory`.
	#[value(name = "Duplicate")]
	Duplicate,
}

impl FaultKind {
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Missing => "Missing",
			Self::Invalid => "Invalid",
			Self::Stale => "Stale",
			Self::Duplicate => "Duplicate",
		}
	}
}

/// Validated immutable campaign settings shared by the provider and proposer.
#[derive(Clone, Debug)]
pub struct Campaign {
	pub fault: FaultKind,
	pub target: u32,
	pub expected_genesis_hash: sp_core::H256,
}

impl Campaign {
	fn validate(
		chain_id: &str,
		role_is_authority: bool,
		fault: FaultKind,
		target: u32,
		expected_genesis_hash: &str,
		acknowledge_disposable: bool,
	) -> std::result::Result<Self, String> {
		if chain_id != ISOLATED_CHAIN_ID {
			return Err(format!(
				"proof campaign refuses chain id {chain_id:?}; expected {ISOLATED_CHAIN_ID:?}"
			));
		}
		if !role_is_authority {
			return Err("proof campaign must run as a collator/authority".into());
		}
		if target == 0 {
			return Err("proof campaign target must be a positive authored block number".into());
		}
		if !acknowledge_disposable {
			return Err(
				"missing --unsafe-proof-campaign-acknowledge-disposable acknowledgement".into()
			);
		}
		if !expected_genesis_hash.starts_with("0x") || expected_genesis_hash.len() != 66 {
			return Err(
				"--proof-campaign-expected-genesis-hash must be a 0x-prefixed 32-byte hash".into(),
			);
		}
		let expected_genesis_hash = sp_core::H256::from_str(expected_genesis_hash)
			.map_err(|error| format!("invalid expected genesis hash: {error}"))?;
		Ok(Self { fault, target, expected_genesis_hash })
	}
}

#[derive(Debug, Parser)]
#[command(name = "origin-orbis-proof-campaign", propagate_version = true)]
pub struct Cli {
	#[command(flatten)]
	pub run: cumulus_client_cli::RunCmd,

	/// Fault to inject exactly once.
	#[arg(long = "proof-campaign-mode", value_enum, ignore_case = true)]
	pub fault: FaultKind,

	/// Exact authored Orbis block at which to inject the fault.
	#[arg(long = "proof-campaign-target-block")]
	pub target: u32,

	/// Expected genesis hash of the disposable Orbis chain.
	#[arg(long = "proof-campaign-expected-genesis-hash")]
	pub expected_genesis_hash: String,

	/// Required acknowledgement that this is a disposable isolated network.
	#[arg(long = "unsafe-proof-campaign-acknowledge-disposable", action = clap::ArgAction::SetTrue)]
	pub acknowledge_disposable: bool,

	/// Relay-chain arguments after `--`.
	#[arg(raw = true)]
	pub relay_chain_args: Vec<String>,
}

#[derive(Debug)]
struct RelayChainCli {
	base: polkadot_cli::RunCmd,
	chain_id: Option<String>,
	base_path: Option<PathBuf>,
}

impl RelayChainCli {
	fn new<'a>(
		para_config: &sc_service::Configuration,
		relay_chain_args: impl Iterator<Item = &'a String>,
	) -> Self {
		let chain_id =
			sc_chain_spec::get_extension::<Extensions>(para_config.chain_spec.extensions())
				.map(|extension| extension.relay_chain.clone());
		Self {
			base: Parser::parse_from(relay_chain_args),
			chain_id,
			base_path: Some(para_config.base_path.path().join("polkadot")),
		}
	}
}

macro_rules! impl_substrate_cli {
	($ty:ty) => {
		impl SubstrateCli for $ty {
			fn impl_name() -> String {
				"Origin Orbis disposable proof campaign".into()
			}
			fn impl_version() -> String {
				env!("SUBSTRATE_CLI_IMPL_VERSION").into()
			}
			fn description() -> String {
				"One-shot transaction-storage proof fault collator for an isolated Orbis network"
					.into()
			}
			fn author() -> String {
				env!("CARGO_PKG_AUTHORS").into()
			}
			fn support_url() -> String {
				"https://github.com/dhiway/cord/issues".into()
			}
			fn copyright_start_year() -> i32 {
				2026
			}
			fn load_spec(
				&self,
				id: &str,
			) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
				ChainSpecLoader.load_spec(id)
			}
		}
	};
}

impl_substrate_cli!(Cli);

impl SubstrateCli for RelayChainCli {
	fn impl_name() -> String {
		Cli::impl_name()
	}
	fn impl_version() -> String {
		Cli::impl_version()
	}
	fn description() -> String {
		Cli::description()
	}
	fn author() -> String {
		Cli::author()
	}
	fn support_url() -> String {
		Cli::support_url()
	}
	fn copyright_start_year() -> i32 {
		2026
	}
	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn ChainSpec>, String> {
		polkadot_cli::Cli::from_iter([RelayChainCli::executable_name()]).load_spec(id)
	}
}

impl DefaultConfigurationValues for RelayChainCli {
	fn p2p_listen_port() -> u16 {
		30334
	}
	fn rpc_listen_port() -> u16 {
		9945
	}
	fn prometheus_listen_port() -> u16 {
		9616
	}
}

impl CliConfiguration<Self> for RelayChainCli {
	fn shared_params(&self) -> &SharedParams {
		self.base.base.shared_params()
	}
	fn import_params(&self) -> Option<&ImportParams> {
		self.base.base.import_params()
	}
	fn network_params(&self) -> Option<&NetworkParams> {
		self.base.base.network_params()
	}
	fn keystore_params(&self) -> Option<&KeystoreParams> {
		self.base.base.keystore_params()
	}
	fn base_path(&self) -> Result<Option<BasePath>> {
		Ok(self
			.shared_params()
			.base_path()?
			.or_else(|| self.base_path.clone().map(Into::into)))
	}
	fn rpc_addr(&self, port: u16) -> Result<Option<Vec<RpcEndpoint>>> {
		self.base.base.rpc_addr(port)
	}
	fn prometheus_config(
		&self,
		port: u16,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<PrometheusConfig>> {
		self.base.base.prometheus_config(port, chain_spec)
	}
	fn init<F>(&self, _: &String, _: &String, _: F) -> Result<()>
	where
		F: FnOnce(&mut sc_cli::LoggerBuilder),
	{
		unreachable!("relay-chain CLI is configured by the parachain runner")
	}
	fn chain_id(&self, is_dev: bool) -> Result<String> {
		let configured = self.base.base.chain_id(is_dev)?;
		Ok(if configured.is_empty() {
			self.chain_id.clone().unwrap_or_default()
		} else {
			configured
		})
	}
	fn role(&self, is_dev: bool) -> Result<sc_service::Role> {
		self.base.base.role(is_dev)
	}
	fn transaction_pool(&self, is_dev: bool) -> Result<sc_service::config::TransactionPoolOptions> {
		self.base.base.transaction_pool(is_dev)
	}
	fn trie_cache_maximum_size(&self) -> Result<Option<usize>> {
		self.base.base.trie_cache_maximum_size()
	}
	fn rpc_methods(&self) -> Result<sc_service::config::RpcMethods> {
		self.base.base.rpc_methods()
	}
	fn rpc_max_connections(&self) -> Result<u32> {
		self.base.base.rpc_max_connections()
	}
	fn rpc_cors(&self, is_dev: bool) -> Result<Option<Vec<String>>> {
		self.base.base.rpc_cors(is_dev)
	}
	fn default_heap_pages(&self) -> Result<Option<u64>> {
		self.base.base.default_heap_pages()
	}
	fn force_authoring(&self) -> Result<bool> {
		self.base.base.force_authoring()
	}
	fn disable_grandpa(&self) -> Result<bool> {
		self.base.base.disable_grandpa()
	}
	fn max_runtime_instances(&self) -> Result<Option<usize>> {
		self.base.base.max_runtime_instances()
	}
	fn announce_block(&self) -> Result<bool> {
		self.base.base.announce_block()
	}
	fn telemetry_endpoints(
		&self,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<sc_telemetry::TelemetryEndpoints>> {
		self.base.base.telemetry_endpoints(chain_spec)
	}
	fn node_name(&self) -> Result<String> {
		self.base.base.node_name()
	}
}

pub fn run() -> Result<()> {
	let cli = Arc::new(Cli::from_args());
	let runner = cli.create_runner(&cli.run.normalize())?;
	let collator_options = cli.run.collator_options();
	runner.run_node_until_exit(move |config| {
		let cli = cli.clone();
		async move {
			let campaign = Campaign::validate(
				config.chain_spec.id(),
				config.role.is_authority(),
				cli.fault,
				cli.target,
				&cli.expected_genesis_hash,
				cli.acknowledge_disposable,
			)
			.map_err(sc_cli::Error::Input)?;
			let relay_cli = RelayChainCli::new(
				&config,
				[RelayChainCli::executable_name()].iter().chain(cli.relay_chain_args.iter()),
			);
			let relay_config = SubstrateCli::create_configuration(
				&relay_cli,
				&relay_cli,
				config.tokio_handle.clone(),
			)
			.map_err(|error| {
				sc_cli::Error::Input(format!("relay-chain argument error: {error}"))
			})?;
			super::service::start(config, relay_config, collator_options, campaign)
				.await
				.map(|(tasks, _)| tasks)
				.map_err(sc_cli::Error::from)
		}
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn validation_requires_all_isolation_guards() {
		let hash = format!("0x{}", "11".repeat(32));
		assert!(
			Campaign::validate("orbis-local", true, FaultKind::Missing, 1, &hash, true).is_err()
		);
		assert!(Campaign::validate(ISOLATED_CHAIN_ID, false, FaultKind::Missing, 1, &hash, true,)
			.is_err());
		assert!(Campaign::validate(ISOLATED_CHAIN_ID, true, FaultKind::Missing, 0, &hash, true)
			.is_err());
		assert!(Campaign::validate(ISOLATED_CHAIN_ID, true, FaultKind::Missing, 1, &hash, false)
			.is_err());
		assert!(
			Campaign::validate(ISOLATED_CHAIN_ID, true, FaultKind::Missing, 1, "11", true).is_err()
		);
		assert_eq!(
			Campaign::validate(ISOLATED_CHAIN_ID, true, FaultKind::Duplicate, 7, &hash, true)
				.unwrap()
				.target,
			7
		);
	}
}
