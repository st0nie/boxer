# boxer

A small CLI that wraps the Debian package build pipeline — quilt patch checks, build-dependency installation, and `dpkg-buildpackage` — into one command.

## Features

- **`boxer check`** — verify quilt patches apply and unapply cleanly
- **`boxer deps`** — generate and install the build-dependency meta package via `mk-build-deps`
- **`boxer build`** — run check + deps, build binary packages, archive the current version's `.deb`s into a tarball, then clean up
- **Auto-install missing tools** — probes `$PATH` for required binaries and offers to `apt-get install` them
- **Configurable privilege escalation** — `sudo` by default, overridable (e.g. `keyi`); credentials are primed up front so a password prompt never interrupts a build
- Colored output, configurable verbosity, and a `--yes` flag for non-interactive runs

## Installation

```sh
cargo install --git https://github.com/st0nie/boxer.git
```

Or build from source:

```sh
git clone https://github.com/st0nie/boxer.git
cd boxer
cargo build --release
install -Dm755 target/release/boxer ~/.local/bin/boxer
```

## Usage

Run from the root of a Debian source tree (where `debian/control` lives).

### `boxer check`

Tests that all patches under `debian/patches` apply with `quilt push -a` and unapply cleanly with `quilt pop -a`. Handles old-format quilt series by running `quilt upgrade` automatically.

```sh
boxer check
```

### `boxer deps`

Uses `mk-build-deps` to build and install a `<source>-build-deps` meta package, then marks it as auto-removable so `apt autoremove` can clean it up later.

```sh
boxer deps                  # uses source name from debian/changelog
boxer deps my-package       # override the target package name
```

### `boxer build`

Full pipeline: `check` → `deps` → `dpkg-buildpackage -b -uc -us` (with `nocheck` merged into `DEB_BUILD_OPTIONS`) → archive the `.deb`/`.udeb`/`.ddeb` files generated for the current source version (including `-dbgsym` debug packages) directly from the parent directory into `<source>-<version>.tar.gz` → remove this source package's leftover build artifacts from the parent directory.

Only artifacts belonging to the current source package are touched: stale `.deb`s from other versions are not added to the tarball, and files belonging to other packages in the parent directory are left alone. Cleanup removes this source package's artifacts of any version — the binary packages declared in `debian/control` (and recorded in `.changes`), their `-dbgsym` companions, and the source's `.buildinfo`/`.changes` files.
```sh
boxer build
```

## Flags

| Flag | Description |
|------|-------------|
| `-v` / `--verbose` | Increase verbosity (repeatable: `-v`, `-vv`, `-vvv`) |
| `-q` / `--quiet` | Suppress all non-error output |
| `-y` / `--yes` | Skip interactive prompts (auto-confirm tool installation) |
| `--color <mode>` | Color output: `auto` (default), `always`, or `never` |

All flags are global and can be placed before or after the subcommand.

## Auto-install

Before running, boxer checks that the required external tools are on `$PATH`:

| Binary | Debian package | Used by |
|--------|----------------|---------|
| `quilt` | `quilt` | check, build |
| `mk-build-deps` | `devscripts` | deps, build |
| `equivs-build` | `equivs` | deps, build |
| `dpkg-buildpackage` | `dpkg-dev` | build |
| `dpkg-parsechangelog` | `dpkg-dev` | deps, build |

If any are missing, boxer lists them and asks for confirmation before running `apt-get install -y` via the configured root command. Use `--yes` to skip the prompt.

## Configuration

Privileged commands run under a root command, resolved in this order:

1. `BOXER_ROOT_CMD` environment variable
2. `root_cmd` in `$XDG_CONFIG_HOME/boxer/config.toml` (default `~/.config/boxer/config.toml`):

   ```toml
   root_cmd = "keyi"
   ```

3. `sudo`

Before `boxer build` or `boxer deps` touches anything privileged, boxer primes the credentials (`root_cmd true`) so any password prompt happens at the very start of the run, and refreshes them in the background until the command finishes.

## License

MIT
