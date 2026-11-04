# Open questions

Decisions made without the owner during autonomous work. Each one has a recommended option that was
**already applied**; the code is marked `TODO(open-question): Qn` where relevant. Agree, or say which to redo.

## Q1. Git author e-mail in commit history

Commits are created with the global git identity of the machine, which includes a personal e-mail.
That is commit metadata, not code, but it becomes public on push.

- **A (recommended, not applied):** before the first public push, set a GitHub noreply address in this repo
  (`git config user.email <id>+<login>@users.noreply.github.com`) and rewrite history with
  `git rebase -r --root --exec 'git commit --amend --no-edit --reset-author'`.
- B: keep it as is.

Nothing was changed — rewriting identity is the owner's call.

## Q2. Crate name `wallpaper-core` instead of `core`

The plan says `cargo run -p core`. A crate named `core` collides with Rust's built-in `core` crate and breaks
`use core::...` everywhere. The directory stays `crates/core`, the package is named `wallpaper-core`
(`cargo run -p wallpaper-core --example pick ...`).

## Q3. Repository URL in the User-Agent

Wikimedia asks API clients to send a User-Agent with contact info. It currently says
`https://github.com/OWNER/dynamic-themed-wallpapers` (also in `Cargo.toml` `repository`).
Replace `OWNER` once the public repo exists — search for `TODO(open-question): Q3`.

## Q4. The sandbox cannot compile C code → pure-Rust choices tonight

nono does not allow reading `/usr/include`, so any crate with a C part fails to build here
(`ring` for TLS, `rusqlite`'s bundled SQLite). Also `~/.cargo` is read-only, so cargo runs with
`CARGO_HOME=$PWD/.cargo-home` (git-ignored).

- TLS: default feature `tls-ring` (standard, builds fine with the Android NDK). Here I build with
  `--no-default-features --features tls-graviola` (pure-Rust provider by the rustls author).
  **Recommended:** keep as is. Optional fix for the sandbox: `nono run --allow /usr/include -- claude`.
- Storage (stage 5): see Q5.
