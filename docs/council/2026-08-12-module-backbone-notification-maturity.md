<!--
date: 2026-08-12
repo_type: module
unit: backbone-notification
focus: maturity
roster: chair (subagent), skeptic (subagent), steelman (subagent), yagni-business (in-context), ddd-bounded-context (in-context), contract-seat (in-context), domain-expert (invited, in-context)
-->

# Council — module:backbone-notification — focus: maturity / completeness

## Best call

**Wire `NotificationWriteService` into the `NotificationModule` struct + builder (field, `build()` construction from the shared pool, accessor) and re-point the doc example in `src/lib.rs:46-53` from `all_crud_routes()` to `readonly_routes()` + the write engine's verbs.**

This is the single move that closes the two real failures against the Metaphor-module bar. The engine itself is mature (correct, ports-and-adapters clean, regen-safe, tested, documented) but it is unreachable through the sanctioned integration surface: `NotificationModule::builder().build()` constructs only the two generic-CRUD services (`src/lib.rs:140-156`), the doc example sends consumers to bare CRUD on an append-only derived table, and the published `exports/services.rs` is template-emptied. The Metaphor module contract (`CLAUDE.md`: "Exposes a `{Domain}Module` struct built via `builder()` that wires **all services**") is not met while the one service that delivers the stated domain purpose is absent from the struct. Wiring is cheap because the engine already self-constructs from a `PgPool` (`NotificationWriteService::new(pool)`, `notification_write_service.rs:74-78`) — the `CommunicationPort` / `NotificationEventSink` are per-call args, correctly remaining the composing service's concern.

- **Residual negative value:** ~2-4 hours (one field, one construction line, one accessor, one doc block). The engine still publishes `NotificationDispatched`/`Failed` only to the in-process sink while `record_delivery` stages to the outbox in-tx (`notification_write_service.rs:171-179` vs `:280-286`) — but that asymmetry is deferred-by-design (P2, marked at `:271`), not caused by this move and not made worse by it. The ambient-scope contract on `dispatch_pending`/`record_delivery` (caller MUST wrap in `with_company_scope`) becomes a documented boundary obligation — it is already true, just unwritten at the module edge.
- **Reversibility:** easy. Additive API change; no existing CRUD service or route signature changes. Reverting is deleting one field + one accessor + reverting a doc block.
- **What would flip this:** evidence that no consumer will ever assemble through `NotificationModule::builder()` — i.e., every consumer constructs `NotificationWriteService::new(pool)` directly, as the tests do today. Per the module type contract in `CLAUDE.md`, the struct IS the integration point, so flipping requires repudiating that contract or proving the module type is vestigial. Neither is in evidence.

## Disagreement map

- **Library-boundary vs module-struct maturity bar** — Steelman grades the module complete at the library boundary (engine is correct, tested, documented, regen-safe). Contract-seat + skeptic grade it at the sanctioned entry point: the struct surfaces only bare CRUD and the real capability is internal. **Crux:** does the Metaphor-module bar require the domain capability to be wired into `{Domain}Module`? The contract says yes ("wires all services"). I side with the contract-seat. The library pieces being mature does not make the module complete when its entry point surfaces the wrong capability.

- **Notification entity: derived append-only vs hand-createable master data** — ddd-bounded-context shows the schema models Notification as a derived dispatch record (idempotency unique index on `event_id, recipient_address`; description "Fan-out creates one row per recipient"), yet `create_notification_write_routes` (`notification_handler.rs:140`) exposes generic create/update/patch/delete on it. NotificationTemplate IS legitimately master data. **Crux:** should the module expose generic writes on a derived append-only table at all? No — the handler's own doc block (`:134-139`) half-admits this ("bypass all business invariants"). The wiring fix (Best call) subsumes this: once the doc points at `readonly_routes()` + the engine, the Notification write routes are demoted to the already-deprecated `routes()` path.

- **Appropriate complexity vs over-build** — yagni-business flags the 6-state machine + pending reaper, the dead `event_store/` (full event-sourcing infra: `append`/`load`/`load_from`/`expected_version` for an insert+update write service — confirmed unwired: referenced only in its own two files), and a bespoke 4-file API-versioning stack with `openapi`/`grpc`/`proto` all DISABLED. Steelman counters idempotency+outbox earns its keep (double-notify is correctness). **Crux:** is the complexity earning its keep against a real requirement? The idempotency unique-index and outbox ARE; the event_store and API-versioning stack are NOT (no consumer in-tree). This is a YAGNI debit, not a completeness blocker — parked.

## Recommendations (ranked by leverage)

