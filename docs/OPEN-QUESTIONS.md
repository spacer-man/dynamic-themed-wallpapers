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
