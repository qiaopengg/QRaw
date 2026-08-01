强制约束：
请始终使用简体中文与我对话，并在回答时保持专业、简洁。
本项目二开必须严格非侵入：禁止直接修改上游 `CyberTimon/RapidRAW` 代码，所有功能仅可通过扩展、包装、配置化或独立模块方式接入。
所有二开功能接入与重构必须同时遵守 `docs/feature-integration-guidelines.md`。

# AGENTS.md

## 1. Core Principles

- Think before coding. Do not assume. Do not hide confusion.
- Read before writing.
- Make surgical changes only. Every change must directly serve the user request.
- Prefer simplicity. Solve the current problem with the least code necessary.
- Reuse first. Do not reimplement functionality that already exists in the project.
- Follow the existing codebase conventions instead of introducing personal preferences.
- Fail loudly. Do not pretend work is complete.
- Every change must be explainable: why this file, why this approach, and how it was verified.

## 2. Before Coding

Before implementation, you must:

- Read the relevant files, callers, exports, tests, and existing utilities.
- Search the project for similar existing implementations.
- Define the goal and success criteria.
- If the request has multiple reasonable interpretations, list them and ask instead of silently choosing.
- If there is a simpler approach, state it.
- If there is risk, conflict, or uncertainty, explain it.

Simple, clear, low-risk tasks may be executed directly.

Tasks involving the following require a short plan before implementation:

- Architecture changes
- New dependencies
- Security or permission logic
- Data migrations
- Public APIs
- File splitting
- Broad changes
- Behavior changes that may affect multiple modules

## 3. Reuse-First Rule

AI must not write new code before checking whether the project already has an existing implementation.

Before writing any new code, ask:

1. Can this code be avoided entirely?
2. Can this be done with less code?
3. Can existing code be reused?
4. Does the project already have a function, component, hook, service, schema, type, config, constant, or utility for this?
5. Is there a nearby pattern that should be followed?
6. Would this new code create duplicate logic?

Requirements:

- Before adding a function, search for similar functions.
- Before adding a component, search for similar components.
- Before adding a type, search for existing type definitions.
- Before adding a constant, search for existing constants or config.
- Before adding validation logic, search for existing schemas or validators.
- Before adding an API call, search for an existing client, service, or repository.
- Before adding formatting, transformation, or parsing logic, search for existing helpers or utilities.
- Before adding error handling, search for existing error types, error codes, and error handling patterns.
- Before adding state management, search for existing stores, contexts, hooks, or state patterns.

Search should focus first on directories, names, call chains, nearby code, and tests relevant to the current task. Do not scan the entire repository without boundaries just to prove that a search happened.

Never:

- Do not copy-paste existing logic and rename it slightly.
- Do not reimplement something because you failed to inspect the codebase.
- Do not create a new utility that overlaps with an existing one.
- Do not create a second state management, request, error handling, or validation pattern.
- Do not create future maintenance cost by making two versions of the same logic.

If a new implementation is truly needed, explain:

- Where you searched.
- Why the existing implementation cannot be reused.
- How the new implementation follows existing patterns.

## 4. Change Scope Rules

- Keep the change as small as possible.
- Every changed line should trace back to the user request.
- Match the existing code style, even if you would personally write it differently.
- For bug fixes, prefer adding a regression test that reproduces the bug.
- When changing behavior, add or update tests.
- Remove only imports, variables, functions, or files made unused by your own change.
- For multi-step tasks, provide a short plan and report progress and validation at meaningful checkpoints.

## 5. Never Do

- Do not refactor unrelated code.
- Do not “clean up” nearby code, comments, formatting, or folder structure.
- Do not add features the user did not request.
- Do not create generic abstractions for one-off logic.
- Do not add dependencies without first explaining why and getting approval.
- Do not read, print, commit, or expose `.env`, secrets, tokens, certificates, private keys, or credentials.
- Do not ignore failing tests.
- Do not claim completion without verification.
- Do not mix two architecture patterns, state management patterns, or coding styles.
- Do not keep appending logic to the same file to hide design problems.

If a task truly needs environment or credential-related information, ask the user for non-sensitive variable names, example values, or redacted information. Do not read or expose real secrets.

## 6. File Growth Control

One of the most common AI coding failures is repeatedly appending code to the same file until it becomes too large, unclear, and hard to maintain. Actively prevent this.

### 6.1 File Growth Judgment Principles

Line-count thresholds are signals for self-checking, not the only judgment criteria.

More important than raw line count:

- Whether the file has multiple responsibilities.
- Whether duplicate logic is appearing.
- Whether the file is hard to read, test, or reuse.
- Whether the file has become a dumping ground for new logic.
- Whether a small extraction would reduce complexity.