| # | Move | Leverage | Residual negative | Reversibility | Evidence to flip |
|---|------|----------|-------------------|---------------|------------------|
| 1 | **Wire `NotificationWriteService` into `NotificationModule` + re-point doc to `readonly_routes()` + engine** (Best call) | high | Engine's in-process-only event publish for notify/dispatch_pending remains (deferred P2, not worsened). ~2-4h effort. | easy | Proof the module struct is vestigial (every consumer builds the engine directly) — none in evidence |
| 2 | Drop `create_notification_write_routes` from the Notification surface (keep read + engine only); keep full CRUD on NotificationTemplate (it IS master data) | high | Slight breaking change if any consumer mounted Notification write routes — but `routes()` is already `#[deprecated]` and docs say "Not a contract" | easy | A consumer is found to legitimately hand-create Notification rows (contradicts the schema's own append-only description) |
| 3 | Publish the write engine's types (`NotifyEvent`, `Recipient`, `CommunicationPort`, `NotifyOutcome`) via `exports/services.rs` so siblings depend on the real capability, not the deleted query service | med | Maintenance surface: exported types become semver-bound. Low: the types are already stable (tests depend on them). | easy | No sibling module will ever call this engine (it is only ever driven from the composing backend-service's event subscriptions) |
| 4 | Add a fail-loud guard: panic or `Err` when `EntityRepoMeta::company_field()` returns `None` on a multi-tenant scope path (skeptic's surviving point) | med | One extra runtime check per scoped query. Negligible. | easy | Evidence the codegen can never emit `company_field() = None` for a `company_id`-bearing entity — the skeptic's falsified kill-shot showed it currently always emits `Some` |
| 5 | Delete the dead `event_store/` + API-versioning stack (4 files) or gate them behind feature flags until a consumer exists | low-med | Lost scaffolding if event-sourcing is adopted later. Recoverable from git. | easy (delete) / one-way (un-gate later) | A planned consumer for either subsystem within the next quarter |

## Maturity scorecard

| Seat | Axis | Score (1-5) | One sentence why |
|------|------|-------------|------------------|
| ddd-bounded-context | Context coherence | 3 | Engine is a clean derived append-only context, but the CRUD entry point collapses Notification (derived) with NotificationTemplate (master data) behind one module struct. |
| contract-seat | Published-contract completeness at boundary | 2 | `exports/services.rs` is template-emptied (the one published capability was deleted); the real capability (fan-out engine) is internal; the doc example points siblings at bare CRUD. |
| domain-expert | Domain fidelity / invariant enforcement | 3 | Idempotency unique-index on `(event_id, recipient_address)` is correct in intent but `recipient_address` is un-normalized free text (`string max=200`) so the same human with two address forms double-notifies. |
| yagni-business | Appropriate complexity / leverage vs over-build | 3 | Idempotency + outbox + pending-reaper earn their keep (double-notify is correctness); the dead `event_store/` (unwired) and bespoke API-versioning stack (with openapi/grpc/proto disabled) do not. |
| skeptic | Adversarial correctness / fail-loud safety | 3 | The regen-erase kill-shot was falsified (`company_field()` is template-emitted and survives), but the surviving point stands: tenant isolation rests on one function with no fail-loud fallback if it ever returns `None` — silent cross-tenant degradation. |

## Parking lot

- **`recipient_address` normalization** (domain-expert) — idempotency leaks at the normalization seam (`+62 812` vs `+62812`, case-variant email). Scope: domain correctness, not completeness. Park for a domain-correctness council.
- **Asymmetric outbox durability for `notify`/`dispatch_pending`** (steelman gap #4) — `NotificationDispatched`/`Failed` publish to in-process sink only while `record_delivery` stages to outbox in-tx. Already marked P2 in-code (`notification_write_service.rs:271`). Scope: delivery correctness, deferred by design.
- **`SubscriptionRegistry` is a bare alias with no implementor** (`registry.rs`) — "subscribes to domain events" is a capability, not runtime behavior in-module. Scope: composing-service concern; park.
- **Dead `event_store/` event-sourcing infra and API-versioning stack** (yagni-business) — unwired complexity. Scope: complexity reduction, not completeness. Captured as recommendation #5 but full deletion vs feature-gating is a separate call.

Key files of record: `src/lib.rs` (module struct + doc example), `src/application/service/notification_write_service.rs` (the engine), `src/exports/services.rs` (empty contract), `src/presentation/http/notification_handler.rs:140` (boundary-violating write routes), `schema/models/notification.model.yaml` (SSoT confirming derived append-only), `src/domain/entity/notification.rs:297` (`company_field()` with no fail-loud guard), `src/infrastructure/event_store/event_store.rs` (dead infra).
