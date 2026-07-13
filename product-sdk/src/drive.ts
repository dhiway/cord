import { invalidDomainInput } from "./errors.ts";
import { finalizedRead, submitAndFinalize, type RequestContext } from "./host.ts";
import {
  page,
  type AccountId,
  type BlockNumber,
  type ContentHash,
  type DecimalU64,
  type DriveId,
  type IdPage,
  type PageInput,
} from "./types.ts";

declare const driveType: unique symbol;
export type DriveName = string & { readonly [driveType]: "DriveName" };
export type DriveStatus = "active" | "archived";

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
  readonly root_storage_ref: ContentHash | null;
  readonly version: DecimalU64;
  readonly status: DriveStatus;
  readonly created_at: BlockNumber;
  readonly updated_at: BlockNumber;
}

export const drive = {
  driveById(context: RequestContext, drive_id: DriveId) {
    return finalizedRead("drive", context, "storage", "drive_by_id", { drive_id });
  },

  ownerDrives(context: RequestContext, owner: AccountId, input?: PageInput) {
    return finalizedRead("drive", context, "storage", "owner_drives", { owner, ...page(input) });
  },

  driveControllers(context: RequestContext, drive_id: DriveId, input?: PageInput) {
    return finalizedRead("drive", context, "storage", "drive_controllers", {
      drive_id,
      ...page(input),
    });
  },

  nextDriveNonce(context: RequestContext, owner: AccountId) {
    return finalizedRead("drive", context, "storage", "next_drive_nonce", { owner });
  },

  createDrive(context: RequestContext, name: DriveName, root_storage_ref: ContentHash | null) {
    return submitAndFinalize("drive", context, "storage", "create_drive", { name, root_storage_ref });
  },

  updateRoot(
    context: RequestContext,
    drive_id: DriveId,
    expected_version: DecimalU64,
    root_storage_ref: ContentHash | null,
  ) {
    return submitAndFinalize("drive", context, "storage", "update_root", {
      drive_id,
      expected_version,
      root_storage_ref,
    });
  },

  setController(
    context: RequestContext,
    drive_id: DriveId,
    controller: AccountId,
    enabled: boolean,
  ) {
    return submitAndFinalize("drive", context, "storage", "drive.set_controller", {
      drive_id,
      controller,
      enabled,
    });
  },

  transferDrive(context: RequestContext, drive_id: DriveId, new_owner: AccountId) {
    return submitAndFinalize("drive", context, "storage", "transfer_drive", { drive_id, new_owner });
  },

  archiveDrive(context: RequestContext, drive_id: DriveId) {
    return submitAndFinalize("drive", context, "storage", "archive_drive", { drive_id });
  },
} as const;

export type DriveIdPage = IdPage<DriveId>;
