---
title: Installation
description: Install the ought CLI on macOS, Linux, or Windows.
order: 2
---

Ought ships as a single static binary. Pick whichever installer matches your environment.

## Shell installer

The shell installer detects your platform and architecture and downloads the right binary.

```sh
curl -sS https://raw.githubusercontent.com/soseinai/ought/main/install.sh | sh
```

This installs `ought` into `~/.local/bin`. Add that directory to your `PATH` if it isn't already.
Pin a version with `OUGHT_VERSION=v0.1.0` or change the install location with `OUGHT_INSTALL_DIR=/usr/local/bin`.

## Python (pipx)

If your environment is already Python-centric, install via [pipx](https://pipx.pypa.io/):

```sh
pipx install ought
```

This puts `ought` on your `PATH` in its own isolated venv. Plain `pip install ought` works too if you're inside a virtualenv.

## Cargo

If you have a Rust toolchain installed, you can build from source via crates.io:

```sh
cargo install ought
```

## Homebrew

On macOS or Linux with Homebrew:

```sh
brew install soseinai/tap/ought
```

## Verifying the install

Run the version command to confirm everything is wired up:

```sh
ought --version
```

You should see the installed version printed back.

## Updating

Re-run whichever installer you used originally. Each installer is idempotent and replaces the existing binary in place.