Simplicity does not mean putting everything into one file. Simplicity means clear responsibilities, less duplication, readable call paths, and easy testing.

### 6.2 Pre-Addition Self-Check

Before adding code to an existing file, check:

- What is this file's current main responsibility?
- Does the new logic belong to that responsibility?
- Will the new logic introduce a second or third responsibility?
- Is there an existing module, component, hook, service, helper, schema, or config file where this belongs?
- Should this be a small new file instead of more code appended to a large file?

### 6.3 Tiered Trigger Rules

These thresholds trigger judgment and explanation. They are not mechanical bans.

- Under 300 lines: modify normally, but keep responsibilities clear.
- 300 to 500 lines: if the new logic introduces a new responsibility, prefer extraction.
- Over 500 lines: before adding major logic, perform a file growth self-check and explain why continuing in this file or extracting is appropriate.
- Over 700 lines: do not keep adding major business logic. Prefer creating a new module, child component, hook, service, helper, schema, or config file.
- Over 800 lines: do not keep growing handwritten source files unless the user explicitly asks and the reason is explained.

### 6.4 Single-Change Growth Rules

- If a change would add more than 50 lines of major logic to one file, check whether it can be reused or extracted.
- If a change would add more than 80 lines of major logic to one file, explain whether a split is needed.
- If a change would add more than 120 lines of major logic to one file, split by default unless there is a clear reason to keep it in one file.

“Major logic” does not include generated content, snapshots, lockfiles, large static data, translation tables, test fixtures, or migration files. These files still must not become places for unrelated manual logic.

### 6.5 Preferred Extraction Patterns

- Move UI subsections into child components.
- Move business logic into services or helpers.
- Move reusable state logic into hooks.
- Move constants, schemas, and config into dedicated files.
- Move complex conditionals into clearly named small functions.
- Split tests by behavior into focused test files.

The purpose of splitting is to isolate responsibilities and reduce complexity, not to create a generic framework or future extension layer.

Do not use “splitting” as an excuse for broad unrelated refactoring. Extract only logic directly touched by the current task.

### 6.6 Anti-Append Rule

- Do not put new state, helpers, UI, validation, side effects, and API calls all into the same large file.
- Do not patch a messy file with more conditionals.
- Keep the original file as an orchestrator when possible, not a dumping ground for all logic.
- If the requested change would worsen file bloat, first propose a smaller split.

## 7. Simplicity Rules

- Solve the current problem with the least code necessary.
- Do not add speculative features.
- Do not add configuration that is only “maybe useful later.”
- Do not handle impossible scenarios.
- If the implementation becomes obviously too long, reconsider the approach.
- If 50 lines can solve it, do not write 200.
- If a senior engineer would call it over-engineered, simplify it.

Simplicity does not mean refusing to split files. If splitting makes responsibilities clearer, reduces duplication, or makes testing easier, it supports simplicity.

## 8. Verification Rules

Before finishing, run the checks relevant to the change.

Prefer existing project commands. Do not invent commands. Common examples include:

- `pnpm lint`
- `pnpm typecheck`
- `pnpm test`
- `pnpm build`
- `npm test`
- `pytest`
- `go test ./...`
- `cargo test`

If a check cannot be run, explain:

- Which check was skipped.
- Why it was skipped.
- What risk remains.
- How the user can verify it.

Do not describe “not run” checks as “passed.”

## 9. Failure Handling

- If the same issue fails after 3 attempts, stop trying blindly.
- Summarize what was tried, what likely failed, and what options remain.
- Do not expand the change scope to hide failure.
- Do not delete tests to make results pass.
- Do not lower the verification standard.
- Do not keep stacking patches without understanding the failure.

## 10. Conflict Handling

When instructions conflict, follow this priority:

1. The user's current explicit request.
2. Repository-specific rules.
3. Existing code patterns and tests.
4. The general rules in this document.
5. General best practices.

If the conflict remains unresolved, stop and ask.

Security, privacy, and credential-protection rules should not be overridden by ordinary implementation requests. If a user request may expose sensitive information, explain the risk and ask for redacted input.

## 11. Final Response Requirements

Simple tasks may use a brief final report. Complex tasks require a complete report.

After completing a task, always include at least:

- Which files changed.
- What behavior or result changed.
- Which validation commands were run.
- What the validation results were.

For complex tasks, also include:

- Why each file needed to change.
- Whether existing implementation was reused; if not, explain why.
- Whether file-splitting or file-growth rules were triggered.
- Any skipped checks, remaining risks, or items needing user confirmation.

Do not only say “done.”
