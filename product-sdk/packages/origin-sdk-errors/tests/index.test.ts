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

import assert from "node:assert/strict";
import test from "node:test";
import { asSdkError, isSdkError, OriginSdkError, SDK_ERROR_MARKER } from "../src/index.ts";
import { sdkErrorFixture } from "../src/testing.ts";

test("SDK errors are structural, serializable and preserve causes", () => {
  const cause = new Error("offline");
  const error = new OriginSdkError({
    source: "chain-client",
    domain: "transport",
    code: "unavailable",
    message: "Endpoint unavailable",
    retryable: true,
    details: { endpoint: "wss://example.invalid" },
    cause,
  });
  assert.equal(error.cause, cause);
  assert.equal(error.marker, SDK_ERROR_MARKER);
  assert.ok(isSdkError(error));
  assert.ok(isSdkError(error.toJSON()));
  assert.equal(error.toJSON().retryable, true);
  assert.equal(asSdkError(error, { source: "x", domain: "x", code: "x" }), error);
  assert.equal(asSdkError(cause, { source: "host", domain: "transport", code: "unknown" }).message, "offline");
  assert.ok(isSdkError(sdkErrorFixture()));
});
