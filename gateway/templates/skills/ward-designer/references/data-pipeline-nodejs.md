# data-pipeline — Node.js (TypeScript)

Use this reference for a TypeScript data-pipeline ward. Strong fit when source systems are JavaScript-native (Stripe webhooks, SaaS APIs with JS SDKs) or when integrating with a Node service. Less common than Python for heavy analytics, but reasonable for ingest + persist workflows.

**Example scenario:** A `billing-pipeline` ward that ingests Stripe events and shopping-cart events, reconciles them, and produces billing-ready records. Sub-domains: `monthly-invoice-run` (monthly invoice generation), `revenue-recognition` (ASC 606 revenue waterfall).

---

## AGENTS.md

````markdown
# Billing Pipeline

A TypeScript ward for extracting billing events from Stripe and the app database, reconciling them, and producing invoice-ready records.

## Scope

**In scope**
- Extract Stripe subscriptions, invoices, payments
- Extract app-database account/usage records
- Reconcile event streams against account records
- Produce invoice-ready records and ASC 606 waterfalls

**Out of scope**
- Live billing display for end users → customer-facing service
- Tax computation → use dedicated tax service
- Payment processing → Stripe handles this

## Conventions

- **Package layout:** npm workspace. `"type": "module"`.
- **Import syntax:** `import { <symbol> } from '@<org>/billing-pipeline/<module>'`.
- **File boundaries:** One file per source, one per transform, one per sink.
- **Idempotency:** Every transform is deterministic. Every sink supports `upsert` keyed on `(entity_type, entity_id, as_of_date)`.
- **Schemas:** `zod` schemas for every source and sink. Live in `src/schemas/`.
- **Error handling:** Fail fast on schema drift. Typed errors (`class SchemaError extends Error`).
- **Data paths:** Raw extracts JSON under `raw/<source>/<YYYY-MM-DD>/`. Processed parquet or JSON under `processed/`. Sub-domain outputs under `reports/<sub-domain>/`.
- **Money:** `string` in internal math (via `decimal.js`). Never `number`.
- **Dates:** ISO-8601 strings at boundaries, `Date` internally. `date-fns` for arithmetic.
- **Tests:** Vitest with fixtures in `src/**/__fixtures__/`.

## Report staging

Parallel `specs/<sub-domain>/` and `reports/<sub-domain>/`. Required `summary.md` + `manifest.json`.

## Sub-domain naming

Kebab-case. Examples: `monthly-invoice-run`, `revenue-recognition`.

## Cross-referencing

Standard `## Related sub-domains` in `summary.md`.

## DOs

- Validate every source response with `zod` at the boundary.
- Upsert, never blind-append. Use `(entity_type, entity_id, as_of_date)` as the key.
- Land raw extracts before transforming.
- Track the data window explicitly in every `manifest.json`.

## DON'Ts

- Do not use `number` for money. Use `decimal.js` via string.
- Do not read production DB directly inside transforms — go through the raw extract layer.
- Do not use `any`. `unknown` + narrow.
- Do not commit Stripe keys. Environment variables only.

## How to use this ward

Check ward.md → structure.md → core_docs.md before writing new code.
````

---

## memory-bank/ward.md

````markdown
# Ward: billing-pipeline

## Purpose
Extract billing events, reconcile against account records, produce invoice-ready outputs.

## Sub-domains this ward supports

| Sub-domain slug | Description | Related |
|---|---|---|
| `monthly-invoice-run` | Monthly invoice generation with reconciliation | — |
| `revenue-recognition` | ASC 606 revenue waterfall per account | `monthly-invoice-run` |
| `dunning-report` | Past-due account tracking for collections | `monthly-invoice-run` |

## Sub-domains this ward does NOT support

- End-user billing UI → customer-facing service
- Tax computation → tax service

## Key concepts

- **Reconciliation** — matching Stripe events to app-DB account records by a shared key.
- **Idempotency key** — `(entity_type, entity_id, as_of_date)` for upserts.
- **Waterfall** — ASC 606 revenue recognition schedule over subscription term.

## Dependencies

- **npm:** `stripe` (official), `pg` or `mysql2`, `zod`, `decimal.js`, `date-fns`, `vitest`.
- **External:** Stripe API, app DB.

## Assumptions

- Node.js >= 20.
- All money values in USD cents unless explicitly marked currency.
````

---

## memory-bank/structure.md

````markdown
# Structure — billing-pipeline

```
billing-pipeline/
├── AGENTS.md
├── package.json
├── tsconfig.json
├── vitest.config.ts
├── memory-bank/
├── src/
│   ├── sources/
│   │   ├── stripe.ts                   # (planned)
│   │   └── app_db.ts                   # (planned)
│   ├── transforms/
│   │   ├── reconcile.ts                # (planned) match Stripe ↔ app records
│   │   ├── invoice_run.ts              # (planned)
│   │   └── rev_rec_waterfall.ts        # (proposed)
│   ├── sinks/
│   │   ├── parquet_writer.ts           # (planned)
│   │   └── invoice_exporter.ts         # (proposed)
│   └── schemas/
│       ├── stripe_invoice.ts
│       └── app_account.ts
├── raw/
├── processed/
├── specs/
│   ├── monthly-invoice-run/            # (planned)
│   └── revenue-recognition/            # (proposed)
└── reports/
    ├── monthly-invoice-run/
    │   ├── summary.md
    │   ├── manifest.json
    │   └── invoices_2026_04.parquet
    └── revenue-recognition/
        └── ...
```

## Extension policy

- New source → `src/sources/<name>.ts` + zod schema in `src/schemas/`.
- New transform → `src/transforms/<name>.ts`, register.
- New sub-domain → `specs/<slug>/` + `reports/<slug>/`.

## Cross-cutting rules

- Every public function returns a typed record array validated by zod.
- Every sink supports `upsert(records, key)` with the canonical key shape.
- Money as `Decimal` (decimal.js) in internal math; convert to cents-int only at sink boundary.
````

---

## memory-bank/core_docs.md

````markdown
# Core docs — billing-pipeline

| Symbol | Module | Signature | Purpose | Added by |
|---|---|---|---|---|
| _(none yet)_ | | | | |
````

---

## Language-specific notes

- **Build:** `tsc -b`.
- **Test:** `vitest run`.
- **Money:** `decimal.js` — string input, `Decimal` arithmetic, string output. Never `number`.
- **Stripe SDK:** official `stripe` npm package. Pin the API version explicitly.
- **Parquet:** `parquetjs-lite` is the most usable Node parquet writer; alternatively export CSV if Parquet isn't required.
- **Secrets:** Stripe keys via `process.env.STRIPE_API_KEY`. Never check in.
- **Type strictness:** `"strict": true` + `"noUncheckedIndexedAccess": true` in `tsconfig.json`.
