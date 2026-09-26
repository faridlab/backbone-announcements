//! Guarded route composition — the recommended way to mount the announcements
//! module's write surface (hand-authored, user-owned).
//!
//! Reads stay on the generated GET endpoints (merged by
//! `AnnouncementsModule::guarded_routes`); every state change goes through a
//! write-service verb. The read mark (an employee action, not admin) also
//! lives here so a composing service can mount it on the self lane with its
//! own org verifier.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use backbone_auth::org::OrgContext;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::service::{
    AnnouncementError, AnnouncementWriteService, NewAnnouncement,
};

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

fn status_of(code: u16) -> StatusCode {
    StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

fn err_response(e: AnnouncementError) -> axum::response::Response {
    (
        status_of(e.http_status()),
        Json(ErrorBody { error: e.code(), message: e.to_string() }),
    ).into_response()
}

#[derive(Debug, Deserialize)]
struct CreateAnnouncementBody {
    title: String,
    body: String,
    /// "company" | "branch" | "department"
    audience_type: String,
    audience_unit_id: Uuid,
    publish_from: Option<DateTime<Utc>>,
    #[serde(default)]
    publish_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pinned: bool,
}

async fn create_announcement(
    State(svc): State<Arc<AnnouncementWriteService>>,
    org: OrgContext,
    Json(b): Json<CreateAnnouncementBody>,
) -> axum::response::Response {
    match svc
        .create_draft(NewAnnouncement {
            title: b.title,
            body: b.body,
            audience_type: b.audience_type,
            audience_unit_id: b.audience_unit_id,
            publish_from: b.publish_from.unwrap_or_else(Utc::now),
            publish_until: b.publish_until,
            pinned: b.pinned,
            created_by: Uuid::parse_str(&org.user_id).ok(),
        })
        .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "id": id })),
        )
            .into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Default, Deserialize)]
struct UpdateDraftBody {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    publish_from: Option<DateTime<Utc>>,
    #[serde(default)]
    publish_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pinned: Option<bool>,
}

async fn update_draft(
    State(svc): State<Arc<AnnouncementWriteService>>,
    _org: OrgContext,
    Path(id): Path<Uuid>,
    body: Option<Json<UpdateDraftBody>>,
) -> axum::response::Response {
    let b = body.map(|Json(b)| b).unwrap_or_default();
    match svc.update_draft(id, b.title, b.body, b.publish_from, b.publish_until, b.pinned).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => err_response(e),
    }
}

async fn schedule_announcement(
    State(svc): State<Arc<AnnouncementWriteService>>,
    _org: OrgContext,
    Path(id): Path<Uuid>,
) -> axum::response::Response {
    match svc.schedule(id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "scheduled" }))).into_response(),
        Err(e) => err_response(e),
    }
}

async fn publish_announcement(
    State(svc): State<Arc<AnnouncementWriteService>>,
    _org: OrgContext,
    Path(id): Path<Uuid>,
) -> axum::response::Response {
    match svc.publish_now(id).await {
        Ok(o) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "published",
                "targetedCount": o.targeted_count,
                "already": o.already,
            })),
        )
            .into_response(),
        Err(e) => err_response(e),
    }
}

async fn archive_announcement(
    State(svc): State<Arc<AnnouncementWriteService>>,
    _org: OrgContext,
    Path(id): Path<Uuid>,
) -> axum::response::Response {
    match svc.archive(id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "status": "archived" }))).into_response(),
        Err(e) => err_response(e),
    }
}

/// The read mark — deliberately extractable WITHOUT an OrgContext: the self
/// lane mounts it under the composing service's own identity verifier, and
/// the user id comes from that verifier's state, never the body. Mounted on
/// the guarded composer it requires the org context like the others.
async fn mark_read(
    State(svc): State<Arc<AnnouncementWriteService>>,
    _org: Option<OrgContext>,
    Path(id): Path<Uuid>,
) -> axum::response::Response {
    // The caller-derived user: the OrgContext when present, else the
    // composing service must wrap this handler with its own identity
    // extraction (the self lane re-uses the verb, not this route).
    let Some(org) = _org else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody { error: "unauthorized", message: "no verified identity".into() }),
        )
            .into_response();
    };
    let Ok(user_id) = Uuid::parse_str(&org.user_id) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody { error: "unauthorized", message: "the verified principal is not a user id".into() }),
        )
            .into_response();
    };
    match svc.mark_read(id, user_id).await {
        Ok(read) => (StatusCode::OK, Json(serde_json::json!({ "read": read }))).into_response(),
        Err(e) => err_response(e),
    }
}

/// The verb routes. Combine with read-only CRUD (see
/// `AnnouncementsModule::guarded_routes`).
pub fn create_guarded_announcement_routes(svc: Arc<AnnouncementWriteService>) -> Router {
    Router::new()
        .route("/announcements", post(create_announcement))
        .route("/announcements/:id", axum::routing::patch(update_draft))
        .route("/announcements/:id/schedule", post(schedule_announcement))
        .route("/announcements/:id/publish", post(publish_announcement))
        .route("/announcements/:id/archive", post(archive_announcement))
        .route("/announcements/:id/read", post(mark_read))
        .with_state(svc)
}
