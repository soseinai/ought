# Ought Soft Launch — Friday April 10, 2026

Target: get Ought in front of developers on HN, X, Reddit, and the Rust community. Limited funding, solo. Everything below is prioritized — do the top stuff first, cut from the bottom if time runs out.


## Critical path (Thursday)

These block everything else. If these aren't done, don't launch.

- [ ] **End-to-end happy path works.** Someone can `cargo install ought` → `ought init` → write a spec → `ought generate` → `ought run` → see a report. Test this on a clean machine (or in a fresh container).
- [ ] **Hide unfinished commands.** If `survey`, `audit`, `blame`, `bisect`, `mcp`, or the web viewer aren't solid, don't register them in the CLI. Ship 3 things that work over 10 that half-work. Can gate behind `--experimental` or just omit them.
- [ ] **BYOK experience is smooth.** First run with no LLM CLI configured should print a clear, friendly message — not a panic or cryptic error. Support `claude` and `chatgpt` CLIs at minimum. Document the setup in README.
- [ ] **Pre-built binaries.** Set up a GitHub Actions release workflow (cargo-dist, or a simple matrix build) to produce binaries for macOS (x86_64 + aarch64) and Linux (x86_64). Most HN readers won't have a Rust toolchain. Tag a release.
- [ ] **Homebrew tap works.** You have a `Formula` directory — verify `brew install` works end-to-end from the tap.
- [ ] **LICENSE file committed.** MIT is already declared in the README — make sure the actual LICENSE file is in the repo root.


## High priority (Thursday evening / Friday morning)

These significantly improve launch success but aren't hard blockers.

- [ ] **Terminal demo recording.** 60-second asciinema or GIF showing the full flow on a real (small) project. Embed at the top of the README. Tools: `vhs` (Charm) lets you script recordings, or `asciinema` + `agg` for GIF conversion.
- [ ] **README trim for launch.** Current README is comprehensive but long. For launch day: one-sentence pitch → 15-second install → 2-minute walkthrough → diagram → link to docs for everything else. Move the deep reference material to docs/.
- [ ] **Landing page at sosein.ai.** Single-page, dark, minimal, terminal-aesthetic. Content: tagline, the triangle diagram, a spec example, install commands, GitHub link. No email capture needed — just drive to the repo. Deploy via GitHub Pages or Cloudflare Pages.
- [ ] **Pick a GitHub org.** If the repo isn't under an org yet, decide if it launches under your personal account or a `sosein` / `ought` org. Org looks more intentional.


## Launch content (Friday)

- [ ] **Write the Show HN post.** Title: "Show HN: Ought – Behavioral specs that test themselves". Body: 3–4 paragraphs max. What it does, why it exists (the is-ought gap / deontic logic angle — HN loves philosophy-meets-engineering), a quick example, a link. Don't oversell.
- [ ] **Write the X thread.** Hook tweet ("What if your test specs could generate their own tests?") → 3–4 tweets showing the flow with code/terminal screenshots → link to repo. Post same morning as HN so they reinforce each other.
- [ ] **Reddit posts.** Tailor the angle per subreddit:
  - r/rust — "I built a Rust CLI that generates tests from markdown specs using LLMs"
  - r/programming — lead with the concept and the philosophy angle
  - r/ExperiencedDevs — lead with the pain point ("specs drift from tests")
- [ ] **Rust community channels.** Post in the Rust Discord (#showcase) and Rust Zulip. These are smaller but high-signal, friendly audiences.
- [ ] **Submit to This Week in Rust.** They accept submissions via PR to their repo — gets you in front of the entire Rust newsletter audience for free.
- [ ] **Lobsters.** If you have an invite or can get one, post there. Very technical audience, high engagement.


## Nice to have (if time permits)

- [ ] **Dev.to / Hashnode post.** A longer "why I built this" narrative. Gives you a shareable URL that's more story than README. Cross-post to both.
- [ ] **Reach out to newsletter curators.** TLDR, Changelog, Console.dev. Long shot on 2-day notice but costs nothing to email.
- [ ] **ought dogfoods itself.** You already have specs in `ought/` — if `ought check` passes on its own specs, mention this in the launch post. Self-referential tools are catnip for HN.
- [ ] **GitHub repo polish.** Topics/tags, a concise "About" description, social preview image (can be a simple dark card with the logo/name + tagline).


## Explicitly skip for now

These are good ideas but will burn time you don't have. Do them next week based on launch feedback.

- Analytics / telemetry
- Comprehensive documentation beyond README + design doc + grammar doc
- Web viewer (ought-server)
- MCP server (unless it already works)
- CI badges, code coverage, contribution guidelines
- Blog post on Anthropic/OpenAI blogs (month-out play)
- Paid promotion of any kind
