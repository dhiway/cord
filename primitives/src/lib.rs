// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Low-level types used throughout the CORD code.

#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	MultiSignature, OpaqueExtrinsic,
};

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on
/// the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it
/// equivalent to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of
/// them.
pub type AccountIndex = u32;

/// Balance of an account.
pub type Balance = u128;

/// Type used for expressing timestamp.
pub type Moment = u64;

/// Index of a transaction in the chain.
pub type Index = u32;

/// Index of a transaction in the relay chain. 32-bit should be plenty.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

// A timestamp: milliseconds since the unix epoch.
/// `u64` is enough to represent a duration of half a billion years, when the
/// time scale is milliseconds.
pub type Timestamp = u64;

/// Digest item type.
pub type DigestItem = generic::DigestItem<Hash>;
/// Header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type.
pub type Block = generic::Block<Header, OpaqueExtrinsic>;
/// Block ID.
pub type BlockId = generic::BlockId<Block>;

// /// App-specific crypto used for reporting equivocation/misbehavior in AURA
// and /// GRANDPA. Any rewards for misbehavior reporting will be paid out to
// this /// account.
// /// TODO - Explore more
// pub mod report {
// 	use super::{Signature, Verify};
// 	use frame_system::offchain::AppCrypto;
// 	use sp_core::crypto::KeyTypeId;

// 	/// Key type for the reporting module. Used for reporting GRANDPA
// 	/// equivocations.
// 	pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"fish");

// 	mod app {
// 		use sp_application_crypto::{app_crypto, ed25519};
// 		app_crypto!(ed25519, super::KEY_TYPE);
// 	}

// 	/// Identity of the equivocation/misbehavior reporter.
// 	pub type ReporterId = app::Public;

// 	/// An `AppCrypto` type to allow submitting signed transactions using the
// reporting 	/// application key as signer.
// 	pub struct ReporterAppCrypto;

// 	impl AppCrypto<<Signature as Verify>::Signer, Signature> for
// ReporterAppCrypto { 		type RuntimeAppPublic = ReporterId;
// 		type GenericSignature = sp_core::ed25519::Signature;
// 		type GenericPublic = sp_core::ed25519::Public;
// 	}
// }
