//! Announcements Module
//!
//! HR broadcast notices targeted by org unit, with a publish window, read
//! tracking, and one tray notification per targeted user at publish.
//!
//! Generated scaffold (entities, repositories, CRUD services, handlers) plus a
//! hand-authored write service that owns the lifecycle verbs. Composition
//! guidance: read-only CRUD for listing/admin, the write service for every
//! state change, the self lane served by the composing service.

#![recursion_limit = "1024"]
#![allow(unused_imports)]

pub mod domain;
pub mod infrastructure;
pub mod application;
pub mod presentation;
pub mod seeders;
pub mod exports;
// <<< CUSTOM MODULES
pub mod request_pool;
// END CUSTOM

// Re-exports for convenience - Domain entities
pub use domain::entity::*;

// Re-exports - Infrastructure
pub use infrastructure::persistence::*;

// Re-exports - Application services
pub use application::service::AnnouncementService;
pub use application::service::AnnouncementReadService;

use std::sync::Arc;
use axum::Router;
use sqlx::PgPool;

/// Announcements module handle
///
/// ```text
/// let announcements = AnnouncementsModule::builder()
///     .with_database(pool.clone())
///     .build()?;
///
/// let router = announcements.guarded_routes();
/// ```
pub struct AnnouncementsModule {
    pub(crate) announcement_service: Arc<AnnouncementService>,
    pub(crate) announcement_read_service: Arc<AnnouncementReadService>,
    // <<< CUSTOM FIELDS
    /// The validated write engine — the lifecycle verbs (schedule / publish /
    /// archive) and the read-mark upsert. Generic CRUD on an announcement row
    /// bypasses the status machine; use this for every state change.
    pub(crate) write_service: Arc<application::service::AnnouncementWriteService>,
    // END CUSTOM
}

impl AnnouncementsModule {
    /// Create a new module builder
    pub fn builder() -> AnnouncementsModuleBuilder {
        AnnouncementsModuleBuilder::new()
    }

    /// Mount ALL generated CRUD endpoints with NO domain validation — the
    /// fully unguarded surface. Prefer a guarded composition for any real
    /// deployment; use this only in trusted/admin/seeding contexts.
    pub fn all_crud_routes(&self) -> Router {
        use presentation::http::{
            create_announcement_routes,
            create_announcement_read_ledger_routes,
        };

        Router::new()
            .merge(create_announcement_routes(self.announcement_service.clone()))
            .merge(create_announcement_read_ledger_routes(self.announcement_read_service.clone()))
    }

    /// Deprecated alias for [`Self::all_crud_routes`].
    #[deprecated(note = "mounts unvalidated generic CRUD; prefer readonly_routes() + validated writes, or all_crud_routes() for the full/unguarded surface")]
    pub fn routes(&self) -> Router {
        self.all_crud_routes()
    }

    /// Read-only routes for every entity (GET endpoints only) — the safe base.
    pub fn readonly_routes(&self) -> Router {
        use presentation::http::{
            create_announcement_read_routes,
            create_announcement_read_ledger_read_routes,
        };

        Router::new()
            .merge(create_announcement_read_routes(self.announcement_service.clone()))
            .merge(create_announcement_read_ledger_read_routes(self.announcement_read_service.clone()))
    }

    // <<< CUSTOM METHODS
    /// The validated write engine (schedule / publish / archive, read-mark).
    pub fn write_service(&self) -> Arc<application::service::AnnouncementWriteService> {
        self.write_service.clone()
    }

    /// The recommended production surface: read-only CRUD for both entities +
    /// the guarded verb routes (draft create/update stay generic writes on a
    /// draft row; schedule/publish/archive and the read mark go through the
    /// write service only).
    pub fn guarded_routes(&self) -> Router {
        use presentation::http::guarded_routes::create_guarded_announcement_routes;

        Router::new()
            .merge(self.readonly_routes())
            .merge(create_guarded_announcement_routes(self.write_service.clone()))
    }
    // END CUSTOM
}

/// Builder for AnnouncementsModule
pub struct AnnouncementsModuleBuilder {
    db_pool: Option<PgPool>,
}

impl AnnouncementsModuleBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            db_pool: None,
        }
    }

    /// Set the database connection pool
    pub fn with_database(mut self, pool: PgPool) -> Self {
        self.db_pool = Some(pool);
        self
    }

    // <<< CUSTOM - custom builder methods
    // END CUSTOM

    /// Build the module with configured dependencies
    pub fn build(self) -> anyhow::Result<AnnouncementsModule> {
        let db_pool = self.db_pool
            .ok_or_else(|| anyhow::anyhow!("Database pool not configured"))?;

        let announcement_repository = Arc::new(AnnouncementRepository::new(db_pool.clone()));
        let announcement_service = Arc::new(AnnouncementService::with_repository(announcement_repository.clone()));

        let announcement_read_repository = Arc::new(AnnouncementReadRepository::new(db_pool.clone()));
        let announcement_read_service = Arc::new(AnnouncementReadService::with_repository(announcement_read_repository.clone()));

        // <<< CUSTOM
        // The validated write engine. Self-constructs from the pool; the event
        // sink stays per-call (the composing service's concern).
        let write_service = Arc::new(application::service::AnnouncementWriteService::new(db_pool.clone()));
        // END CUSTOM

        Ok(AnnouncementsModule {
            announcement_service,
            announcement_read_service,
            // <<< CUSTOM
            write_service,
            // END CUSTOM
        })
    }
}

impl Default for AnnouncementsModuleBuilder {
    fn default() -> Self {
        Self::new()
    }
}
