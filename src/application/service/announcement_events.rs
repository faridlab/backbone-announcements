//! The announcements event port: what the module tells the world, and the
//! seam a composing service implements to make it durable.
//!
//! One event exists today — [`AnnouncementEvent::Published`] — fired the
//! moment an announcement becomes visible. The composing service's producer
//! arm resolves the audience ONCE per envelope (envelope-id dedup makes a
//! redelivery a no-op), which is the audience-snapshot semantics the council
//! settled: later org changes never rewrite who was notified, and
//! `targeted_count` on the row is the snapshot's persistence home.

use uuid::Uuid;

/// What the module announces to the outside.
#[derive(Debug, Clone)]
pub enum AnnouncementEvent {
    /// An announcement just became visible to its audience.
    Published {
        /// The announcement row.
        announcement_id: Uuid,
        /// The audience org unit (the producer resolves members under it).
        audience_unit_id: Uuid,
        /// The headline (enough for a tray row without a read-back).
        title: String,
        /// The membership snapshot size, stamped on the row at publish.
        targeted_count: i64,
        /// The publish moment.
        published_at: chrono::DateTime<chrono::Utc>,
    },
}

/// The event sink port. A durable composition stages the event into its
/// outbox here (same transaction posture as the sibling modules' sinks);
/// the default logs and drops.
pub trait AnnouncementEventSink: Send + Sync {
    fn publish(&self, event: AnnouncementEvent);
}

/// The default sink: logs. Nothing durable, nothing delivered — a composing
/// service that wants tray rows wires a real sink.
pub struct LoggingSink;

impl AnnouncementEventSink for LoggingSink {
    fn publish(&self, event: AnnouncementEvent) {
        match event {
            AnnouncementEvent::Published { announcement_id, targeted_count, .. } => {
                tracing::info!(
                    target: "announcements",
                    announcement_id = %announcement_id,
                    targeted_count,
                    "announcement published (no event sink wired)"
                );
            }
        }
    }
}
