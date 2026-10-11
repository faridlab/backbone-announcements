//! The module's extension (hand-written; ADR-0031): what the module adds beside its
//! generated services. The generated module and builder carry `ModuleExt` and
//! `ModuleBuilderExt` and dereference to them, so their fields read as `module.field`.

#[allow(unused_imports)]
use super::*;

/// State the module adds; read through the generated module's `Deref`.
pub struct ModuleExt {
    /// The validated write engine — the lifecycle verbs (schedule / publish /
    /// archive) and the read-mark upsert. Generic CRUD on an announcement row
    /// bypasses the status machine; use this for every state change.
    pub(crate) write_service: Arc<application::service::AnnouncementWriteService>,
}

/// State the builder adds.
pub struct ModuleBuilderExt {
}

impl Default for ModuleBuilderExt {
    fn default() -> Self {
        Self {
        }
    }
}

impl ModuleBuilderExt {
    /// Build the extension's state from what the generated build made.
    #[allow(unused_variables, clippy::redundant_clone)]
    pub(crate) fn build(self, parts: &ModuleParts<'_>) -> anyhow::Result<ModuleExt> {
        let db_pool = parts.db_pool.clone();
        // The validated write engine. Self-constructs from the pool; the event
        // sink stays per-call (the composing service's concern).
        let write_service = Arc::new(application::service::AnnouncementWriteService::new(db_pool.clone()));
        Ok(ModuleExt {
            write_service,
        })
    }
}

impl crate::ThisModule {
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
}
