//! Gateway adapters (ADR-0024 seam) — the module-side transports for its
//! outbound ports. Announcements ships no transport: the publish event rides
//! the composing service's integration bus.
