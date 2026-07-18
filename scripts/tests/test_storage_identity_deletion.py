# This file is part of CORD – https://cord.network

# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later

# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

"""Hostile exact-surface tests for the storage/Identity deletion census."""

from __future__ import annotations

import copy
import importlib.util
import tempfile
import unittest
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib


CORD = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "deletion_validator", CORD / "scripts/validate-storage-identity-deletion.py"
)
assert SPEC and SPEC.loader
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)


class DeletionCensusTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = tomllib.loads(
            (CORD / "docs/specs/storage-identity-deletion-v1.toml").read_text()
        )

    def test_absent_declared_symbol_is_rejected(self) -> None:
        row = copy.deepcopy(self.manifest["item"][0])
        row["symbol"] = "definitely_absent_surface"
        self.assertFalse(VALIDATOR.exact_surface_exists(row, CORD))

    def test_post_cutover_manifest_has_no_pending_or_structural_drift(self) -> None:
        report = VALIDATOR.deletion_dag(self.manifest, CORD, None)
        for key in (
            "dag_missing_fields", "dag_duplicate_ids", "dag_duplicate_orders",
            "dag_invalid_items", "dag_invalid_edges", "dag_cycle_count",
            "absent_symbol_count", "duplicate_surface_count", "unmapped_surface_count",
            "declaration_only_surface_count",
        ):
            self.assertEqual(report[key], 0, key)
        self.assertEqual(report["pending_delete_item_count"], 0)
        self.assertGreater(report["deleted_item_count"], 0)
        self.assertGreater(report["replacement_active_item_count"], 0)

    def test_deleted_surface_cannot_silently_reappear(self) -> None:
        manifest = copy.deepcopy(self.manifest)
        row = next(item for item in manifest["item"] if item["status"] == "replacement-active")
        row["status"] = "deleted"
        report = VALIDATOR.deletion_dag(manifest, CORD, None)
        self.assertIn(row["id"], report["dag_details"]["invalid_items"])
        self.assertEqual(report["unmapped_surface_count"], 1)

    def test_provider_census_excludes_only_canonical_control_reads(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._copy_census_fixture(root)
            api = "origin/orbis/provider-node/src/api.rs"
            self._append(root, api, '(Method::GET, "/info"),\n(Method::POST, "/commit"),\n')
            surfaces = VALIDATOR.current_surface_census(root)
            self.assertNotIn((api, "provider-route", "GET /health"), surfaces)
            self.assertNotIn((api, "provider-route", "GET /info"), surfaces)
            self.assertIn((api, "provider-route", "POST /commit"), surfaces)

    def test_rust_export_excludes_internal_identity_routes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = "origin-rs/src/product_sdk/transport.rs"
            source = root / relative
            source.parent.mkdir(parents=True)
            source.write_text(
                """
pub(crate) trait InternalIdentityReadBinding {
    async fn identity_personhood(&self) {}
}
pub(crate) fn prepare_identity_personhood_command() {}
""",
                encoding="utf-8",
            )
            row = {
                "path": relative,
                "surface_locator": "rust-export",
                "symbol": "rust-export::identity_personhood",
            }
            self.assertFalse(VALIDATOR.exact_surface_exists(row, root))
            row["symbol"] = "rust-export::prepare_identity_personhood_command"
            self.assertFalse(VALIDATOR.exact_surface_exists(row, root))

            source.write_text(
                """
pub trait FinalizedReadBinding {
    async fn identity_personhood(&self) {}
}
pub fn prepare_identity_personhood_command() {}
""",
                encoding="utf-8",
            )
            row["symbol"] = "rust-export::identity_personhood"
            self.assertTrue(VALIDATOR.exact_surface_exists(row, root))
            row["symbol"] = "rust-export::prepare_identity_personhood_command"
            self.assertTrue(VALIDATOR.exact_surface_exists(row, root))

    def test_unmapped_new_dispatchable_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._copy_census_fixture(root)
            pallet = root / "origin/orbis/pallets/transaction-storage/src/lib.rs"
            with pallet.open("a") as output:
                output.write("\n#[pallet::call_index(99)]\npub fn newly_added() {}\n")
            surfaces = VALIDATOR.current_specialized_surfaces(root)
            self.assertIn(
                (pallet.relative_to(root).as_posix(), "pallet-call", "Pallet::call[99]::newly_added"),
                surfaces,
            )

    def _copy_census_fixture(self, root: Path) -> None:
        fixtures = {
            "origin/orbis/pallets/transaction-storage/Cargo.toml":
                '[package]\nname = "pallet-orbis-transaction-storage"\nversion = "0.1.0"\n',
            "origin/orbis/pallets/transaction-storage/src/lib.rs": """
#[pallet::call_index(0)]
pub fn store() {}
pub enum Event<T: Config> {
    Stored,
}
#[pallet::storage]
pub type Transactions<T> = ();
""",
            "origin/orbis/pallets/transaction-storage/src/extension.rs":
                "pub struct ValidateStorageCalls;\n",
            "origin/orbis/pallets/hop-promotion/Cargo.toml":
                '[package]\nname = "pallet-orbis-hop-promotion"\nversion = "0.1.0"\n',
            "origin/orbis/node/src/proof_campaign/config.rs": "pub fn run() {}\n",
            "origin/orbis/provider-node/src/api.rs": '(Method::GET, "/health"),\n',
            "origin/orbis/runtime/src/lib.rs": """
TransactionStorage: pallet_orbis_transaction_storage = 110,
HopPromotion: pallet_orbis_hop_promotion = 111,
impl sp_transaction_storage_proof::runtime_api::TransactionStorageApi<Block> for Runtime {
    fn proof() {}
}
type LongTermStorageDataStore = TransactionStorage;
pallet_orbis_transaction_storage::extension::ValidateStorageCalls<Runtime>
""",
            "product-sdk/packages/descriptors/generated/incumbent.json": "{}\n",
            "product-sdk/packages/descriptors/src/identity-host-routes.ts": """
export const personhoodHostRoutes = {
  read() {},
};
""",
            "product-sdk/packages/origin-sdk-host/src/protocol.ts":
                '  "resources.allocate": {},\n',
            "product-sdk/packages/origin-sdk-personhood/src/index.ts":
                "export interface PersonhoodClient {}\n",
            "product-sdk/packages/origin-sdk-resources/src/index.ts":
                "export interface ResourcesClient {}\n",
            "origin-rs/src/product_sdk/transport.rs":
                "pub async fn read_personhood() {}\npub async fn prepare_storage_command() {}\n",
        }
        for relative, content in fixtures.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")

    @staticmethod
    def _append(root: Path, relative: str, text: str) -> None:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as output:
            output.write(text)

    def _inject_surface(self, root: Path, category: str) -> tuple[str, str, str]:
        pallet = "origin/orbis/pallets/transaction-storage/src/lib.rs"
        runtime = "origin/orbis/runtime/src/lib.rs"
        if category == "pallet-call":
            self._append(root, pallet, "\n#[pallet::call_index(99)]\npub fn hostile_call() {}\n")
            return pallet, category, "Pallet::call[99]::hostile_call"
        if category == "pallet-event":
            path = root / pallet
            text = path.read_text()
            marker = text.index("{", text.index("pub enum Event<T: Config>")) + 1
            path.write_text(text[:marker] + "\nHostileEvent,\n" + text[marker:])
            return pallet, category, "Event::HostileEvent"
        if category == "pallet-storage":
            self._append(root, pallet, "\n#[pallet::storage]\npub type HostileStore<T> = ();\n")
            return pallet, category, "Storage::HostileStore"
        if category == "runtime-pallet-index":
            self._append(root, runtime, "\nHostileIgnored: pallet_hostile = 199,\n")
            # The independent policy census intentionally restricts this class to incumbent names.
            path = root / runtime
            text = path.read_text().replace(
                "TransactionStorage: pallet_orbis_transaction_storage = 110,",
                "TransactionStorage: pallet_orbis_transaction_storage = 110,\n"
                "TransactionStorage: pallet_hostile = 199,",
            )
            path.write_text(text)
            return runtime, category, "TransactionStorage: pallet_hostile = 199"
        if category == "runtime-api-method":
            path = root / runtime
            text = path.read_text()
            marker = text.index(
                "{", text.index("impl sp_transaction_storage_proof::runtime_api::TransactionStorageApi")
            ) + 1
            path.write_text(text[:marker] + "\nfn hostile_api() {}\n" + text[marker:])
            trait = "sp_transaction_storage_proof::runtime_api::TransactionStorageApi"
            return runtime, category, f"{trait}::hostile_api"
        if category == "runtime-api":
            path = root / runtime
            text = path.read_text().replace(
                "impl sp_transaction_storage_proof::runtime_api::TransactionStorageApi<Block>",
                "impl sp_transaction_storage_proof::runtime_api::TransactionStorageApiHostile<Block>",
                1,
            )
            path.write_text(text)
            return runtime, category, "sp_transaction_storage_proof::runtime_api::TransactionStorageApiHostile"
        if category == "signed-extension":
            relative = "origin/orbis/pallets/transaction-storage/src/extension.rs"
            self._append(root, relative, "\npub struct HostileExtension;\n")
            return relative, category, "HostileExtension"
        if category == "runtime-config":
            self._append(root, runtime, "\ntype LongTermStorageDataStoreHostile = TransactionStorage;\n")
            return runtime, category, "LongTermStorageDataStoreHostile = TransactionStorage"
        if category == "node-service":
            relative = "origin/orbis/node/src/proof_campaign/hostile.rs"
            self._append(root, relative, "pub fn hostile_service() {}\n")
            return relative, category, "hostile_service"
        if category == "provider-route":
            relative = "origin/orbis/provider-node/src/api.rs"
            self._append(root, relative, '\n(Method::GET, "/hostile"),\n')
            return relative, category, "GET /hostile"
        if category == "generated-descriptor":
            relative = "product-sdk/packages/descriptors/generated/hostile.json"
            self._append(root, relative, "{}\n")
            return relative, category, f"file:{relative}"
        if category == "public-package":
            relative = "product-sdk/hostile-taxonomy.ts"
            self._append(root, relative, "export const old = 'PeopleLite';\n")
            return relative, category, "PeopleLite"
        if category == "obsolete-reference":
            relative = "origin/hostile-reference.rs"
            self._append(root, relative, "type Old = TransactionStorage;\n")
            return relative, category, "TransactionStorage"
        if category == "obsolete-file":
            relative = "origin/orbis/pallets/transaction-storage/src/hostile.rs"
            self._append(root, relative, "// incumbent\n")
            return relative, category, f"file:{relative}"
        if category == "cargo-crate":
            relative = "origin/orbis/pallets/transaction-storage/hostile/Cargo.toml"
            self._append(root, relative, '[package]\nname="hostile-storage-crate"\nversion="0.1.0"\n')
            return relative, category, "hostile-storage-crate"
        if category == "public-route":
            relative = "product-sdk/packages/origin-sdk-personhood/src/index.ts"
            self._append(root, relative, "\nexport interface HostilePublicRoute {}\n")
            return relative, category, "export::HostilePublicRoute"
        if category == "host-operation":
            relative = "product-sdk/packages/origin-sdk-host/src/protocol.ts"
            self._append(root, relative, '\n  "resources.hostile": {}\n')
            return relative, category, "resources.hostile"
        if category in {"fixture", "contract-fixture"}:
            suffix = ".json" if category == "fixture" else ".sol"
            relative = f"product-sdk/hostile/fixtures/hostile{suffix}"
            self._append(root, relative, "preimage retention\n")
            return relative, category, f"file:{relative}"
        raise AssertionError(f"missing hostile category {category}")

    def test_unmapped_new_surface_for_every_independent_category(self) -> None:
        categories = (
            "pallet-call", "pallet-event", "pallet-storage", "runtime-pallet-index",
            "runtime-api", "runtime-api-method", "signed-extension", "runtime-config",
            "node-service", "provider-route", "generated-descriptor", "public-package",
            "obsolete-reference", "obsolete-file", "cargo-crate", "public-route",
            "host-operation", "fixture", "contract-fixture",
        )
        for category in categories:
            with self.subTest(category=category), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                self._copy_census_fixture(root)
                before = VALIDATOR.current_surface_census(root)
                expected = self._inject_surface(root, category)
                after = VALIDATOR.current_surface_census(root)
                self.assertIn(expected, after - before)

    def test_duplicate_finding_ownership_is_rejected(self) -> None:
        finding = {"item_id": "obsolete", "path": "path.rs"}
        manifest = {"item": [
            {"id": "one", "path": "path.rs", "owns_obsolete": ["obsolete"]},
            {"id": "two", "path": "path.rs", "owns_obsolete": ["obsolete"]},
        ]}
        report = VALIDATOR.ownership_audit(manifest, [finding])
        self.assertEqual(report["duplicate_owner_count"], 1)

    def test_exclusion_tracks_adr_acceptance(self) -> None:
        accepted = VALIDATOR.deletion_dag(
            self.manifest,
            CORD,
            CORD / "docs/specs/web3-storage-capability-ledger-v1.json",
        )
        self.assertEqual(accepted["approved_exclusion_invalid"], 0)

        proposed = copy.deepcopy(self.manifest)
        with tempfile.NamedTemporaryFile(mode="w", suffix=".md") as adr:
            adr.write(
                "## Status\n\nProposed.\n\n"
                "### no-parallel-storage-chain\n\nPending.\n\n"
                "### no-duplicate-contract-state\n\nPending.\n"
            )
            adr.flush()
            for reference in proposed["decision_reference"]:
                reference["adr_path"] = adr.name
            rejected = VALIDATOR.deletion_dag(
                proposed,
                CORD,
                CORD / "docs/specs/web3-storage-capability-ledger-v1.json",
            )
        self.assertEqual(rejected["approved_exclusion_invalid"], 2)


if __name__ == "__main__":
    unittest.main()
