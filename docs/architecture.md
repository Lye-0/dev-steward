# Architecture

## Boundaries

Dev Steward is a local desktop application. React renders the interface; Tauri invokes a typed Rust service. SQLite stores discovered state, plans and operation history. The application does not require a user-managed Node, Python, mise or uv process to start. The production UI is embedded in the executable. Windows still needs the OS WebView2 runtime.

`crates/steward-core` has no Tauri dependency and can be tested independently. `src-tauri` owns native window creation, application-data discovery and the IPC bridge. No production HTTP command server is exposed.

## Environment and path discovery

- A local environment and individually discovered WSL distributions have distinct identities.
- Home and application-data directories come from OS APIs. Tool locations come from PATH, known executable-directory hints, or explicit overrides.
- Installation roots come from managers, not an application-owned runtime directory. `uv python dir` is used to distinguish uv-managed Python from other Python discovered by uv.
- Aliases and Python launchers are grouped by distribution key, with the original paths retained as evidence. Remote relative paths are resolved against the environment's home.
- mise results from the environment home and results from a project directory have different meanings. The UI offers an explicit query of the effective tools in a project.
- No developer username, drive letter, clone root or WSL distribution is required by the implementation. The application identifier contains the publisher name; it is not a discovery rule.

## Static discovery versus execution

Git roots, manifests, locks and pyvenv.cfg are read without invoking project code. Git fsmonitor and hooks are disabled for inspection. Git diff uses `--no-ext-diff --no-textconv`. Known dependency and build directories are excluded from bounded walks. File reads are bounded to 8 MB on the local filesystem. WSL commands have time/output limits; large or unsupported files are reported rather than treated as successfully parsed.

Scripts, dependency synchronization, compilers and package build hooks can execute project or third-party code. These are separate, confirmed plan steps. MCP configuration inspection retains command paths and static virtual-environment metadata, never server arguments or environment values. The app does not automatically start MCP servers.

## Registry and vulnerability evidence

Direct npm, PyPI and Cargo dependencies are compared against public registries. pnpm/package-lock, uv.lock and Cargo.lock supply resolved versions for OSV batch queries. Local/VCS Python packages and non-public registry packages are not asserted to be public registry releases. Unsupported source formats remain incomplete.

Candidates preserve the major version; zero-major packages also preserve minor. Python runtime candidates preserve minor. This rule is a proposal filter, not compatibility proof. Registry resolution, local verification, CI, and application-specific connection tests are separate evidence. pnpm multi-document locks are parsed in full. Importer records connect a declaration to its exact resolved version. Fallback resolution filters by the declared constraint and refuses ambiguous versions rather than selecting another workspace's major version. OSV queries individual supported locked versions across all platforms; reachability on the current OS is a separate question.

## Plans and concurrency

Only backend-generated plans are persisted. The UI cannot submit an arbitrary executable and argument list. Each plan step records executable, arguments, working directory, risk, status and captured output. One state directory can be opened by one service process, enforced by an OS file lock. A process-local mutex serializes mutating steps.

Before a project step, the current HEAD, branch, status, diff and supported manifest contents must match the stored fingerprint. After each step the project is rescanned and the next expected fingerprint is stored. Registry candidates also carry the fingerprint from their scan. Changing the working tree requires reinspection/replanning.

The user confirms each next step. Failed/cancelled/interrupted plans cannot replay. Persisted running steps become interrupted after restart. Process output is drained concurrently and retained up to 2 MB per stream. Timeouts and cancellation terminate the process tree on Windows; Unix uses process groups. A failed external operation may have partial effects; neither rollback nor transactionality of external installers is promised.

Common credential patterns are removed from retained execution output, but arbitrary script output cannot be guaranteed secret-free. Do not run scripts that print secrets. The app never reads GitHub credentials; GitHub CLI owns authentication.

## Updates, publication and cleanup

Dependency changes start on a generated branch. Package install scripts are disabled for npm/pnpm update steps. uv sync and explicit tests may run build code. Publication stages dependency files, shows a diff, commits, pushes and creates a PR only if included in the confirmed plan. Local validation never writes to GitHub without such a plan.

A separate manual merge plan requires an open non-draft PR, a matching local/remote commit and successful checks. Checks are repeated immediately before merge and `--match-head-commit` guards races. No checks, skipped or failed checks block this path. Default branch synchronization uses the remote's current symbolic HEAD and fast-forward only. No hard reset, force push or automatic branch deletion is used.

Windows mise cleanup requires manager prunability, fresh scans of registered roots/projects/MCP configs, no unresolved declarations, no matching virtual-environment references, no direct PATH reference, and no running-process reference. Checks repeat before uninstall. Unknown/unregistered use is explicitly outside that proof. Python, WSL, OS and bundled-runtime deletion is not automated in this release.

Manager updates use WinGet if the exact package is registered there; otherwise official self-update commands are proposed and may refuse installer-managed installations. Failures do not trigger replacement by another manager.

## Storage and distribution

Default storage is the OS application-data directory for the application identifier. `--data-dir <absolute-directory>` selects a separate profile; tests use this to avoid changing real state. SQLite contains local paths and should be handled as private workspace data.

Rust is pinned through rust-toolchain.toml, Node/pnpm through mise.toml, and frontend dependencies through pnpm-lock.yaml. The application itself is built with Tauri's custom protocol for embedded assets. Its own distribution/update mechanism is independent from tool updates; automatic signed application updates are not configured before a publishing/signing process exists.
