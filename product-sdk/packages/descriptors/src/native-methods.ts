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

import { NATIVE_ROUTE_CONTRACT } from "../generated/native-route-contract.ts";

export type NativeHostFinality = "finalized" | "submit-and-finalize";
export interface NativeHostMethodContract {
  readonly capability: "identity" | "attestation" | "names" | "storage" | "content" | "assets" | "transaction";
  readonly method: string;
  readonly finality: NativeHostFinality;
  readonly payloadFields: readonly string[];
}

/** Generated projection of the authoritative checked-in native route contract. */
export const NATIVE_HOST_METHODS = NATIVE_ROUTE_CONTRACT.routes.map((route) => ({
  capability: route.capability,
  method: route.method,
  finality: route.finality,
  payloadFields: route.parameters.map(({ name }) => name),
})) satisfies readonly NativeHostMethodContract[];

const identities = NATIVE_HOST_METHODS.map(({ capability, method }) => `${capability}:${method}`);
if (new Set(identities).size !== identities.length) throw new Error("duplicate native host method identity");
