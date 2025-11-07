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

use sc_cli::{utils, with_crypto_scheme, CryptoScheme, Error, SubstrateCli};
use sc_keystore::LocalKeystore;
use sc_service::config::{BasePath, KeystoreConfig};
use sp_core::crypto::{AccountId32, KeyTypeId, SecretString};
use sp_keystore::{Keystore, KeystorePtr};
use std::sync::Arc;

use crate::subcommands::{GenSessionKeysCmd, KeySubcommand};

impl KeySubcommand {
	/// Run the command
	pub fn run<C: SubstrateCli>(&self, cli: &C) -> Result<(), Error> {
		match self {
			Self::GenerateSessionKeys(cmd) => cmd.run(cli),
			Self::Key(cmd) => cmd.run(cli),
		}
	}
}

const KEY_TYPES: [(KeyTypeId, CryptoScheme); 3] = [
	(KeyTypeId(*b"babe"), CryptoScheme::Sr25519),
	(KeyTypeId(*b"gran"), CryptoScheme::Ed25519),
	(KeyTypeId(*b"audi"), CryptoScheme::Sr25519),
];

impl GenSessionKeysCmd {
	/// Run the command
	pub fn run<C: SubstrateCli>(&self, cli: &C) -> Result<(), Error> {
		let suri = utils::read_uri(self.suri.as_ref())?;
		let base_path = self
			.shared_params
			.base_path()?
			.unwrap_or_else(|| BasePath::from_project("", "", &C::executable_name()));
		let chain_id = self.shared_params.chain_id(self.shared_params.is_dev());
		let chain_spec = cli.load_spec(&chain_id)?;
		let config_dir = base_path.config_dir(chain_spec.id());

		let mut public_keys_bytes = Vec::with_capacity(160);
		for (key_type_id, crypto_scheme) in KEY_TYPES {
			let (keystore, public) = match self.keystore_params.keystore_config(&config_dir)? {
				KeystoreConfig::Path { path, password } => {
					let public =
						with_crypto_scheme!(crypto_scheme, to_vec(&suri, password.clone()))?;
					let keystore: KeystorePtr = Arc::new(LocalKeystore::open(path, password)?);
					(keystore, public)
				},
				_ => unreachable!("keystore_config always returns path and password; qed"),
			};

			Keystore::insert(&*keystore, key_type_id, &suri, &public[..])
				.map_err(|_| Error::KeystoreOperation)?;

			public_keys_bytes.extend_from_slice(&public[..]);
		}

		let mut buffer = [0; 32];

		// babe
		buffer.copy_from_slice(&public_keys_bytes[..32]);
		println!("babe: {}", AccountId32::new(buffer));

		// grandpa
		buffer.copy_from_slice(&public_keys_bytes[32..64]);
		println!("grandpa: {}", AccountId32::new(buffer));

		// authority discovery
		buffer.copy_from_slice(&public_keys_bytes[64..96]);
		println!("authority_discovery: {}", AccountId32::new(buffer));

		println!("Session Keys: 0x{}", hex::encode(public_keys_bytes));

		Ok(())
	}
}

fn to_vec<P: sp_core::Pair>(uri: &str, pass: Option<SecretString>) -> Result<Vec<u8>, Error> {
	let p = utils::pair_from_suri::<P>(uri, pass)?;
	Ok(p.public().as_ref().to_vec())
}
