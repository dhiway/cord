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

//! Genesis configs presets for the CORD Origin relay staging runtime

use crate::*;
#[cfg(not(feature = "std"))]
use alloc::format;
use babe_primitives::AuthorityId as BabeId;
use beefy_primitives::ecdsa_crypto::AuthorityId as BeefyId;
use origin_runtime_constants::currency::UNITS as ORU;
use pallet_grandpa::AuthorityId as GrandpaId;
use polkadot_primitives::{
	node_features::FeatureIndex, vstaging::SchedulerParams, AccountPublic, AssignmentId,
	AsyncBackingParams,
};
use runtime_parachains::configuration::HostConfiguration;
use sp_core::{sr25519, Pair, Public};
use sp_genesis_builder::PresetId;
use sp_keyring::Sr25519Keyring;
use sp_runtime::traits::IdentifyAccount;

/// Helper function to generate a crypto pair from seed
fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate stash, controller and session key from seed
fn get_authority_keys_from_seed(
	seed: &str,
) -> (
	AccountId,
	AccountId,
	BabeId,
	GrandpaId,
	ValidatorId,
	AssignmentId,
	AuthorityDiscoveryId,
	BeefyId,
) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<ValidatorId>(seed),
		get_from_seed::<AssignmentId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
		get_from_seed::<BeefyId>(seed),
	)
}

fn testnet_accounts() -> Vec<AccountId> {
	Sr25519Keyring::well_known().map(|k| k.to_account_id()).collect()
}

fn default_parachains_host_configuration() -> HostConfiguration<polkadot_primitives::BlockNumber> {
	use polkadot_primitives::{MAX_CODE_SIZE, MAX_POV_SIZE};

	runtime_parachains::configuration::HostConfiguration {
		validation_upgrade_cooldown: 2u32,
		validation_upgrade_delay: 2,
		code_retention_period: 1200,
		max_code_size: MAX_CODE_SIZE,
		max_pov_size: MAX_POV_SIZE,
		max_head_data_size: 32 * 1024,
		max_upward_queue_count: 8,
		max_upward_queue_size: 1024 * 1024,
		max_downward_message_size: 1024 * 1024,
		max_upward_message_size: 50 * 1024,
		max_upward_message_num_per_candidate: 5,
		hrmp_sender_deposit: 0,
		hrmp_recipient_deposit: 0,
		hrmp_channel_max_capacity: 8,
		hrmp_channel_max_total_size: 8 * 1024,
		hrmp_max_parachain_inbound_channels: 4,
		hrmp_channel_max_message_size: 1024 * 1024,
		hrmp_max_parachain_outbound_channels: 4,
		hrmp_max_message_num_per_candidate: 5,
		dispute_period: 6,
		no_show_slots: 2,
		n_delay_tranches: 25,
		needed_approvals: 2,
		relay_vrf_modulo_samples: 2,
		zeroth_delay_tranche_width: 0,
		minimum_validation_upgrade_delay: 5,
		scheduler_params: SchedulerParams {
			group_rotation_frequency: 20,
			paras_availability_period: 4,
			lookahead: 3,
			..Default::default()
		},
		dispute_post_conclusion_acceptance_period: 100u32,
		minimum_backing_votes: 1,
		node_features: NodeFeatures::from_element(
			(1u8 << (FeatureIndex::EnableAssignmentsV2 as usize)) |
				(1u8 << (FeatureIndex::ElasticScalingMVP as usize)) |
				(1u8 << (FeatureIndex::CandidateReceiptV2 as usize)) |
				(1u8 << (FeatureIndex::CandidateReceiptV3 as usize)),
		),
		async_backing_params: AsyncBackingParams {
			max_candidate_depth: 3,
			allowed_ancestry_len: 2,
		},
		max_relay_parent_session_age: 0,
		executor_params: Default::default(),
		max_validators: None,
		pvf_voting_ttl: 2,
		approval_voting_params: ApprovalVotingParams { max_approval_coalesce_count: 1 },
	}
}

#[allow(clippy::type_complexity)]
fn origin_staging_genesis(
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
		BeefyId,
	)>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> serde_json::Value {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);

	const ENDOWMENT: u128 = 1_000_000_000_000 * ORU;

	serde_json::json!({
		"balances": {
			"balances": endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"authorityManager":  {
			"initialAuthorities": initial_authorities
				.iter()
				.map(|x| x.0.clone())
				.collect::<Vec<_>>(),
		},
		"session": {
			"keys": initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						origin_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
							x.7.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		"babe": {
			"epochConfig": Some(BABE_GENESIS_EPOCH_CONFIG),
		},
		"configuration": {
			"config": default_parachains_host_configuration(),
		},
		"sudo": { "key": Some(root_key) },
	})
}

fn origin_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	para_validator: ValidatorId,
	para_assignment: AssignmentId,
	authority_discovery: AuthorityDiscoveryId,
	beefy: BeefyId,
) -> SessionKeys {
	SessionKeys { babe, grandpa, para_validator, para_assignment, authority_discovery, beefy }
}

pub fn origin_staging_config_genesis() -> serde_json::Value {
	origin_staging_genesis(
		vec![
			get_authority_keys_from_seed("Alice"),
			get_authority_keys_from_seed("Bob"),
			get_authority_keys_from_seed("Chrlie"),
			get_authority_keys_from_seed("Dave"),
			get_authority_keys_from_seed("Eve"),
			get_authority_keys_from_seed("Fredie"),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}
// pub fn origin_staging_config_genesis() -> serde_json::Value {
// 	origin_staging_genesis(
// 		vec![get_authority_keys_from_seed("Alice"), get_authority_keys_from_seed("Bob")],
// 		get_account_id_from_seed::<sr25519::Public>("Alice"),
// 		None,
// 	)
// }

pub fn origin_development_config_genesis() -> serde_json::Value {
	origin_staging_genesis(
		vec![get_authority_keys_from_seed("Alice")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// Provides the names of the predefined genesis configs for this runtime.
pub fn preset_names() -> Vec<PresetId> {
	vec![
		PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
		PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
	]
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
	let patch = match id.as_ref() {
		sp_genesis_builder::DEV_RUNTIME_PRESET => origin_development_config_genesis(),
		sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => origin_staging_config_genesis(),
		_ => return None,
	};
	Some(
		serde_json::to_string(&patch)
			.expect("serialization to json is expected to work. qed.")
			.into_bytes(),
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn default_parachains_host_configuration_is_consistent() {
		default_parachains_host_configuration().panic_if_not_consistent();
	}

	#[test]
	fn default_parachains_host_configuration_supports_elastic_scaling() {
		let config = default_parachains_host_configuration();

		assert_eq!(config.scheduler_params.lookahead, 3);
		assert_eq!(config.async_backing_params.max_candidate_depth, 3);
		assert!(FeatureIndex::EnableAssignmentsV2.is_set(&config.node_features));
		assert!(FeatureIndex::ElasticScalingMVP.is_set(&config.node_features));
		assert!(FeatureIndex::CandidateReceiptV2.is_set(&config.node_features));
		assert!(FeatureIndex::CandidateReceiptV3.is_set(&config.node_features));
	}

	#[test]
	fn enterprise_genesis_uses_sudo_managed_non_staking_authorities() {
		let genesis = origin_staging_config_genesis();

		assert!(genesis.get("sudo").is_some());
		assert!(genesis.get("authorityManager").is_some());
		assert!(genesis.get("session").is_some());
		assert!(genesis.get("staking").is_none());
		assert!(genesis.get("referenda").is_none());
		assert!(genesis.get("convictionVoting").is_none());
	}
}
