use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

use super::common::{
	invalid, AccountId, BlockNumber, ContentHash, DomainResult, FinalizedQuery, FinalizedValue,
	ProviderReference, ReservationId, SubmitAndFinalize, Validate,
};

pub type StorageRead = FinalizedQuery<StorageQuery>;
pub type StorageWrite = SubmitAndFinalize<StorageCommand>;

/// Canonical decimal representation of a runtime `u64`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DecimalU64(String);

impl DecimalU64 {
	pub fn new(value: impl Into<String>) -> DomainResult<Self> {
		let value = value.into();
		let parsed = value.parse::<u64>().map_err(|_| invalid("invalid decimal u64"))?;
		if parsed.to_string() != value {
			return Err(invalid("expected a canonical decimal u64 string"));
		}
		Ok(Self(value))
	}

	pub fn from_u64(value: u64) -> Self {
		Self(value.to_string())
	}

	pub fn as_u64(&self) -> DomainResult<u64> {
		self.validate()?;
		self.0.parse().map_err(|_| invalid("invalid decimal u64"))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl Validate for DecimalU64 {
	fn validate(&self) -> DomainResult<()> {
		Self::new(self.0.clone()).map(|_| ())
	}
}

/// Canonical standard-alphabet padded base64 content.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Base64Content(String);

impl Base64Content {
	pub fn new(value: impl Into<String>) -> DomainResult<Self> {
		let value = value.into();
		let bytes = STANDARD
			.decode(&value)
			.map_err(|_| invalid("content must be canonical padded base64"))?;
		if bytes.is_empty() || STANDARD.encode(&bytes) != value {
			return Err(invalid("content must be non-empty canonical padded base64"));
		}
		Ok(Self(value))
	}

	pub fn decode(&self) -> DomainResult<Vec<u8>> {
		self.validate()?;
		STANDARD.decode(&self.0).map_err(|_| invalid("invalid base64 content"))
	}
}

impl Validate for Base64Content {
	fn validate(&self) -> DomainResult<()> {
		Self::new(self.0.clone()).map(|_| ())
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HashingAlgorithm {
	Blake2b256,
	Sha2_256,
	Keccak256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CidConfig {
	pub codec: DecimalU64,
	pub hashing: HashingAlgorithm,
}

impl Validate for CidConfig {
	fn validate(&self) -> DomainResult<()> {
		self.codec.validate()
	}
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorageRef {
	pub block: BlockNumber,
	pub transaction_index: u32,
}

impl Validate for StorageRef {
	fn validate(&self) -> DomainResult<()> {
		Ok(())
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransactionRef {
	Position { block: BlockNumber, index: u32 },
	ContentHash { content_hash: ContentHash },
}

impl Validate for TransactionRef {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::Position { .. } => Ok(()),
			Self::ContentHash { content_hash } => content_hash.validate(),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountAuthorization {
	pub expires_at: BlockNumber,
	pub bytes_allowance: DecimalU64,
	pub bytes_used: DecimalU64,
	pub bytes_permanent_used: DecimalU64,
	pub transactions_allowance: u32,
	pub transactions_used: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StorageActor {
	Account { account: AccountId },
	Root,
	Preimage { content_hash: ContentHash },
	AutoRenew { account: AccountId },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceClosure {
	Cancelled,
	Expired,
	Exhausted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveResourceReservation {
	pub owner: AccountId,
	pub purpose_digest: ContentHash,
	pub bytes_remaining: DecimalU64,
	pub transactions_remaining: u32,
	pub created_at: BlockNumber,
	pub expires_at: BlockNumber,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceReservationTombstone {
	pub owner: AccountId,
	pub purpose_digest: ContentHash,
	pub final_bytes_remaining: DecimalU64,
	pub final_transactions_remaining: u32,
	pub outcome: ResourceClosure,
	pub closed_at: BlockNumber,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", content = "reservation", rename_all = "snake_case")]
pub enum ResourceReservationView {
	Active(ActiveResourceReservation),
	Tombstone(ResourceReservationTombstone),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceReservationLink {
	pub reservation_id: ReservationId,
	pub content_hash: ContentHash,
	pub storage_ref: StorageRef,
	pub owner: AccountId,
	pub size: u32,
	pub retention_boundary: BlockNumber,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "response", content = "result", rename_all = "snake_case")]
pub enum StorageResponse {
	AccountAuthorization(FinalizedValue<AccountAuthorization>),
	CanStore(FinalizedValue<bool>),
	CanRenew(FinalizedValue<bool>),
	StoredContentProvenance(FinalizedValue<StorageActor>),
	ResourceReservation(FinalizedValue<ResourceReservationView>),
	ResourceReservationLink(FinalizedValue<ResourceReservationLink>),
	ResourceProviderRef(FinalizedValue<ProviderReference>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "query", content = "arguments", rename_all = "snake_case")]
pub enum StorageQuery {
	AccountAuthorization { account: AccountId },
	CanStore { account: AccountId, data_len: u32 },
	CanRenew { account: AccountId, entry: TransactionRef },
	StoredContentProvenance { reference: StorageRef },
	ResourceReservation { reservation_id: ReservationId },
	ResourceReservationLink { reservation_id: ReservationId, content_hash: ContentHash },
	ResourceProviderRef { reservation_id: ReservationId },
}

impl Validate for StorageQuery {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::AccountAuthorization { account } => account.validate(),
			Self::CanStore { account, data_len } => {
				let _ = data_len;
				account.validate()
			},
			Self::CanRenew { account, entry } => {
				account.validate()?;
				entry.validate()
			},
			Self::StoredContentProvenance { reference } => reference.validate(),
			Self::ResourceReservation { reservation_id }
			| Self::ResourceProviderRef { reservation_id } => reservation_id.validate(),
			Self::ResourceReservationLink { reservation_id, content_hash } => {
				reservation_id.validate()?;
				content_hash.validate()
			},
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", content = "arguments", rename_all = "snake_case")]
pub enum StorageCommand {
	Store {
		content_base64: Base64Content,
	},
	StoreWithCidConfig {
		cid_config: CidConfig,
		content_base64: Base64Content,
	},
	StoreReserved {
		reservation_id: ReservationId,
		cid_config: CidConfig,
		content_base64: Base64Content,
	},
	RenewReserved {
		reservation_id: ReservationId,
		content_hash: ContentHash,
	},
	AttachProvider {
		reservation_id: ReservationId,
		provider_ref: ProviderReference,
	},
	Renew {
		entry: TransactionRef,
	},
	ForceRenew {
		entry: TransactionRef,
	},
	EnableAutoRenew {
		content_hash: ContentHash,
	},
	DisableAutoRenew {
		content_hash: ContentHash,
	},
}

impl Validate for StorageCommand {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::Store { content_base64 } => content_base64.validate(),
			Self::StoreWithCidConfig { cid_config, content_base64 } => {
				cid_config.validate()?;
				content_base64.validate()
			},
			Self::StoreReserved { reservation_id, cid_config, content_base64 } => {
				reservation_id.validate()?;
				cid_config.validate()?;
				content_base64.validate()
			},
			Self::RenewReserved { reservation_id, content_hash } => {
				reservation_id.validate()?;
				content_hash.validate()
			},
			Self::AttachProvider { reservation_id, provider_ref } => {
				reservation_id.validate()?;
				provider_ref.validate()
			},
			Self::Renew { entry } | Self::ForceRenew { entry } => entry.validate(),
			Self::EnableAutoRenew { content_hash } | Self::DisableAutoRenew { content_hash } => {
				content_hash.validate()
			},
		}
	}
}
