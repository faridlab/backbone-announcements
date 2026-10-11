// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

pub mod announcement_events;
pub mod announcement_write_service;
pub use announcement_events::{AnnouncementEvent, AnnouncementEventSink, LoggingSink};
pub use announcement_write_service::{
    AnnouncementError, AnnouncementWriteService, NewAnnouncement, PublishOutcome,
};
