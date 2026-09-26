# backbone-notification — business flows & golden cases

## Flow: domain event → template → fan-out → dispatch (idempotently, recoverably)
```
create_template (per event_type/channel, {{placeholder}} body)
   │
   ▼  notify(event, recipients, data)  [domain event, at-least-once]
   │     ├─ resolve active template — none → skipped (recorded, nothing sent)
   │     └─ per recipient: render → claim (event_id, recipient) dedup slot (pending)
   │           ├─ conflict → deduped (redelivery, no double-notify)
   │           └─ new → dispatch via CommunicationPort → sent(+message_id) / failed(+reason)
   │
   └▶ dispatch_pending(limit)  [scheduled reaper] → re-drive rows stuck 'pending' → sent
```
Every dispatch carries the notification's `idempotency_key` so a re-drive can't double-notify. Posts NO GL.

## Golden cases (`tests/notification_golden_cases.rs`)
- **NGC-1 — fan-out per recipient.** Two recipients of one event → two dispatches.
- **NGC-2 — template renders placeholders.** `Halo {{name}}, pesanan {{order_no}} dikonfirmasi.` +
  `{name:Budi, order_no:SO-1001}` → `Halo Budi, pesanan SO-1001 dikonfirmasi.`
- **NGC-3 — idempotent per (event, recipient).** The same event_id redelivered → deduped, one dispatch.
- **NGC-4 — no template skips.** A recipient with no active template → skipped, nothing sent.
- **NGC-5 — record_delivery closes the loop.** Two `sent` recipients → `record_delivery` reconciles one
  `delivered` + one `undelivered` from the provider's receipts; a redelivered receipt is a no-op. So a
  consumer can tell delivered from bounced and escalate on undelivered.

## Integrity probes (`tests/integrity_probes.rs`)
- **NIP-1 — recipient needs an address.**
- **NIP-3 — dispatch rejection recorded.** A rejection persists a `failed` row + reason (not swallowed).
- **NIP-4 — dedup is per-recipient.** A different recipient on the same event still sends.
- **NIP-5 — the reaper re-drives a stranded pending.** A notification stuck `pending` (crash-after-claim)
  → `dispatch_pending` sends it (`sent`), carrying its idempotency key. Proven-by-revert.

(The former NIP-2 — one template per (event, channel), duplicate refused — retired with the
composition-installed tenancy strip, ADR-0029: the module tables carry no such unique; a composing
service's tenancy decorator may declare a per-unit one.)

## Seam (`tests/notification_communication_seam.rs`)
- **NSEAM-1 — dispatches a REAL communication message.** `notify` → the REAL backbone-communication
  `send_outbound` lands an outbound Message (sent, carrying the rendered body); the notification records
  the communication `message_id`. Zero normal Cargo edge.

## §5 round-trip (`scripts/notification_communication_seam_roundtrip.sh`)
Regen (`--force`) leaves the seam files (`notification_ports.rs`, `notification_events.rs`,
`notification_write_service.rs`) byte-identical; the oracle + seam re-run green.
