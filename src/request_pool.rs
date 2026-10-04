//! The composer-installed request pool, made visible to the write service.
//!
//! The module is tenant-agnostic and the composing service owns routing to
//! the right database. The composer's tenant router inserts the request's
//! `PgPool` into the request extensions; [`bind_request_pool`] copies it into
//! a task-local for the duration of the handler future, and the write service
//! resolves its database through [`current`] with its composed pool as the
//! fallback. A mount without tenant routing behaves exactly as before; a verb
//! under a tenant mount reads and writes that tenant's database.
//!
//! Work that runs outside a request (a scheduled publish pass over one tenant
//! database, say) binds the pool itself with [`with_pool_scope`].

use axum::{body::Body, http::Request, middleware::Next, response::Response};
use sqlx::PgPool;

tokio::task_local! {
    static REQUEST_POOL: PgPool;
}

/// The pool this request's tenant router installed, if any.
pub fn current() -> Option<PgPool> {
    REQUEST_POOL.try_with(|pool| pool.clone()).ok()
}

/// Middleware that binds the composer-inserted pool for the whole handler
/// call. Requests without an inserted pool pass straight through.
pub async fn bind_request_pool(req: Request<Body>, next: Next) -> Response {
    match req.extensions().get::<PgPool>().cloned() {
        Some(pool) => REQUEST_POOL.scope(pool, next.run(req)).await,
        None => next.run(req).await,
    }
}

/// Run a future with a pool bound as this module's request pool, so every
/// write-service call inside it resolves that database.
pub async fn with_pool_scope<F: std::future::Future<Output = O>, O>(
    pool: PgPool,
    fut: F,
) -> O {
    REQUEST_POOL.scope(pool, fut).await
}
