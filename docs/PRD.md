# backbone-notification — PRD

Channels pillar (Tier 5) · outbound **templated fan-out** · posts **no GL** · dispatches via
backbone-communication.

## Why
Every module needs to tell the customer something — order confirmed, invoice due, SLA breached. That is
one shared outbound context, not per-module send code. `backbone-notification` subscribes to domain
events, renders a template per recipient, and dispatches each through the **channel gateway**
(backbone-communication), **idempotent per (event_id, recipient)** so an at-least-once event never
double-notifies.

## Scope (KEEP — tier5-deferred.md §4)
- **NotificationTemplate** — a reusable template keyed by (event_type, channel), with a
  `{{placeholder}}` body rendered from the event's data.
- **Notification** — one rendered notification per recipient of one event; the row is the **idempotency
  record** (unique `(event_id, recipient_address)`).
- **The fan-out engine** — `notify(event, recipients, data)` resolves the template, renders per recipient,
  and dispatches through the `CommunicationPort`; a redelivered event dedups (no double-notify); a
  recipient with no active template is skipped.
- **Dispatch through communication** — the actual WhatsApp/email/SMS send is the channel gateway's job;
  notification hands it a rendered message and records the returned `message_id`.

## Non-goals (CUT / DEFER — tier5-deferred.md §4)
- Full omnichannel **campaign / marketing automation** (explicitly cut for SMB), audience segmentation,
  A/B testing.
- The concrete channel provider — that lives in backbone-communication behind its own port.
- Recipient **preferences / opt-out** management (a later addition; the fan-out is the core).
- Rich templating (loops, conditionals) — a flat `{{placeholder}}` substitution is enough for
  transactional notifications.

## Success criteria
- A domain event notifies each recipient **exactly once** even under at-least-once redelivery.
- A rendered notification lands a real outbound message in backbone-communication (proven against the REAL
  module).
- Zero normal Cargo edge; survives a full codegen regen (§5). Posts no GL.
