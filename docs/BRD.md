# backbone-notification — BRD

## Documents
NotificationTemplate (per event_type/channel) · Notification (one per recipient of one event). Own
Postgres schema `notification`. Posts **no GL**. Dispatches through backbone-communication. The module is
tenant-agnostic (composition-installed tenancy, ADR-0029): a composing service that wants org-scoped
behavior declares it in its tenancy decorator.

## Business rules

**BR-1 (template).** `create_template` defines the active template for an (event_type, channel),
with a `{{placeholder}}` body. The body is required. One active template per unit per
(event_type, channel) — where a deployment wants that posture — is the composing service's tenancy
decorator's declaration, not a module rule.

**BR-2 (fan-out — the idempotency invariant).** `notify(event, recipients, data)` resolves the active
template and, per recipient, renders and records exactly one notification, keyed **unique on (event_id,
recipient_address)**. A redelivered domain event conflicts on that key and is **deduped** — no
double-notify. Dedup is **per recipient**: a different recipient on the same event is a distinct
notification. A recipient with no active template is **skipped** (recorded in the outcome, nothing sent).

**BR-3 (dispatch).** Each new notification is dispatched through the `CommunicationPort` (backbone-
communication). On acceptance it becomes `sent` (handed to the gateway) with the communication
`message_id`; on rejection it becomes `failed` with the reason (recorded, not swallowed).
`NotificationDispatched` / `NotificationFailed` are emitted for audit.

**BR-3a (delivery reconciliation).** `sent` is not proof of receipt. `record_delivery(message_id, outcome)`
closes the loop with the provider's real outcome — `sent → delivered | undelivered` — when
backbone-communication emits `MessageDelivered`/`MessageFailed`. Correlated by `message_id`, state-guarded
on `sent` (idempotent), emitting `NotificationDelivered`/`NotificationUndelivered` so a consumer can drive
delivery-based escalation (completeness council 2026-07-08).

**BR-4 (retry reaper).** `dispatch_pending` re-drives notifications stranded `pending` by a crash between
the committed slot claim and the dispatch, carrying each notification's idempotency key so the re-drive
cannot double-notify (maturity council 2026-07-08).

**BR-5 (render).** `{{key}}` in the subject/body is substituted from the event `data` (a JSON object); an
unknown key renders empty. Flat substitution only.

## Events
`NotificationDispatched` (notification_id, event_id, message_id), `NotificationFailed` (notification_id,
event_id, reason), `NotificationDelivered` (notification_id, event_id), `NotificationUndelivered`
(notification_id, event_id, reason) — the observability surface; `Delivered`/`Undelivered` let a consumer
drive delivery-based escalation.

## Deferred (with reason)
Campaign/marketing automation (cut for SMB), recipient preferences/opt-out, rich templating, the concrete
channel provider (→ backbone-communication).
