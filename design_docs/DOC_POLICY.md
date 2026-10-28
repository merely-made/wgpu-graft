# Documentation Policy

> **Canonical core v2 (2026-10-22; v1 2026-08-24).** Everything from "## Core
> principles" down to the end of §11 is the shared core, copied verbatim into
> every repository
> under `Code` that keeps a `design_docs/`. It is not owned by any one repo:
> change it in all of them or not at all, so that `diff` between any two copies
> shows only local addenda. Repo-specific rules belong under
> **Local addendum** at the foot of this file, never inside the core.

## Core principles

### 1. Control doc growth

Add to an existing doc unless the material is substantial (>500 words), covers
a distinct topic, and is unrelated to any current document. Keep the total doc
count low. Do not create a file for a one-time analysis.

### 2. Eliminate redundancy

Audit before commits and after substantial changes. Newer documents are
generally more authoritative. If two docs disagree, reconcile them — do not let
drift accumulate. Material shared across several repos lives once, in a named
home, and is cited by path from the others; never copied.

### 3. No legacy friction

When a path changes, optimize for clean fit with the new path. Do not preserve
obsolete parallel systems or migration shims unless they are needed for real
user data. Tests track current semantics only.

### 4. Location and archival

- **Active docs** live directly in `design_docs/`. Flat is fine, and is the
  right default. When one domain accumulates enough material to justify it,
  promote that domain to an area root, `design_docs/<area>_docs/`.
- **Area roots**, once a repo has them, take a consistent set of category
  subdirectories. Use only the ones a given area needs:

  | Category | Holds |
  |---|---|
  | `research/` | briefs, surveys, reports, critiques, design probes |
  | `technical_architecture/` | component definitions, boundaries, interfaces, decisions |
  | `implementation_strategy/` | dated plans, development approaches, roadmaps |
  | `design/` | UI/UX, interaction design, accessibility |
  | `testing/` | test plans, harness docs, manual checklists |

- **Docs live with the repo that owns the subject, at that repo's doc root.**
  Do not scatter `design_docs/` into member crates of a workspace: a doc in a
  member crate is invisible to the canonical index, which is a violation of §6
  rather than a matter of taste.
- **Archive**: `design_docs/archive_docs/<YYYY-MM-DD>/` for retired plans and
  superseded notes. Check for an existing checkpoint folder before creating a
  new one. Move rather than delete; delete only with rationale and
  confirmation.

### 5. Cross-referencing

- Within a repo: relative links.
- Across repos: cite by path (`isometry/design_docs/...`), since relative links
  do not cross repository boundaries reliably and rot silently when the
  neighbour moves or is archived.
- Crates: link to crates.io when referring to a public API
  (`https://crates.io/crates/<name>`).
- When a doc moves, repair the links that pointed at it in the same session.

### 6. DOC_README authority

`design_docs/DOC_README.md` is the sole canonical index. It must contain:

- AI-assistant working principles for this project
- An index of all active docs with one-line descriptions
- Pointers to `DOC_POLICY.md` and `PROJECT_DESCRIPTION.md`

Any doc added, moved, or removed requires a `DOC_README.md` update in the same
session. If any other index disagrees with `DOC_README.md`, `DOC_README.md`
wins.

### 7. PROJECT_DESCRIPTION.md ownership

`design_docs/PROJECT_DESCRIPTION.md` — inside the doc root, not at the
repository root — is reserved for the maintainer. Do not edit it without
explicit instruction. Treat it as authoritative and surface contradictions for
discussion rather than resolving them silently.

The root `README.md` is derived from `PROJECT_DESCRIPTION.md` and the current
authoritative docs. Speculative features without plans appear only in
`PROJECT_DESCRIPTION.md`.

### 8. Plan documents

Work that changes code — not doc-only work — gets a dated plan named
`<YYYY-MM-DD>_<keyword>_plan.md`, in `design_docs/` or, where the repo has area
roots, in `<area>_docs/implementation_strategy/`. Each plan carries:

- A dated **Status** line, kept current: plan, in progress, landed, superseded
  by X.
- **Phases** organised by feature target and validation criteria, each with
  **done-conditions**. Never calendar labels — no "Day 1", no "Week 2" — and
  never time estimates.
- A **Findings** section for facts verified during the work, dated, with code
  references.
