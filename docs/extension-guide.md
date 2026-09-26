# backbone-notification — Extension Guide

## Public surface (stable)
- **Dispatch port** (`application::service::notification_ports`): `CommunicationPort` + DTOs
  (`DispatchRequest`, `DispatchAck`, `DispatchRejected`) — the seam a composing service implements over
  backbone-communication's `send_outbound`. Notification never imports communication.
- **Write path** (`application::service::notification_write_service::NotificationWriteService`):
  `create_template`, `notify` (the fan-out engine), `dispatch_pending` (the retry reaper),
  `record_delivery` (closes the loop on a `sent` notification with the provider's real delivery
  outcome), plus DTOs (`NewTemplate`, `NotifyEvent`, `Recipient`, `NotifyOutcome`, `NotifyError`,
  `DeliveryOutcome`).
- **Events** (`application::service::notification_events`): `NotificationDispatched`,
  `NotificationFailed`, the `NotificationEvent` union, and `NotificationEventSink` — a terminal
  observability surface.

## How a consuming service uses notification
Author templates per (event_type, channel) with `{{placeholder}}` bodies. On a domain event, call
`notify(NotifyEvent { event_id, event_type, channel, recipients, data }, port, sink)` — pass the event's
stable id as `event_id` so redelivery dedups. Implement `CommunicationPort::dispatch` over
backbone-communication, forwarding `DispatchRequest.idempotency_key` as a provider-level dedup token so a
retry never double-sends. Run `dispatch_pending` on a schedule to recover notifications left `pending` by
a crash between the slot claim and the dispatch. When backbone-communication emits a delivery receipt
(`MessageDelivered` / `MessageFailed`), call `record_delivery(message_id, DeliveryOutcome::…, sink)` so a
`sent` notification can transition to `delivered` / `undelivered` and a delivery-driven escalation
(undelivered → retry another channel) becomes implementable.

## Who wires what: subscriptions are NOT this module's job
`backbone-notification` is a **terminal engine**, not a subscriber. It owns no event bus, no poller, no
background task — the `SubscriptionRegistry` you may see in-tree is a bare type alias with no
implementor, a placeholder only. Subscribing to the domain events that should trigger notifications is
the **composing backend-service's** responsibility. That service:

1. Subscribes to whichever domain events it wants fanned out (e.g. `OrderConfirmed`, `InvoiceOverdue`) on
   its own bus / outbox drain.
2. Translates each into a `NotifyEvent`, carrying the source event's stable id as `event_id` (the dedup
   key).
3. Calls `NotificationWriteService::notify` (or drives `dispatch_pending` on a schedule). When the
   composing service wants the lifecycle events' company-keyed outbox mirror rows stamped with its
   tenant, it binds its ambient org request scope around the call; with no scope bound, staging the
   lifecycle event fails closed (`NoCompanyScope`) rather than guessing a key.
4. Supplies the `CommunicationPort` over backbone-communication and a `NotificationEventSink`.

This keeps the module free of runtime/plumbing choices (which bus, which polling cadence, which tenant a
scheduler runs under) that belong to the service that assembles it. The module's contract is the engine;
the subscription wiring is the host's.

## Not a contract
- The 12 generated CRUD endpoints per entity are convenience scaffolding. Do **not** insert a notification
  or flip a status through the generic PATCH surface — it bypasses the (event_id, recipient) dedup and the
  dispatch/retry gating. Use `NotificationWriteService`.
- `// <<< CUSTOM` blocks preserve local edits only; not a cross-module extension point.

## Invariants a consumer must not break
- One notification per `(event_id, recipient_address)`; a redelivered event never double-notifies.
- `recipient_address` is canonicalized by channel **before** the dedup claim: phone channels (sms /
  whatsapp / …) keep only an optional leading `+` and the digits; email is trimmed + lowercased (`@` and
  `.` are structural, never stripped). **Supply international phone form (`+<country><number>`)** — the
  engine strips formatting noise but will not guess a country code, so a national form (`0812 345`) and
  its international form (`+62812345`) are intentionally two distinct recipients.
- Every lifecycle event (`NotificationDispatched` / `NotificationFailed` / `NotificationDelivered` /
  `NotificationUndelivered`) is staged to the transactional outbox (`notification.outbox_events`) **in
  the same tx** as the status transition that produced it, then also published to the in-process sink.
  Drain the outbox for the durable signal — it survives a dropped in-process publish (`LoggingSink`, or a
  crash mid-publish).
- A notification is dispatched at most once *to the recipient* when the `CommunicationPort` honors the
  idempotency key; `dispatch_pending` re-drives only rows not yet `sent`.
- Rendering is flat `{{placeholder}}` substitution — templates carry no logic.
