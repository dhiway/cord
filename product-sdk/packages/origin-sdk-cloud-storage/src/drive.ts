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

import { invalidDomainInput, type AccountId, type BlockNumber, type ContentCommitment, type DecimalU64, type DriveId, type IdPage } from "./types.ts";

declare const driveType: unique symbol;
export type DriveName = string & { readonly [driveType]: "DriveName" };
export type DriveStatus = "active" | "archived" | "deleted";

export function driveName(value: string): DriveName {
  if (!value || new TextEncoder().encode(value).length > 128) {
    invalidDomainInput("drive", "drive_name", "drive name must contain 1-128 UTF-8 bytes");
  }
  return value as DriveName;
}

export interface DriveView {
  readonly drive_id: DriveId;
  readonly owner: AccountId;
  readonly name: DriveName;
  readonly root_manifest: ContentCommitment | null;
  readonly root_provider_commitment: ContentCommitment | null;
  readonly version: DecimalU64;
  readonly status: DriveStatus;
  readonly created_at: BlockNumber;
  readonly updated_at: BlockNumber;
  readonly controllers: readonly AccountId[];
}


export type DriveIdPage = IdPage<DriveId>;
