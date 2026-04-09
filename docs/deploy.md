# Ought — Release & Deploy Guide

How to cut a release of `ought`, what gets shipped where, and how the
automation works under the hood.

---

## TL;DR — How to release

1. Go to **[Actions → Release → Run workflow](https://github.com/soseinai/ought/actions/workflows/release.yml)**
2. **Branch**: leave as `main`
3. **New semver**: type the version (e.g. `0.2.0`, or `0.2.0-rc1` for a release candidate)
4. Click **Run workflow**

That's it. Wait ~5–10 minutes and the release is live across all distribution
channels. You'll see **two workflow runs** in the Actions tab — that's normal,
each does half the work (see [Architecture](#architecture)).

---

## What you get from a release

A single click produces all of the following automatically:

| Channel | Install command | What it ships |
|---|---|---|
| **GitHub Releases** | _direct download_ | Prebuilt `.tar.gz` for 4 targets: x86_64-linux-gnu, aarch64-linux-gnu, x86_64-darwin, aarch64-darwin |
| **Shell installer** | `curl -sS https://raw.githubusercontent.com/soseinai/ought/main/install.sh \| sh` | Downloads the right binary for the user's OS+arch from GitHub Releases |
| **Homebrew tap** | `brew install soseinai/tap/ought` | Downloads the prebuilt binary for the user's OS+arch from GitHub Releases via the formula in [`soseinai/homebrew-tap`](https://github.com/soseinai/homebrew-tap), auto-bumped on each release. Installs in seconds; no Rust toolchain required |
| **crates.io** | `cargo install ought` | Source distribution; user compiles locally. All 8 workspace crates are published |

The `ought` binary itself is the same in every case. The differences are
just how it gets onto the user's machine.

---

## Pre-flight checklist

Before you click Run workflow, verify:

- [ ] **`main` is green.** Check the latest CI run on `main` — if it's red, fix
      that first. The release workflow will NOT re-run CI before tagging.
- [ ] **You've decided on a version number.** Use semver. For untested changes,
      consider an `rcN` release candidate first (see below).
- [ ] **All PRs you intended for this release are merged.** Anything still in
      draft or open will not make it.
- [ ] **You're an org admin (or the workflow has the right permissions).** The
      `Run workflow` button is gated by repo permissions.

### When to do a release candidate first

A release candidate (`0.X.Y-rc1`) is the cheap insurance policy for changes that
touch the release pipeline itself or the binary's runtime behavior in ways CI
can't fully test. Examples:

- You changed `Cargo.toml`'s workspace dependencies
- You added a new platform target
- You modified `release.yml` itself
- You bumped a major dependency
- You added or changed a CLI subcommand and want to verify it works on a
  freshly-installed binary

`rc1` exercises the entire pipeline end-to-end at zero risk: the rc tag is
publicly an "obviously a test" version, crates.io accepts pre-release versions
and won't pick them up in `cargo install` by default, and if anything breaks
you fix it and try `rc2` (or skip ahead to the stable version).

For pure bug fixes or doc updates, you can usually skip straight to the stable
version.

---

## Architecture

### One workflow, two trigger paths

Everything lives in a single file: **`.github/workflows/release.yml`**. It has
two trigger paths that together form one logical release operation:

```
                ┌──────────────────────┐
                │   You click "Run"    │
                │  with version 0.2.0  │
                └──────────┬───────────┘
                           │
              ╔════════════▼══════════════╗
              ║  Run #1: workflow_dispatch ║
              ╠════════════════════════════╣
              ║  bump-and-tag              ║   ← runs only on workflow_dispatch
              ║  ─ cargo set-version       ║
              ║  ─ git commit + tag        ║
              ║  ─ git push (main + tag)   ║
              ╚════════════╤══════════════╝
                           │
                  pushes v0.2.0 tag
                           │
              ╔════════════▼══════════════╗
              ║  Run #2: tag push           ║   ← triggered automatically
              ╠════════════════════════════╣
              ║  build (4-target matrix)   ║   ─┐
              ║  release                    ║    │  all four
              ║  publish-crates             ║    │  run only
              ║  bump-homebrew              ║   ─┘  on tag refs
              ╚════════════════════════════╝
```

Each release results in **two workflow runs** in the Actions tab, both labeled
"Release". This is intentional: GitHub Actions creates a separate run per
trigger event, and the conditional gating (`if: github.event_name == ...`)
ensures each run executes only its half.

### Job-level conditionals

| Job | `if:` condition | Purpose |
|---|---|---|
| `bump-and-tag` | `github.event_name == 'workflow_dispatch'` | Manual entry point — runs only on Run #1 |
| `build` | `startsWith(github.ref, 'refs/tags/v')` | Only runs on tag pushes (Run #2) |
| `release` | `startsWith(github.ref, 'refs/tags/v')` | Same |
| `publish-crates` | `startsWith(github.ref, 'refs/tags/v')` | Same |
| `bump-homebrew` | `startsWith(github.ref, 'refs/tags/v')` | Same |

So in Run #1 you'll see `bump-and-tag` succeed and the other 4 jobs marked
"this job was skipped". In Run #2 it's the reverse: the 4 release jobs succeed
and `bump-and-tag` is skipped. Both are correct.

### What each job does

**`bump-and-tag`** (Run #1, ~1m40s)

1. Validates the version input is semver
2. Mints an installation token from `sosein-release-bot` scoped to `soseinai/ought`
3. Checks out `main` with the bot's token
4. Verifies the tag doesn't already exist on origin
5. Installs `cargo-edit`
6. Runs `cargo set-version --workspace <version>` (updates `Cargo.toml` and `Cargo.lock`)
7. Commits as `sosein-release-bot[bot]`, tags `vX.Y.Z`, pushes both to `main`

The push of the tag triggers Run #2 automatically.

**`build`** (Run #2, parallel matrix, ~1–2 min per target)

For each of 4 targets (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
`x86_64-apple-darwin`, `aarch64-apple-darwin`):

1. Checks out the tag
2. Sets up Rust toolchain with the cross target
3. Sets up Node.js + `just`
4. Installs `gcc-aarch64-linux-gnu` if cross-compiling for aarch64-linux
5. Builds the UI (`just build-ui`)
6. Builds the release binary (`cargo build --release --target X -p ought`)
7. Tar+gzips the binary into `ought-<target>.tar.gz`
8. Uploads as a workflow artifact

**`release`** (Run #2, ~10s)

1. Downloads all 4 build artifacts
2. Creates a GitHub Release at the tag with auto-generated release notes
3. Attaches all 4 tarballs as release assets

**`publish-crates`** (Run #2, ~1m15s)

1. Checks out the tag
2. Sets up Rust + Node.js + `just`
3. Runs `just publish-crates` which:
   - Builds the UI (so `ought-server`'s `dist/` exists for `rust-embed`)
   - Runs `cargo publish --workspace --allow-dirty` — cargo handles dependency
     ordering and index propagation between dependent publishes automatically
4. All 8 crates land on crates.io

`--allow-dirty` is required because `ought-server` ships its built UI assets
via `[package] include = ["dist/**/*"]`, and those files are gitignored. See
[`crates/ought-server/Cargo.toml`](../crates/ought-server/Cargo.toml).

**`bump-homebrew`** (Run #2, ~10s)

1. Mints a token scoped to `soseinai/homebrew-tap` (NOT `soseinai/ought`)
2. Checks out `soseinai/homebrew-tap` (the separate tap repo)
3. Downloads each of the 4 prebuilt binary tarballs from this release's GitHub
   Release (with retries — release assets can take a moment to be downloadable
   after the `release` job finishes)
4. Computes a SHA256 for each platform's binary
5. **Generates `Formula/ought.rb` from a heredoc template** in the workflow,
   substituting the version, base URL, and 4 SHAs. The formula uses
   `on_macos`/`on_linux` × `on_arm`/`on_intel` blocks so brew picks the right
   binary for the user's platform
6. Commits and pushes to the tap repo's `main` as `sosein-release-bot[bot]`
7. The push uses `[skip ci]` to avoid triggering downstream CI on the tap repo

After this, `brew install soseinai/tap/ought` downloads the right prebuilt
binary for the user's platform — install completes in seconds. There's no
build step on the user's machine and no Rust toolchain dependency.

**Why generate from a template instead of `sed`-patching?** The formula has
8 fields that change per release (4 url + 4 sha256). Sed-patching that many
lines is fragile — one whitespace difference and it silently misses an update.
Generating fresh from a heredoc each release means the workflow file is the
single source of truth for the formula's structure: edit the template in
`release.yml` and the next release picks it up.

---

## Branch protection & the release bot

`main` is protected by a GitHub Ruleset (`main-protection`) with these rules:

- **Require pull request before merging** (0 approvals — solo dev friendly)
- **Require status check `test` to pass**
- **No force pushes**
- **No branch deletion**

**No human can push directly to `main`** — including org admins. All
human-initiated changes go through PRs.

The exception is the **`sosein-release-bot` GitHub App**, which is in the
ruleset's `bypass_actors` list (actor type `Integration`, app ID `3199403`).
The bot's installation token can:

- Push the version-bump commit to `main` from the `bump-and-tag` job
- Push tags to `main`
- Push the formula bump to `soseinai/homebrew-tap`'s `main`

This is the only way around branch protection in this repo. Direct pushes from
a personal access token would also work if the user were a bypass actor — but
no human is, so PATs don't help.

The bot's installation has `repository_selection: all` at the org level, so
it automatically has access to any new repo in `soseinai/` (including the
homebrew-tap repo we created later).

---

## Required secrets

All stored as **organization secrets** on `soseinai`, accessible to all repos:

| Secret | Used by | Purpose |
|---|---|---|
| `APP_ID` | `bump-and-tag`, `bump-homebrew` | The numeric GitHub App ID for `sosein-release-bot` (3199403) |
| `APP_PRIVATE_KEY` | `bump-and-tag`, `bump-homebrew` | The PEM-encoded private key for the app, used by `actions/create-github-app-token@v1` to mint installation tokens |
| `CARGO_REGISTRY_TOKEN` | `publish-crates` | crates.io API token with `publish-update` scope for the 8 ought crates |

The standard `GITHUB_TOKEN` (auto-provided per workflow run) is used by
`softprops/action-gh-release@v2` to create the GitHub release. It does NOT
have permission to push to `main` (because the ruleset's bypass list contains
only `sosein-release-bot`, not `github-actions`), which is why we use the
bot's token for those operations.

---

## Failure modes & recovery

### `bump-and-tag` fails

Common causes and fixes:

| Symptom | Cause | Fix |
|---|---|---|
| `Error: version must be semver format` | Typo in the version input | Re-trigger with a corrected version |
| `Error: tag v0.2.0 already exists on origin` | You already released this version | Bump to the next version, OR delete the tag if you really need to re-release: `gh api -X DELETE repos/soseinai/ought/git/refs/tags/v0.2.0` (rare; usually wrong) |
| `cargo install cargo-edit` fails | Transient network issue | Re-run the workflow |
| `git push origin main` fails with `non-fast-forward` | Someone else pushed to main between checkout and push | Re-run the workflow |
| Bot token mint fails | `APP_ID` or `APP_PRIVATE_KEY` secret is missing/wrong | Verify the org secrets exist; check the app's private key hasn't been rotated |

If `bump-and-tag` fails midway (e.g. commit succeeded but `git push` failed),
you may have a local commit on the runner that was never pushed. Since the
runner is ephemeral, this is fine — just re-run the workflow with the same
version. The "Verify tag does not already exist" check will pass because the
tag was never pushed either.

### `build` fails

The most common cause is the **`aarch64-unknown-linux-gnu`** cross-compile.
If your dependencies start needing a C library that isn't installed, this
target will fail with a linker error.

To fix:

1. Check the failed job's logs for the missing library
2. Add it to the `Install cross-compilation tools` step in `release.yml` —
   this step only runs for the aarch64-linux target

The other 3 targets (x64-linux, x64-darwin, arm64-darwin) build natively on
their respective runners and almost never have cross-compile issues.

If `build` fails on **all targets**, the cause is usually a Rust toolchain
issue or a `cargo.toml` mistake. Reproduce locally with `cargo build --release`.

### `release` fails

Almost always a transient GitHub Actions / GitHub API issue. **Re-run the
job** (not the whole workflow — just this job, via the Actions UI). The
`build` artifacts will still be available from the prior attempt.

### `publish-crates` fails

| Symptom | Cause | Fix |
|---|---|---|
| `error: no matching package found: ought-spec ^X.Y.Z` | Crates.io index propagation race | Re-run the workflow — by the second attempt, the index has caught up |
| `error: crate version is already uploaded` | The version already exists on crates.io for some crates (e.g. you partially published manually) | This means the workspace publish succeeded for some but not all. You'll need to add `--exclude` flags for the ones already published, OR bump to a new version. The fastest fix is to just go to the next version |
| `error: failed to verify package tarball: failed to download <crate> v<version>` | Workspace verification chicken-and-egg | Add `--no-verify` to `just publish-crates` invocation. Note: this is only an issue for the very first publish of a brand-new crate, not for subsequent releases |
| `error: 429 Too Many Requests` | Hit crates.io's "new crate" rate limit (5 new crates per ~10 min) | Wait, then re-run. Only happens when publishing brand-new crates that don't exist on crates.io yet — once they exist, version updates are not rate-limited the same way |

For the rate limit scenario specifically, this only ever happens on the **first
publish** of a workspace — once all 8 crates exist on crates.io with at least
one version, future releases publish updates which are not subject to the
"new crate" limit.

### `bump-homebrew` fails

| Symptom | Cause | Fix |
|---|---|---|
| `failed to download <target>.tar.gz` | The `release` job's binary upload to GitHub Releases hadn't propagated yet (the retry loop has 5 attempts, 5s apart) | Re-run the job — by then the assets are available |
| `Permission denied` on push | Bot doesn't have access to `homebrew-tap` (e.g. app was uninstalled) | Re-install `sosein-release-bot` on the org with `repository_selection: all` |
| `git push` non-fast-forward | Tap repo's main moved (very rare) | Re-run the job |

If `bump-homebrew` fails permanently and you give up on auto-bumping for that
release, you can manually update the formula in `soseinai/homebrew-tap`. The
formula needs all 4 platform tarball SHAs, which you can compute with:

```sh
TAG=v0.2.0
BASE=https://github.com/soseinai/ought/releases/download/$TAG

for target in aarch64-apple-darwin x86_64-apple-darwin aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu; do
  sha=$(curl -fsSL "$BASE/ought-$target.tar.gz" | sha256sum | awk '{print $1}')
  echo "$target  $sha"
done
```

Then clone the tap repo and edit `Formula/ought.rb`, replacing the four
`sha256` lines and the `version` line. Commit and push:

```sh
git clone https://github.com/soseinai/homebrew-tap
cd homebrew-tap
# edit Formula/ought.rb
git commit -am "Bump Homebrew formula to $TAG"
git push origin main
```

Alternatively, copy the heredoc template from `release.yml`'s `Generate
Homebrew formula` step and substitute the values yourself — that's the
single source of truth for the formula structure.

---

## Verification (post-release)

After the workflow completes, run these to confirm everything landed:

```sh
TAG=v0.2.0   # or whatever you released

# 1. GitHub release exists with all 4 binaries
gh release view $TAG --json tagName,assets --jq '{tag: .tagName, assets: [.assets[].name]}'

# 2. crates.io has the new version for all 8 crates
for c in ought-spec ought-gen ought-run ought-report ought-analysis ought-server ought-mcp ought; do
  ver=$(curl -s "https://crates.io/api/v1/crates/$c" | grep -o '"max_version":"[^"]*"' | head -1)
  printf "  %-16s %s\n" "$c" "$ver"
done

# 3. Homebrew tap has been bumped
gh api repos/soseinai/homebrew-tap/commits --jq '.[0:2] | .[] | {sha: .sha[0:8], msg: .commit.message, author: .commit.author.name}'

# 4. The shell installer fetches the right version
curl -sS https://raw.githubusercontent.com/soseinai/ought/main/install.sh | OUGHT_INSTALL_DIR=/tmp/ought-test sh
/tmp/ought-test/ought --version

# 5. brew install actually works
brew install soseinai/tap/ought
ought --version
```

If all 5 succeed, the release is fully shipped.

---

## Initial setup (one-time, for reference)

These steps were done once when setting up the release pipeline. You don't
need to redo them for every release. They're documented here so the setup
is reproducible.

### 1. The `sosein-release-bot` GitHub App

Created at https://github.com/organizations/soseinai/settings/apps with:

- **Permissions**: Repository contents (read & write), Metadata (read)
- **Repository access**: All repositories (`repository_selection: all`)
- **App ID**: 3199403
- **Private key**: stored as `APP_PRIVATE_KEY` org secret

### 2. Branch protection ruleset on `main`

Created via `gh api repos/soseinai/ought/rulesets` with:

- Target: `~DEFAULT_BRANCH`
- Bypass actors: `[{actor_id: 3199403, actor_type: Integration, bypass_mode: always}]`
- Rules: `pull_request` (0 approvals), `required_status_checks` (`test`),
  `non_fast_forward`, `deletion`

To inspect or modify:

```sh
gh api repos/soseinai/ought/rulesets
gh api -X PUT repos/soseinai/ought/rulesets/<id> --input <json-file>
```

### 3. The `soseinai/homebrew-tap` repo

Created via `gh repo create soseinai/homebrew-tap --public` with `Formula/ought.rb`
copied from the original location. The bot has automatic access via
`repository_selection: all`. No additional setup needed.

### 4. Org secrets

Set at https://github.com/organizations/soseinai/settings/secrets/actions:

- `APP_ID` — `3199403`
- `APP_PRIVATE_KEY` — the PEM contents from the GitHub App's private key download
- `CARGO_REGISTRY_TOKEN` — token from https://crates.io/me with `publish-update`
  scope for the 8 ought crates

All three are visible to all repos in the org, so any future repo that needs
to mint a bot token or publish to crates.io can use them.

---

## Release cadence and versioning

Ought follows [semantic versioning](https://semver.org/):

- **Patch (`0.X.Y` → `0.X.Y+1`)**: bug fixes, internal cleanups, doc updates
- **Minor (`0.X.Y` → `0.X+1.0`)**: new features, backwards-compatible API additions
- **Major (`X.Y.Z` → `X+1.0.0`)**: breaking changes (unlikely until 1.0)
- **Pre-release (`X.Y.Z-rcN`, `X.Y.Z-betaN`, `X.Y.Z-alphaN`)**: a release
  candidate that doesn't change semantics if it goes through. crates.io
  ranks these below stable versions; `cargo install ought` won't pick them
  up by default (use `cargo install ought@0.2.0-rc1` to opt in).

There is no fixed release cadence. Cut a release whenever there's something
worth shipping, and use rcs for anything that needs manual smoke-testing
before going stable.
