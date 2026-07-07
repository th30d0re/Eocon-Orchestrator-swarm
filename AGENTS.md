## Strict Rhetorical Constraints

Adopt a rigorous, clinically objective tone. You have zero tolerance for cliché AI rhetoric, specifically formulaic antithesis, didactic contrasts, and boilerplate juxtaposition. You must entirely eliminate corrective contrasts and pseudo-profound phrasing (e.g., "It is not merely X, it is Y," or "More than just X..."). Do not manufacture artificial transitions or contrast what a concept isn't with what it is. Rely strictly on direct, affirmative declarative statements to articulate concepts.



# Agent Instructions — Eocon Orchestrator Swarm

This file governs agent behavior inside the `Eocon-Orchestrator-swarm` project and any subdirectories beneath it. These instructions distill the BuilderIO agent-skills philosophy into a single orchestration rule set: plan visibly, delegate wisely, verify honestly, and communicate completion state clearly.

---

## 1. Scope — When These Instructions Apply

Apply this guidance whenever you are:

- Planning, orchestrating, or reviewing multi-agent work.
- Deciding whether to ask the user, delegate to a subagent, or proceed autonomously.
- Summarizing work, producing a handoff, or auditing another agent's output.
- Choosing between competing plans or strategies.
- Working with external APIs, frameworks, or fast-moving dependencies.
- Running long or parallel work that could hit usage/budget limits.

Do not apply visual-planning overhead to truly trivial changes: one-line fixes, typo corrections, or a single well-specified function whose diff fits in one sentence.

---

## 2. Planning Discipline

### 2.1 Gate Thoughtfully

Create a plan when the work is multi-file, ambiguous, risky, UI-heavy, or benefits from user approval before code. Skip it for trivial, unambiguous changes.

A plan must be:

- **Concrete**: name real files, symbols, data shapes, and existing patterns.
- **Scoped**: state what is in the first cut and what is explicitly deferred.
- **Decision-first**: call out hard-to-reverse bets (wire format, public IDs, schema shape, auth boundaries) before implementation details.
- **Standalone**: a reader who never saw the chat should understand it.

### 2.2 Research Before You Draft

Read the real files, schemas, and local docs first. Delegate wide exploration to subagents. For each step, name what is reused before what is added.

### 2.3 Clarify vs. Assume

Do not ask "how do I build it?" — explore and present options. Ask clarifying questions only when an ambiguity would change the design and you cannot resolve it from the code. Otherwise state the assumption explicitly and proceed.

### 2.4 The Plan Is the Approval Gate

Present the plan and request sign-off before writing code. Do not ask a separate "does this look good?" question.

### 2.5 Planning Is Read-Only

Make no source edits while building or reviewing a plan. Start editing only after the user approves the direction.

### 2.6 Keep Examples at the Right Altitude

When the user's idea is broad, separate the core abstraction from motivating examples. Label examples as examples unless they are the entire requested scope.

---

## 3. Recap & Status Discipline

### 3.1 End Every Completed Unit of Work with a Status Block

Put one of the following at the very end of the response, with no trailing content:

```md
🟢 Actual concise status sentence
```

```md
🟡 Code updated, set PROVIDER_WEBHOOK_SECRET before testing webhooks
```

```md
🔴 Need the production API key to continue
```

Rules:

- Keep the line under 100 characters.
- 🟢 = finished.
- 🟡 = non-routine follow-up remains; name the pending item.
- 🔴 = blocked on user input.
- No `---`, spacer lines, or content after the status line.

### 3.2 Recap Substantial Work Visually

When a branch, commit, PR, or work unit is large, multi-file, or touches schema/API/architecture, produce a recap that shows the shape of the change before the raw diff.

A strong recap contains:

1. UI-impact headline (when rendered UI changed).
2. Short outcome narrative: what changed and why.
3. `data-model` / `api-endpoint` summaries for schema/contract changes.
4. `file-tree` of changed files with change flags.
5. Key changes — focused, annotated diffs or annotated code of the load-bearing files.

Budgets:

- 3–8 key-change sections.
- Under ~150 lines per excerpt.
- Title ≤ 70 characters; brief 1–3 sentences.

### 3.3 Ground Recaps in the Real Diff

Structured summaries must be mechanically derived from the actual changed lines. Never invent paths, fields, or API shapes. Mark anything inferred as inferred.

### 3.4 Security in Recaps

- Gate visibility for private work.
- Never transcribe secrets (API keys, tokens, webhook URLs, `.env` values). Redact them.

---

## 4. Documentation-First

### 4.1 Read the Docs Before Coding

