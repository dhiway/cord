// This file is part of CORD – https://cord.network
//
// Typed request/response DTOs and error taxonomy for runtime view functions.

use crate::{authorization::Authorization, identifier::Ss58Identifier, AccountId, Signature};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

use scale_decode::DecodeAsType;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

/// Maximum payload length (in bytes) supported by the portable view authorization DTOs.
pub const AUTHORIZATION_MAX_BYTES: u32 = 256;
type AuthorizationPayloadLimit = ConstU32<AUTHORIZATION_MAX_BYTES>;

/// Bounded payload used when constructing [`AuthorizationRequest`].
pub type AuthorizationPayload = BoundedVec<u8, AuthorizationPayloadLimit>;

/// Convenience alias for the client-facing view authorization DTO.
pub type AuthorizationRequest = Authorization<AccountId, AuthorizationPayload, Signature>;

/// Maximum attribute key bytes supported by portable DTOs.
pub const MAX_ATTRIBUTE_KEY_BYTES: u32 = 1024;
type AttributeKeyLimit = ConstU32<MAX_ATTRIBUTE_KEY_BYTES>;

/// Attribute key shape shared by entity/register view DTOs.
pub type AttributeKey = BoundedVec<u8, AttributeKeyLimit>;

/// Request payload for `Register::details`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RegisterDetailsRequest {
	pub auth: AuthorizationRequest,
	pub registry: Ss58Identifier,
}

/// Request payload for `Register::lookup_specs`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RegisterLookupSpecsRequest {
	pub auth: AuthorizationRequest,
	pub registry: Ss58Identifier,
}

/// Request payload for `Register::packet_snapshot`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RegisterPacketSnapshotRequest {
	pub auth: AuthorizationRequest,
	pub registry: Ss58Identifier,
	pub packet: Ss58Identifier,
	pub version: Option<u32>,
}

/// Request payload for `Entity::attribute_history_entries`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EntityAttributeHistoryRequest {
	pub auth: AuthorizationRequest,
	pub token: Ss58Identifier,
}

/// Request payload for `Entity::attribute_history_for_key_entries`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EntityAttributeHistoryForKeyRequest {
	pub auth: AuthorizationRequest,
	pub token: Ss58Identifier,
	pub key: AttributeKey,
}

/// Request payload for `Entity::attribute_history_entry_view`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EntityAttributeHistoryEntryRequest {
	pub auth: AuthorizationRequest,
	pub token: Ss58Identifier,
	pub key: AttributeKey,
	pub version: u64,
}

/// Request payload for `Entity::account_token`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EntityAccountTokenRequest {
	pub auth: AuthorizationRequest,
	pub account: AccountId,
}

/// Request payload for `Entity::entity_info_bytes`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EntityInfoBytesRequest {
	pub auth: AuthorizationRequest,
	pub token: Ss58Identifier,
}

/// Request payload for `Entity::linked_accounts`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EntityLinkedAccountsRequest {
	pub auth: AuthorizationRequest,
	pub token: Ss58Identifier,
}

/// Request payload for `Entity::entity_nym`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EntityNymRequest {
	pub auth: AuthorizationRequest,
	pub token: Ss58Identifier,
}

/// Request payload for `Token::state_version`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TokenStateVersionRequest {
	pub auth: AuthorizationRequest,
	pub token: Ss58Identifier,
}

/// Request payload for `Token::resolve_identifier`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TokenResolveIdentifierRequest {
	pub auth: AuthorizationRequest,
	pub token: Ss58Identifier,
}

/// Request payload for `Token::timeline`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TokenTimelineRequest {
	pub auth: AuthorizationRequest,
	pub token: Ss58Identifier,
	pub start: Option<u32>,
	pub limit: Option<u32>,
}

/// Request payload for `Token::resolve_pallet`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TokenResolvePalletRequest {
	pub auth: AuthorizationRequest,
	pub index: u16,
}

/// Shared error taxonomy surfaced by SDKs and pallets.
#[derive(
	Copy, Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, DecodeAsType,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AuthorizationError {
	Unauthorized,
	NotFound,
	InvalidInput,
	TooLarge,
	Internal,
}

#[cfg(test)]
mod tests {
	use super::*;
	use alloc::vec;
	use sp_core::sr25519;

	fn sample_auth() -> AuthorizationRequest {
		let payload: AuthorizationPayload =
			BoundedVec::try_from(vec![b'a'; 32]).expect("payload within bounds");
		let sig = Signature::from(sr25519::Signature::from_raw([1u8; 64]));
		let account = AccountId::from([2u8; 32]);
		AuthorizationRequest { account, payload, signature: sig }
	}

	fn sample_token(id: u8) -> Ss58Identifier {
		let digest = [id; 32];
		Ss58Identifier::to_encoded(digest, 100, 4, 0).expect("valid identifier")
	}

	fn key(bytes: &[u8]) -> AttributeKey {
		BoundedVec::try_from(bytes.to_vec()).expect("bounded key")
	}

	fn roundtrip<T>(value: &T)
	where
		T: Encode + Decode + PartialEq + core::fmt::Debug,
	{
		let encoded = value.encode();
		let decoded = T::decode(&mut &encoded[..]).expect("decode");
		assert_eq!(&decoded, value);
	}

	#[test]
	fn register_requests_roundtrip() {
		let auth = sample_auth();
		let reg = sample_token(9);
		roundtrip(&RegisterDetailsRequest { auth: auth.clone(), registry: reg.clone() });
		roundtrip(&RegisterLookupSpecsRequest { auth: auth.clone(), registry: reg.clone() });
		roundtrip(&RegisterPacketSnapshotRequest {
			auth: auth.clone(),
			registry: reg.clone(),
			packet: sample_token(10),
			version: Some(3),
		});
	}

	#[test]
	fn entity_requests_roundtrip() {
		let auth = sample_auth();
		let token = sample_token(11);
		roundtrip(&EntityAttributeHistoryRequest { auth: auth.clone(), token: token.clone() });
		roundtrip(&EntityAttributeHistoryForKeyRequest {
			auth: auth.clone(),
			token: token.clone(),
			key: key(b"name"),
		});
		roundtrip(&EntityAttributeHistoryEntryRequest {
			auth: auth.clone(),
			token: token.clone(),
			key: key(b"name"),
			version: 42,
		});
		roundtrip(&EntityAccountTokenRequest {
			auth: auth.clone(),
			account: AccountId::from([3u8; 32]),
		});
		roundtrip(&EntityInfoBytesRequest { auth: auth.clone(), token });
	}

	#[test]
	fn token_requests_roundtrip() {
		let auth = sample_auth();
		let token = sample_token(7);
		roundtrip(&TokenStateVersionRequest { auth: auth.clone(), token: token.clone() });
		roundtrip(&TokenResolveIdentifierRequest { auth: auth.clone(), token: token.clone() });
		roundtrip(&TokenTimelineRequest {
			auth: auth.clone(),
			token: token.clone(),
			start: Some(5),
			limit: Some(10),
		});
		roundtrip(&TokenResolvePalletRequest { auth, index: 12 });
	}

	#[test]
	fn view_error_roundtrip() {
		for variant in [
			AuthorizationError::Unauthorized,
			AuthorizationError::NotFound,
			AuthorizationError::InvalidInput,
			AuthorizationError::TooLarge,
			AuthorizationError::Internal,
		] {
			roundtrip(&variant);
		}
	}
}
