# AGENTS.md

Guidance for AI coding agents working in this repository.

## Project overview

`boxer` is a small Rust CLI that wraps the Debian package build pipeline
(quilt patch checks, build-dependency installation via `mk-build-deps`, and
`dpkg-buildpackage`) into one command. It is a **user-facing tool**, not a
library: the crate compiles to a single binary named `boxer`.

Commands:

- `boxer check` — verify that quilt patches apply/unapply cleanly
- `boxer deps [package]` — generate & install the `<source>-build-deps` meta package
- `boxer build` — check → deps → `dpkg-buildpackage` → tarball `.deb`s → clean up

It must be run from the root of a Debian source tree (where `debian/control`
lives). See `README.md` for user-facing docs.

## Build & verify

```sh
cargo build                       # debug build
cargo build --release             # release binary
cargo check
cargo clippy --all-targets -- -D warnings   # lib.rs has #![warn(clippy::all)]
cargo fmt --check                 # rustfmt (no custom rustfmt.toml, keep defaults)
cargo test                        # run unit tests
```

There is **no CI configuration**. Most behavior shells out to external Debian
tools, so integration tests are impractical without a container; pure logic is
covered by inline `#[cfg(test)]` unit tests (`cargo test`) and new logic should
get unit tests in the relevant module.

## Repository layout

- `src/main.rs` — entry point: parse `Args`, build `Colors`, dispatch on `Op`
- `src/args.rs` — `Args` / `Op` / `ColorMode` via clap derive. All global flags
  (`-v`, `-q`, `-y`, `--color`) are defined here and apply to every subcommand.
- `src/lib.rs` — module declarations, `setup_logging(verbose, quiet)`
- `src/fmt.rs` — paru-style colour palette (`Colors`, including `quiet` mode) +
  `info`/`success`/`warning`/`error`/`detail` output helpers
- `src/exec.rs` — process helpers: `command`, `command_output`, `output`,
  `root_cmd` (cached via `OnceLock`: env var `BOXER_ROOT_CMD` > `root_cmd` in
  `$XDG_CONFIG_HOME/boxer/config.toml` > `sudo`), `is_root` (`euid == 0` fast
  path), `acquire_root` (up-front credential priming + Rust thread background
  refresh, `RootSession` guard), `sudo`, `run`, `run_sudo`
- `src/tools.rs` — `TOOL_PACKAGES` table + `ensure_installed` (`$PATH` probe
  via `which`, then `apt-get install` via the root command)
- `src/debian.rs` — source-tree validation (`ensure_source_tree`), parse
  `debian/changelog` (`get_source_info`), `debian/control`
  (`get_binary_packages`), and `.changes` (`get_changes_binaries`),
  generated-artifact selection (`BuildArtifacts`), predicate-based file
  collection (`collect_files`)
- `src/check.rs` — `run_check` (quilt `push`/`pop` with pre-pop and failure
  rollback, old-format `quilt upgrade`)
- `src/deps.rs` — `run_deps` (`mk-build-deps` + `apt-mark auto`)
- `src/build.rs` — `run_build` (full pipeline, `DEB_BUILD_OPTIONS` merging,
  direct `tar -C` tarball creation, cleanup)

## Code conventions

- **Edition 2024**, `#![warn(clippy::all)]` at the top of `lib.rs` — keep
  `clippy` clean (run with `-D warnings`).
- **Error handling**: `anyhow::Result` + `bail!` / `.with_context(...)`
  throughout. No panics, no custom error types.
- **Logging**: use `log` macros (`debug!`, ...) for diagnostic output — they
  are filtered by `setup_logging`. Do **not** call `println!`/`eprintln!`
  directly for progress; use the `fmt` helpers instead.
- **Coloring**: `Colors` is built once in `main` and threaded through as `&c`.
  Use `fmt::info` / `fmt::success` / `fmt::warning` / `fmt::error` /
  `fmt::detail` rather than raw ANSI sequences.
- **External commands**: always build `std::process::Command` and run it through
  the `exec` helpers. Never shell out via `sh -c`.
- **Global flags**: `--quiet` suppresses all non-error output; `verbose`
  levels map 0=Warn, 1=Info, 2=Debug, 3+=Trace, and `quiet` wins over
  `verbose`.
- **Keep it small**: dependency list is intentionally minimal (clap, anyhow,
  log, env_logger, ansiterm, which). Avoid adding heavy dependencies.
- **Commit messages** follow conventional style seen in git history:
  `feat:`, `fix(scope):`, `refactor:`, `docs:`.

## Important behaviors / gotchas

- `ensure_installed` probes `$PATH` for `quilt`, `mk-build-deps`,
  `equivs-build`, `dpkg-buildpackage`, `dpkg-parsechangelog` and offers `apt-get install`
  via the root command unless `--yes`. Note `dpkg-buildpackage` and
  `dpkg-parsechangelog` both map to the `dpkg-dev` package (deduped before apt).
- `build` and `deps` validate `ensure_source_tree` (`debian/control` and
  `debian/changelog`) first, then call `acquire_root` so a sudo password prompt
  never appears outside a valid Debian source tree or mid-build, and a
  background thread refreshes credentials until the command exits (skipped when
  already running as root).
- `check` only touches `debian/patches`; it skips gracefully when the directory
  or any patches are missing. Any already-applied patches are popped first,
  old-format quilt series triggers `quilt upgrade`, and a failed `quilt push -a`
  rolls back partially applied patches with `quilt pop -a`.
- `build` preserves existing `DEB_BUILD_OPTIONS` from the environment and
  appends `nocheck` when not already present.
- `build` cleans stale parent-directory artifacts of this source package before
  invoking `dpkg-buildpackage`, then collects generated `.deb`/`.udeb`/`.ddeb`
  from the **parent** directory (where `dpkg-buildpackage` drops them) belonging
  to the current build: `BuildArtifacts` matches the file-name version (dpkg
  strips the epoch from file names) against the changelog version and the
  package name against the binary packages in `debian/control`, any binaries
  recorded in the generated `.changes` file, plus their `-dbgsym` companions.
  Those files are archived directly from the parent directory via
  `tar --force-local -C <parent> -czf <source>-<version>.tar.gz` without moving
  debs into the source tree.
- `deps` marks the generated `<source>-build-deps` package as auto-removable
  via `apt-mark auto` so `apt autoremove` can clean it up later.
- `build` cleanup removes only this source package's parent-directory
  artifacts (any version): `.deb`/`.udeb`/`.ddeb` for its binary packages plus
  `.buildinfo`/`.changes`. Other packages' files in a shared parent
  directory are never deleted.
