---
name: commit
description: Make a commit in this repository safely — scan the staged diff for personal data and secrets, write a Conventional Commit message. Use whenever committing.
---

This project is open source and must stay free of personal data. Before every commit:

1. `git add` explicit paths (or `git add -A` after checking `git status`).
2. Scan what is staged:
   ```sh
   git diff --cached --name-only | grep -vE 'Cargo.lock|bun.lock' | xargs grep -InE \
     "/home/|/Users/|@gmail|@outlook|@proton|api[_-]?key|secret|token|password|BEGIN (RSA|OPENSSH|PRIVATE)" || echo clean
   ```
   Also look for: absolute paths of this machine, device names, keystores (`*.jks`, `*.keystore`), `.env*`,
   `local.properties`, screenshots showing personal data. Anything found → remove it, do not commit it.
3. Run the checks that match the change: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, `bun test`, `svelte-check`.
4. Commit message: Conventional Commits (`feat(core): …`, `fix(ui): …`, `docs: …`), imperative subject ≤ 72 chars,
   body explains *why*. Keep the attribution lines the harness asks for.
5. Never `git push`, never add a remote, unless the user explicitly asks.