- A **Progress** log, dated, appended as phases land: a line per landing,
  naming what landed.

A plan holds no receipts. Run evidence — hashes, seeds, timings, counts,
capture paths, commands run, run narratives — goes to
`archive_docs/<YYYY-MM-DD>/<plan file stem>_receipts.md`, when it is kept at
all; evidence that matters is a test in the repository or a line in Findings.
Thresholds in done-conditions stay in the plan. When a plan grows its history
past its open work, cut a new plan from its open, deferred and follow-on items
and archive the old one whole (as on 2026-10-28).

Code samples in a plan state whether they are illustrative or compile-ready.

Update the plan every two prompts on the project, or every two completed tasks.
Re-read it before resuming work rather than working from memory of it. On
completion, extract any deferred or still-open points into a new or existing
plan *before* moving it to `archive_docs/<date>/`.

### 9. Implementation feedback loop

Every implementation pass is also a design probe. After each pass, disseminate
structural learnings to the relevant plans and docs in the same session.
Surface architectural problems explicitly in the plan even when the fix is
deferred.

### 10. Workflow rule for AI assistants

Read `DOC_README.md` first, then this policy, before starting work. Any durable
working principle learned during a session is promoted into `DOC_README.md`'s
working-principles section in that same session.

### 11. Decisions and doctrine

Answers to design questions are *cases*, kept in the words given: a
repository's case record, where it keeps one, and each plan's Decisions
section. What is read during work is the *doctrine*, `doctrine.md` in
`design_docs/`: short statements by area, each citing what it rests on. An
answer to an operational question (merge, push, which run to accept) is
not a case; it lives in the commit message. A repository with no doctrine
yet keeps rules 1 to 4 until its first statement is written.

1. **Keys are mechanical.** A case in the record takes the next number in
   answer order; a plan's decision takes `<plan keyword> D<n>` in answer
   order, the keyword from the plan's file name; a statement keeps
   `<area slug>.<n>` within its area. A key in another repository is
   prefixed with its name, `mere:stack.1`. A plan decision that code or
   another plan comes to rely on is promoted to a case number; its entry
   keeps its key and gains `promoted`.
2. **Every answer has an outcome.** A question names the statement it
   bears on. The answer is recorded as *applies* (filed under the
   statement, unchanged), *refines* (the statement rewritten),
   *contradicts* (back as a question before anything is written) or *new*
   (a statement added). Only *new* grows the doctrine.
3. **Links are typed and written once.** Each case, plan decision and
   statement carries one fields line, `Links: area … · draws on … ·
   amends … · replaces … · contradicts … · plans … · governs … · stack ·
   summary …`, `summary` last. Reverse links, mentions and code citations
   are generated, never written.
4. **Generated files are never edited or hand-merged.** The decisions
   tool writes `decisions.toml` in `design_docs/` and, beside a case
   record, `case_register.md`; the pre-commit hook runs it and `lookup`
   answers from it. Its journal and derived citations are a local cache.
   A `decisions_config.toml` beside the manifest names the case record, if
   any.
5. **Doctrine is consolidated, not accumulated.** `doctrine_budget.toml`
   in `design_docs/` caps each area's statements and every statement's
   body lines; `check` reports an overrun and never refuses, and the next
   round touching the area opens with its consolidation. A statement whose
   governed code is gone or whose standing has faded enters that round
   too, to be kept, merged or retired.
6. **One note per statement.** A statement carries one italic note after
   its fields line: its latest outcome, in the words given. Earlier notes
   leave; git keeps them. What is built, owed or unverified is said
   before the fields line.

## Local addendum — Wgpu-graft

This policy was founded 2026-09-03, when the cross-repo release plan needed a
durable home in this repository.

Docs are flat in `design_docs/`; no area roots have been promoted yet, and
core §4 says flat is the right default until one domain earns promotion.

The repository already had a root-level `docs/` directory before this policy
was founded. Those documents remain indexed from `DOC_README.md` as legacy
reference material until a deliberate doc-hygiene pass moves or archives them.
New durable implementation and release plans go in `design_docs/`.

**One core requirement is not yet met:**

- Core §7 refers to `PROJECT_DESCRIPTION.md`, which does not exist here yet.
  Until it does, the root `README.md` and `docs/project_wgpu_graft.md` stand
  alone and §7's derivation rule is inert rather than violated.
