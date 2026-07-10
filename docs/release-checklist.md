
## Notes

### Burn In

Ensure that DevOps has run the new release for at least 12 hours prior to publishing the release.

### Build Artifacts

Add any necessary assets to the release. They should include:

- Linux binary
- GPG signature of the Linux binary
- SHA256 of binary
- Source code
- Wasm binaries of any runtimes


### Release Environment Tooling

Before producing release artifacts, verify that the release runner has Docker and runtime
inspection tooling available:

```bash
docker info
subwasm --version
srtool --version
```

`subwasm` should be installed from the upstream repository, not the placeholder crates.io
package. `try-runtime-cli` is provided as the `try-runtime` binary and should be
installed from the upstream CLI repository tag used by SDK v1.24-era tooling:

```bash
cargo install --locked --git https://github.com/chevdor/subwasm
cargo install --locked srtool-cli
cargo install --locked --git https://github.com/paritytech/try-runtime-cli --tag v0.10.1
try-runtime --version
```

If the local shell exports `NO_COLOR=1`, run subwasm with `NO_COLOR` unset or set to a
boolean value because current subwasm expects `--no-color`/`NO_COLOR` to parse as
`true` or `false`:

```bash
env -u NO_COLOR subwasm --version
```

For CORD runtimes, pass the non-standard runtime directory explicitly when using srtool
locally, for example:

```bash
srtool build --engine docker --package cord-braid-runtime --runtime-dir runtimes/braid .
srtool build --engine docker --package cord-loom-runtime --runtime-dir runtimes/loom .
srtool build --engine docker --package cord-weave-runtime --runtime-dir runtimes/weave .
srtool build --engine docker --package origin-runtime --runtime-dir origin/base/runtime .
srtool build --engine docker --package origin-hub-system-runtime --runtime-dir origin/hub/system .
```

Attach the srtool JSON, compact/compressed Wasm artifacts, and subwasm `info`,
`meta`, and `diff` outputs to the release record.

### Try Runtime

CORD follows the SDK v1.24 try-runtime model: node binaries expose the
`try-runtime` Cargo feature and runtimes implement `frame_try_runtime::TryRuntime`;
execution is performed with the external `try-runtime-cli`, not a custom node
subcommand.

First verify that the native runtimes compile with try-runtime enabled:

```bash
cargo check -p cord-node-cli -p origin-node-cli -p origin-hub --locked --features try-runtime
```

For releases with migrations, run `try-runtime on-runtime-upgrade` against the
new runtime Wasm and representative live or snapshot state before publishing, for
example:

```bash
try-runtime --runtime <new-runtime-try-runtime.wasm> on-runtime-upgrade \
  --blocktime <blocktime-ms> live --uri <wss-or-ws-endpoint>

try-runtime --runtime <new-runtime-try-runtime.wasm> on-runtime-upgrade \
  --blocktime <blocktime-ms> snap --path <state-snapshot>
```

Keep the command output with the release artifacts.

### Release notes

The release notes should list:

- The priority of the release (i.e., how quickly users should upgrade) - this is based on the max priority of any *client* changes.
- Which native runtimes and their versions are included
- The proposal hashes of the runtimes as built with [srtool](https://gitlab.com/chevdor/srtool)

### Spec Version

A runtime upgrade must bump the spec number. This may follow a pattern with the client release (e.g. runtime v12 corresponds to v0.8.12, even if the current runtime is not v11).

### Old Migrations Removed

Any previous `on_runtime_upgrade` functions from old upgrades must be removed to prevent them from executing a second time. The `on_runtime_upgrade` function can be found in `runtime/<runtime>/src/lib.rs`.

### New Migrations

Ensure that any migrations that are required due to storage or logic changes are included in the `on_runtime_upgrade` function of the appropriate pallets.

### Extrinsic Ordering

Offline signing libraries depend on a consistent ordering of call indices and functions. Compare the metadata of the current and new runtimes and ensure that the `module index, call index` tuples map to the same set of functions. In case of a breaking change, increase `transaction_version`.

Note: Adding new functions to the runtime does not constitute a breaking change as long as the indexes did not change.
TODO: Automate this

### Benchmarks

The benchmarks should be updated before the release. The weights should be (Currently manually) checked to make sure there are no big outliers (i.e., twice or half the weight).

### SDK & API
Ensure that a release of [CORD SDK & API]() contains any new types or interfaces necessary to interact with the new runtime.
