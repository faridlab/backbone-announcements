# backbone-notification — FSD

## Entities
The module carries no tenant column or fence of its own (composition-installed tenancy, ADR-0029): a
composing backend-service that wants these tables org-scoped installs `org_unit_id`, the RLS policy, and
per-unit uniques via its tenancy decorator.

NotificationTemplate (`event_type`, `channel`, `name`, `subject_template?`, `body_template`,
`status`; no module-level unique — a composing service's decorator may add a per-unit
`(event_type, channel)` one) · Notification (`event_id` logical,
`event_type`, `template_id?` logical, `channel`, `recipient_party_id?` logical, `recipient_address`,
`subject?`, `body`, `status`, `message_id?` logical, `failure_reason?`; unique `(event_id,
recipient_address)` — the idempotency guard). Enums:
NotifChannel {whatsapp, email, sms}, NotificationStatus {pending, sent, delivered, undelivered, failed,
skipped} (`sent`=handed to the gateway; `delivered`/`undelivered`=the provider's confirmed outcome;
`failed`=gateway rejected the hand-off).

## Write path (`NotificationWriteService`, hand-authored, user-owned)
- `create_template(NewTemplate)` → define the active template for an (event_type, channel); replace any
  existing one for the key
- `notify(NotifyEvent, &dyn CommunicationPort, &dyn NotificationEventSink)` → resolve template, render per
  recipient, claim the `(event_id, recipient_address)` dedup slot, dispatch, mark sent/failed; returns
  `NotifyOutcome {dispatched, deduped, failed, skipped}`
- `dispatch_pending(limit, port, sink)` → the reaper: re-drive notifications stranded `pending` (recovery
  keyed on state), carrying each notification's idempotency key so the re-drive can't double-notify
- `record_delivery(message_id, outcome, sink)` → close the loop with the provider's real outcome
  (`sent → delivered | undelivered`), correlated by `message_id`, state-guarded/idempotent
- `render` → flat `{{placeholder}}` substitution from the event `data`

Errors: `NotifyError {Db, Invalid, NoCompanyScope}` — `NoCompanyScope` fires when a lifecycle event must
be staged to the company-keyed outbox mirror and the ambient org scope carries no company node (the
mirror's key fails closed rather than being guessed).

## Seams (ports — zero normal Cargo edge)
- **Dispatch → communication (proven, NSEAM-1):** the rendered message is handed to backbone-communication
  through `CommunicationPort` (implemented over `send_outbound`), landing a real outbound Message; the
  returned `message_id` is persisted for reconciliation. `DispatchRequest` carries an `idempotency_key`.
- **Inbound:** notification subscribes to domain events (order confirmed, invoice due, SLA breached) — the
  composing service calls `notify` on each.
- **Outbound events:** `NotificationDispatched`/`NotificationFailed` for audit.

## Test oracle
`notification_golden_cases` (5: NGC-1 fan-out per recipient, NGC-2 template render, NGC-3 idempotent per
event+recipient, NGC-4 no-template skip, NGC-5 record_delivery closes the delivered/undelivered loop),
`integrity_probes` (5: NIP-1 recipient needs address, NIP-2 one template per event/channel, NIP-3 dispatch
rejection recorded, NIP-4 dedup per-recipient, NIP-5 the reaper re-drives a stranded pending),
`notification_communication_seam` (1: NSEAM-1 notification lands a REAL communication message) + §5
round-trip. **11 tests.**

> The generated `integration_tests.rs` hits an external HTTP server and is environmental scaffolding, not
> part of this module's correctness gate.
