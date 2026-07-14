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

use serde::{Deserialize, Serialize};

use super::common::{
	ensure_bytes, AccountId, ContentCommitment, DomainResult, DriveId, FinalizedQuery, PageRequest,
	SubmitAndFinalize, Validate,
};

pub const MAX_DRIVE_NAME_BYTES: usize = 128;

pub type DriveRead = FinalizedQuery<DriveQuery>;
pub type DriveWrite = SubmitAndFinalize<DriveCommand>;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveStatus {
	Active,
	Archived,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DriveName(Vec<u8>);

impl DriveName {
	pub fn new(value: Vec<u8>) -> DomainResult<Self> {
		ensure_bytes(&value, 1, MAX_DRIVE_NAME_BYTES, "drive name")?;
		Ok(Self(value))
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.0
	}
}

impl Validate for DriveName {
	fn validate(&self) -> DomainResult<()> {
		ensure_bytes(&self.0, 1, MAX_DRIVE_NAME_BYTES, "drive name")
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DriveView {
	pub drive: DriveId,
	pub owner: AccountId,
	pub name: DriveName,
	pub root_storage_ref: Option<ContentCommitment>,
	pub version: u64,
	pub status: DriveStatus,
	pub created_at: super::common::BlockNumber,
	pub updated_at: super::common::BlockNumber,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "response", content = "result", rename_all = "snake_case")]
pub enum DriveResponse {
	Drive(super::common::FinalizedValue<DriveView>),
	Drives(super::common::FinalizedPage<DriveId>),
	Controllers(super::common::FinalizedPage<AccountId>),
	NextDriveNonce(super::common::FinalizedValue<u64>),
	IsDriveOwner(super::common::FinalizedValue<bool>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "query", content = "arguments", rename_all = "snake_case")]
pub enum DriveQuery {
	DriveById { drive: DriveId },
	OwnerDrives { owner: AccountId, page: PageRequest },
	Controllers { drive: DriveId, page: PageRequest },
	NextDriveNonce { owner: AccountId },
	IsDriveOwner { owner: AccountId, drive: DriveId },
}

impl Validate for DriveQuery {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::DriveById { drive } => drive.validate(),
			Self::OwnerDrives { owner, page } => {
				owner.validate()?;
				page.validate()
			},
			Self::Controllers { drive, page } => {
				drive.validate()?;
				page.validate()
			},
			Self::NextDriveNonce { owner } => owner.validate(),
			Self::IsDriveOwner { owner, drive } => {
				owner.validate()?;
				drive.validate()
			},
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", content = "arguments", rename_all = "snake_case")]
pub enum DriveCommand {
	Create {
		name: DriveName,
		root_storage_ref: Option<ContentCommitment>,
	},
	UpdateRoot {
		drive: DriveId,
		expected_version: u64,
		root_storage_ref: Option<ContentCommitment>,
	},
	SetController {
		drive: DriveId,
		controller: AccountId,
		enabled: bool,
	},
	Transfer {
		drive: DriveId,
		new_owner: AccountId,
	},
	Archive {
		drive: DriveId,
	},
}

impl Validate for DriveCommand {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::Create { name, root_storage_ref } => {
				name.validate()?;
				root_storage_ref.as_ref().map_or(Ok(()), Validate::validate)
			},
			Self::UpdateRoot { drive, expected_version, root_storage_ref } => {
				drive.validate()?;
				if *expected_version == 0 {
					return Err(super::common::invalid("expected drive version must be non-zero"));
				}
				root_storage_ref.as_ref().map_or(Ok(()), Validate::validate)
			},
			Self::SetController { drive, controller, .. } => {
				drive.validate()?;
				controller.validate()
			},
			Self::Transfer { drive, new_owner } => {
				drive.validate()?;
				new_owner.validate()
			},
			Self::Archive { drive } => drive.validate(),
		}
	}
}
