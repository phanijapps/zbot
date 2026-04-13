# Ward Curation

When you enter a ward (root agent only), you own `AGENTS.md` and `memory-bank/{ward,structure,core_docs}.md`. Code does not rewrite them. You curate.

Wards follow **spec-driven development**: every substantive task gets a `specs/{task}/spec.md` (the contract — what and why) and a `specs/{task}/plan.md` (the execution recipe). Specs survive across sessions. Read them before planning new work in an existing ward.

## Reuse hierarchy (check in this order before writing anything new)

1. **Skills (global).** Search, PDF/image text extraction, URL fetch, markdown rendering, file I/O — capabilities that apply across wards live as skills. Check available skills first. If the capability exists, use it. If it doesn't and the need is general-purpose, propose a new skill rather than copying logic into the ward.

2. **Ward-local primitives directory.** Pick a location conventional for the ward's language — `core/`, `pkg/`, `lib/`, `src/`, `internal/`. Put reusable, parameterized domain code there — logic that multiple instances in this ward will call. Register each primitive with a one-liner in `memory-bank/core_docs.md` — name, signature, when to use.

3. **Instance directories.** Thin orchestration only — ideally ≤ 30 lines that wire skills + primitives to this specific input. If you find yourself writing more than that in an instance dir, a primitive is being duplicated; extract it up.

## Curation rules

- **Read before you write.** On ward entry, read `memory-bank/*.md` and walk `specs/` for prior contracts. Extend what's there; don't reinvent.
- **No forced structure.** These files have no mandated headers or format. Shape them to fit the domain and language. A Go ward and a Python ward will end up looking different. That is correct.
- **Terse beats tidy.** Record only what future-you (or a fresh session) will need: conventions, reusable primitives, decisions worth remembering, sharp edges. Skip filler.
- **Update as you learn.** When you add a primitive, add its one-liner to `core_docs.md` in the same turn. When you hit a gotcha worth recording, write it into `ward.md`.
- **Don't re-seed.** If a memory-bank file is populated, don't overwrite it — extend.
- **Specs are the contract.** Before implementation, `specs/{task}/spec.md` must state goal + acceptance criteria + constraints + reuse. Code-agent reads spec first, then plan, then implements.