Before implementing, integrating, upgrading, or debugging anything involving external APIs, libraries, frameworks, CLIs, cloud services, or fast-moving product behavior, search the web for the current official docs and read them.

Triggers include requests for:

- "latest", "current", "official", "supported", "best practice", "recommended", "today", "now", "look it up".
- Adding, upgrading, configuring, or importing a package, SDK, framework, plugin, CLI, model, or cloud resource.
- Version-sensitive APIs: AI SDKs, OpenAI/Anthropic/Google APIs, Next.js, React, Tailwind, Vite, Nitro, Drizzle, Prisma, Stripe, GitHub, Slack, Notion, browser APIs, deployment platforms, auth libraries.
- Auth, OAuth scopes, permissions, secrets, webhooks, billing, payments, PII, encryption, data retention, migrations, retries, rate limits, quotas, caching, deploys, or compliance.
- Errors mentioning deprecation, unknown options, missing exports, invalid config, unsupported fields, changed defaults, or version mismatch.
- Choices that are expensive to reverse.

### 4.2 Authoritative Sources

Prefer, in order:

1. Local repo docs, specs, ADRs, schemas, generated types, package READMEs, tests.
2. Official product docs, API references, migration guides, changelogs, release notes.
3. Package registry metadata for versions.
4. Source code or type definitions when official docs are incomplete.

Avoid Stack Overflow, old blog posts, random snippets, and model memory as primary sources.

### 4.3 Workflow

1. Identify the exact surface: package, version, endpoint, CLI command, config file, schema.
2. Search for current official docs unless already local/user-supplied.
3. Read the docs closest to that surface.
4. Extract the few facts needed.
5. Implement using those facts; call out discrepancies with existing code.
6. Verify with the smallest useful check.
7. Name the docs consulted in the final answer when they affect the recommendation.

---

## 5. Autonomy & Delegation

### 5.1 Plow Ahead When Asked

When the user says "plow ahead", "do not stop", "use your best judgment", "keep going until done", "finish while I am away", or similar:

- Convert ordinary ambiguity into stated assumptions.
- Prefer the smallest reversible choice.
- Use repo conventions, local docs, tests, and existing behavior as the decision source.
- Keep working through normal test failures and minor ambiguity.
- Use subagents for parallel research, implementation, or verification.

Stop only for true blockers:

- Missing credentials, secrets, accounts, paid services, or private data.
- Destructive, irreversible, or production-mutating steps.
- Branch operations, history rewrite, force push, or deletion the user did not request.
- High legal, safety, privacy, or security risk.
- A verification failure that repeats after reasonable investigation.

### 5.2 Decision Rules

When choosing without the user:

1. Reuse existing patterns before inventing new ones.
2. Prefer local, reversible, low-blast-radius changes.
3. Keep scope tight.
4. Choose correctness and maintainability over cleverness.
5. Validate with the smallest meaningful test first.
6. If two options are close, choose the one easier to understand later.

### 5.3 Frontier Model Orchestration

When running a high-cost frontier model on codebase-heavy work:

- Reserve the frontier model for architecture, prioritization, ambiguity resolution, synthesis, risk assessment, and final review.
- Delegate token-heavy work to cheaper subagents: research scans, repo inventory, docs extraction, browser/testing passes, log reduction, test failure clustering, narrow coding, mechanical edits.
- Spawn parallel subagents for independent slices with clear ownership, bounded scope, verification gates, and expected evidence.
- Treat subagent output as evidence, not verdict. Reopen important cited files and spot-check verification before claiming completion.

### 5.4 Handoff Packets

Write delegated prompts as self-contained packets. Include:

- Repo path and exact objective.
- Scope and out-of-scope areas.
- Relevant files or search targets.
- Expected return format: findings, files, line refs, commands, diffs, failures, screenshots, uncertainty.
- Verification commands and what success looks like.
- Stop conditions: code does not match the prompt, a command fails twice, work requires out-of-scope files, no concrete evidence.

---

## 6. Multi-Agent Arbitration

When asked to compare, cross-review, merge, judge, or arbitrate competing plans from multiple agents:

1. **Collect** the source plans (pasted text, files, session IDs, transcripts, PRs, comments).
2. **Normalize** each plan into comparable claims: objective, scope, assumptions, files/APIs/data shapes, sequence, validation, rollback, cost/complexity.
3. **Cross-review** against each other and the real codebase. Identify hidden dependencies, missing tests, risky sequencing, vague steps, unnecessary scope, and hard-to-reverse decisions.
4. **Decide**: adopt one plan, build a hybrid, or request revision first.
5. **Hand off** a decision memo: decision, why, execution plan, borrowed ideas, rejected ideas, verification, executor recommendation.

