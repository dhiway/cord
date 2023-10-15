use std::{fs, io::Write, path::PathBuf};

use sc_cli::{
	clap::{self, Args},
	Error,
};
use serde::Deserialize;

use crate::chain_spec::{
	bootstrap::{cord_custom_config, ChainParams},
	ChainType, CordChainSpec,
};

#[derive(Debug, Deserialize)]
pub struct ChainConfigParams {
	pub chain_name: String,
	pub chain_type: String,
	pub authorities: Vec<Vec<String>>,
	pub well_known_nodes: Vec<Vec<String>>,
	pub network_members: Vec<Vec<String>>,
	pub council_members: Option<Vec<String>>,
	pub tech_committee_members: Option<Vec<String>>,
	pub sudo_key: Option<String>,
}

#[derive(Debug, Args)]
pub struct BootstrapChainCmd {
	#[arg(long = "raw")]
	raw: bool,

	#[arg(long, short = 'c')]
	config: PathBuf,
}

impl BootstrapChainCmd {
	pub fn run(&self) -> Result<(), Error> {
		let toml_config = fs::read_to_string(&self.config)?;
		let config: ChainConfigParams =
			toml::from_str(&toml_config).map_err(|e| sc_cli::Error::Application(Box::new(e)))?;

		if config.authorities.is_empty() || config.well_known_nodes.is_empty() {
			eprintln!("Error: authorities and well_known_nodes cannot be empty");
			std::process::exit(1);
		}

		let chain_name = if config.chain_name.len() <= 64 {
			config.chain_name.clone()
		} else {
			return Err("Chain name should not be more than 64 characters".into())
		};

		let chain_type: Result<ChainType, String> = match config.chain_type.as_str() {
			"dev" => Ok(ChainType::Development),
			"local" => Ok(ChainType::Local),
			"live" => Ok(ChainType::Live),
			other => Err(format!(
				"Invalid chain_type: {}. Possible values are 'dev', 'local', 'live'",
				other
			)),
		};

		let chain_type = chain_type?;

		let initial_members: Vec<String> =
			config.network_members.iter().map(|net| net[1].clone()).collect();

		let initial_well_known_nodes: Vec<Vec<String>> = config
			.well_known_nodes
			.iter()
			.map(|node| vec![node[1].clone(), node[2].clone()])
			.collect();

		let initial_authorities: Vec<Vec<String>> = config
			.authorities
			.iter()
			.map(|auth| vec![auth[1].clone(), auth[2].clone()])
			.collect();

		let initial_council_members: Vec<String> =
			if let Some(council_members) = &config.council_members {
				council_members.to_vec()
			} else {
				config.authorities.iter().map(|auth| auth[1].clone()).collect()
			};

		let initial_tech_committee_members: Vec<String> =
			if let Some(committee_members) = &config.tech_committee_members {
				committee_members.to_vec()
			} else {
				config.authorities.iter().map(|auth| auth[1].clone()).collect()
			};

		let initial_sudo_key: String = config.sudo_key.unwrap_or_else(|| {
			config
				.authorities
				.get(0)
				.map(|auth| auth[1].clone())
				.expect("No authorities provided; cannot set sudo_key")
		});

		let chain_params = ChainParams {
			chain_name,
			chain_type,
			authorities: initial_authorities,
			well_known_nodes: initial_well_known_nodes,
			network_members: initial_members,
			council_members: initial_council_members,
			tech_committee_members: initial_tech_committee_members,
			sudo_key: initial_sudo_key,
		};

		let chain_spec: CordChainSpec = cord_custom_config(chain_params)?;

		let json = sc_service::chain_ops::build_spec(&chain_spec, self.raw)?;
		if std::io::stdout().write_all(json.as_bytes()).is_err() {
			let _ = std::io::stderr().write_all(b"Error writing to stdout\n");
		}

		Ok(())
	}
}
