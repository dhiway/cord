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
import { andThen, err, isErr, isOk, map, mapError, ok, unwrapOr } from "../src/index.ts";

test("Result helpers preserve discriminants and values", () => {
  const value = ok(4);
  assert.ok(isOk(value));
  assert.deepEqual(map(value, (item) => item * 2), ok(8));
  assert.deepEqual(andThen(value, (item) => ok(String(item))), ok("4"));
  assert.equal(unwrapOr(value, 0), 4);

  const failure = err("denied");
  assert.ok(isErr(failure));
  assert.deepEqual(map(failure, (item: number) => item * 2), failure);
  assert.deepEqual(mapError(failure, (error) => ({ code: error })), err({ code: "denied" }));
  assert.equal(unwrapOr(failure, 9), 9);
});
