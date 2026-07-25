# CLAUDE.md — wgpu-graft Repository

This file currently covers workspace tooling only. See `docs/` or `README.md`
for project context.

## Workspace Tooling: sem & weave

Two non-authoritative structural tools from Ataraxy Labs are wired into this
repo. Both read code structure via tree-sitter, not program semantics; they
never replace `cargo check` / `cargo test` / compiling.

**weave** (entity-level git merge driver). `.gitattributes` maps ~46-54 file
types to `merge=weave`; ordinary `git merge` resolves false conflicts where
independent edits touch different functions, structs, or keys in the same
file. A true same-entity conflict still produces markers, tagged with the
entity name and reason (e.g. `function 'foo': both modified`). Preview a
merge before running it with `weave-cli preview <branch>`.

The merge-driver binary path is machine-local, not committed (git can't
version a local binary path). It is wired via `git config --global
merge.weave.driver` on this machine, which covers every repo including
fresh clones, so no per-repo setup is needed here. On a new machine, install
with `cargo install --git https://github.com/Ataraxy-Labs/weave weave-cli
weave-driver`, then either repeat the global `git config --global
merge.weave.*` setup or run `weave setup` in each repo.

**sem** (semantic version control): entity-level diff, context, impact, and
blame queries on top of Git. Installed via `cargo install --git
https://github.com/Ataraxy-Labs/sem sem-cli` and registered as a
user-scoped Claude Code MCP server (`sem_diff`, `sem_context`, `sem_impact`,
`sem_entities`, `sem_blame`, `sem_log`; call these directly as tools). CLI
fallback if the MCP tools are not available:

```bash
sem diff --format plain
sem context <Symbol> --budget 2000 --json
sem impact <Symbol> --file <path> --json
```

Use `sem context` and `sem impact` to brief yourself on a symbol before
editing it. Avoid unfiltered scans over large directories: `sem entities
crates --json` on a big tree dumps a lot.
