# ADR-001 — Templated fan-out, per-recipient idempotency, and the pending reaper

Status: accepted · 2026-07-08 · Channels pillar (Tier 5; posts no GL)

## Context
Every module needs to notify the customer (order confirmed, invoice due, SLA breached). That is one shared
outbound context. `backbone-notification` subscribes to domain events, renders a template per recipient,
and dispatches through the channel gateway (backbone-communication).

## Decision
1. **Idempotency is the notification row's natural business key.** One `Notification` per `(event_id,
   recipient_address)`, enforced by a DB unique index and claimed with `INSERT … ON CONFLICT DO NOTHING
   RETURNING id`. A redelivered domain event (at-least-once) dedups per recipient — no double-notify — and
   a partially-processed event re-attempts only the unclaimed recipients. The row IS the inbox record.
2. **Dispatch is a port, not a dependency.** Notification renders; the actual WhatsApp/email/SMS send is
   backbone-communication's job, reached through `CommunicationPort`. Zero Cargo edge — proven by NSEAM-1
   landing a real outbound message in the REAL communication module.
3. **`pending` is recoverable, not terminal.** The slot commits as `pending` before dispatch; a crash
   between the commit and the dispatch would otherwise strand the row forever while the dedup slot blocks
   any redelivery from ever sending it. `dispatch_pending` re-drives `pending` rows (recovery keyed on
   STATE), and every dispatch carries the notification's `idempotency_key` so the re-drive can't
   double-notify when the gateway honors the token (maturity council 2026-07-08).
4. **Templates are flat.** `{{placeholder}}` substitution from the event data — no logic in templates; the
   Indonesia/business content is the template author's concern.
5. **Posts no GL.** Notification is a terminal consumer; it emits only `NotificationDispatched`/`Failed`
   for audit.

## Consequences
- Turn notification off and no outbound messages fan out; it is the one place domain events become
  customer messages. The channel gateway decouples it from the provider.
- Proven against the REAL communication module (NSEAM-1) and recoverable across a crash (NIP-5); survives
  regen (§5).

## Parking lot (each with a gate)
- **`sent` overstated success — no delivery reconciliation** — FIXED (completeness council 2026-07-08):
  notification persisted `message_id` but never consumed communication's `MessageDelivered`/`MessageFailed`,
  so `sent` (handed to the gateway) was indistinguishable from bounced and delivery-based escalation was
  unimplementable; fixed with `delivered`/`undelivered` statuses + a `record_delivery(message_id, outcome)`
  verb + `NotificationDelivered`/`NotificationUndelivered` events (NGC-5, proven-by-revert).
- **`pending` strand → silent never-notified** — FIXED (maturity council 2026-07-08): a crash between the
  committed `pending` INSERT and the dispatch stranded the row forever while the dedup slot blocked
  redelivery; fixed with a state-based `dispatch_pending` reaper + an `idempotency_key` on the dispatch so
  the re-drive can't double-notify (NIP-5, proven-by-revert).
- **Gateway must dedup on the idempotency key** — the double-notify guard depends on communication deduping
  outbound on the token. Gate: outbound dedup in communication.
- **Transient `failed` not auto-retried** — the reaper targets `pending`; `failed` needs bounded retry with
  an attempt counter. Gate: a retry-with-backoff policy.
- **Recipient preferences / opt-out, rich templating, campaign automation** — deferred (PRD non-goals).
