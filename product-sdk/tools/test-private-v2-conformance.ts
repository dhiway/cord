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

import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const sdkRoot = resolve(import.meta.dirname, "..");
const repositoryRoot = resolve(sdkRoot, "..");

interface Trace {
  readonly name: string;
  readonly command: string;
  readonly args: readonly string[];
  readonly cwd: string;
}

const nodeTrace = (name: string, test: string): Trace => ({
  name,
  command: process.execPath,
  args: ["--experimental-strip-types", "--test", test],
  cwd: sdkRoot,
});

const traces: readonly Trace[] = [
  nodeTrace("storage", "packages/origin-sdk-cloud-storage/tests/storage-v2-intents.test.ts"),
  nodeTrace("identity", "packages/origin-sdk-identity/tests/v2.test.ts"),
  nodeTrace("host", "packages/origin-sdk-host/tests/host-v2-internal.test.ts"),
  nodeTrace("browser", "packages/origin-sdk-host/tests/host-v2-browser-internal.test.ts"),
  nodeTrace("descriptor", "packages/origin-sdk-descriptors/tests/host-v2-descriptor.test.ts"),
  {
    name: "desktop",
    command: "cargo",
    args: [
      "test",
      "--manifest-path",
      "origin-rs/Cargo.toml",
      "--lib",
      "product_sdk::host_v2::tests::desktop_frames_enforce_big_endian_cap_and_split_coalesced_streams",
      "--",
      "--exact",
    ],
    cwd: repositoryRoot,
  },
];

for (const trace of traces) {
  process.stdout.write(`TRACE ${trace.name}: ${trace.command} ${trace.args.join(" ")}\n`);
  const result = spawnSync(trace.command, trace.args, {
    cwd: trace.cwd,
    encoding: "utf8",
    stdio: "inherit",
  });
  if (result.error !== undefined) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${trace.name} private-v2 conformance trace failed with status ${result.status ?? "unknown"}`);
  }
}

process.stdout.write(`PASS private-v2 source conformance: traces=${traces.length}\n`);