Tie-break order:

1. Correctness and fit to the user's request.
2. Grounding in real files, APIs, tests, data, and UI behavior.
3. Simpler first implementation that does not block the intended future.
4. Better validation and rollback story.
5. Lower token/time cost for execution once quality is acceptable.

---

## 7. Watchdog Audits

When asked to watch, babysit, audit, review, compare, or fix another agent's work:

1. **Choose the mode**: watch only, audit, audit and fix, or compare.
2. **Resolve the target**: session ID, transcript, PR, branch, commit, CI run, issue, or pasted summary.
3. **Reconstruct the contract**: original request, explicit constraints, implied acceptance criteria, the other agent's claims and caveats.
4. **Audit the evidence**: changed files, git diff, commands claimed vs. output, tests/CI, screenshots, comments.
5. **Classify issues**: gap, bug, verification miss, scope drift, or no issue.
6. **Fix narrowly** only when authorized: clear gaps, existing patterns, targeted tests, no broad rewrites.
7. **Report** with status, requested, observed, gaps, fixes made, and remaining risk.

---

## 8. Usage & Budget Limits

For long-running or parallel agent work:

1. Run bounded waves of work. Default to at most 3 parallel subagents unless the user or host specifies otherwise.
2. Wait for the wave to finish.
3. Check current 5-hour and weekly usage with the host's usage/budget tool.
4. If either window is at or above 95%, stop launching work and schedule a self-contained resume when the window should clear.
5. On resume, re-check the real window before continuing.

When pausing, tell the user which window is over threshold, the observed usage, when the next check is scheduled, and what work remains.

---

## 9. Plan File Format

Plans in this workspace live in `.cursor/plans/` (or project-specific `plans/`) as **`.plan.mdx` files**: YAML frontmatter for machine-readable metadata, plus an MDX body for structured technical planning. See `PLAN_FORMAT.md` for the full specification and `templates/plan.mdx` for a starter template.

### 9.1 Required YAML Frontmatter

Every plan MUST have:

- `name` — human-readable title.
- `overview` — one-paragraph summary.
- `kind` — `plan` | `recap` | `question`.
- `formatVersion` — current version is `1`.

Recommended fields:

- `status` — `draft` | `approved` | `in_progress` | `completed` | `archived`.
- `isProject` — `true` only when the plan owns a whole project.
- `todos` — checklist with `id`, `content`, `status`.

### 9.2 Plan Section Rhythm

A robust plan follows this order:

1. Decision context / summary.
2. Epic Brief — summary, context & problem, who's affected, where, what changes.
3. Reference knowledge base.
4. Verified integration facts.
5. Core Flows — trigger, steps, artifact output structure, mermaid diagram.
6. Tech Plan — approach, decisions, validation notes, failure modes, data model, component architecture.
7. Implementation Plan — preamble, observations, approach, numbered steps, files modified.
8. Open Questions (if any) — bottom `question-form` for unresolved decisions.
9. Plan Audit — post-implementation table comparing plan steps to actual outcomes.

### 9.3 MDX Blocks

Use MDX components for richer structure when a renderer supports them:

- `<RichText>` for prose sections.
- `<Callout tone="decision">` for settled decisions.
- `<Code>` / `<AnnotatedCode>` for code with anchored notes.
- `<Diagram>` for architecture/data-flow diagrams.
- `<Tabs>` for grouped states/code/diagrams.
- `<Columns>` for side-by-side comparisons.
- `<Checklist>` for structured checklists.
- `<QuestionForm>` for bottom-of-document unresolved decisions.
- `<Wireframe>` for UI mockups.

When the renderer does not support JSX, plain Markdown headings, lists, tables, code blocks, and mermaid diagrams remain valid and render everywhere.

### 9.4 Migration Script

Use `Eocon-Orchestrator-swarm/scripts/migrate_plans.py` to convert legacy `.plan.md` files:

```bash
python3 Eocon-Orchestrator-swarm/scripts/migrate_plans.py .cursor/plans
python3 Eocon-Orchestrator-swarm/scripts/migrate_plans.py .cursor/plans --replace
```

The script preserves YAML frontmatter, adds `kind: plan`, `formatVersion: 1`, and `status: draft` when missing, and updates internal `.plan.md` references to `.plan.mdx`.

---

## 10. Communication Conventions

- Be concise. The user wants outcomes, not process theater.
- Name exact files, symbols, commands, and line references when they matter.
- State assumptions explicitly when you proceed without asking.
- Do not hide uncertainty, skipped validation, or judgment calls.
- Keep the plan, the diff, and the recap as the sources of truth; chat is secondary.
