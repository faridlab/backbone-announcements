//! The hand-authored announcements write path (user-owned; survives regen).
//!
//! The lifecycle: `draft → scheduled → published → archived`. Drafts are
//! editable; everything after is immutable except through the verbs. Publish
//! is the emitting moment — it stamps `targeted_count` (the audience
//! snapshot's persistence home, the read-stats denominator) and fires
//! [`AnnouncementEvent::Published`] through the sink port.
//!
//! The scheduler tick (`publish_due`) flips every `scheduled` row whose
//! `publish_from` has arrived, SKIP-locked so two ticks never double-publish.
//!
//! Tenancy (ADR-0029): the module is tenant-agnostic. Writes relay the
//! ambient org request scope onto their transactions; the audience counter
//! sets its own scope var to the audience subtree on a dedicated connection
//! (the notification-producer pattern) so the count passes the composer's
//! fence regardless of the caller's ambient state.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use super::announcement_events::{AnnouncementEvent, AnnouncementEventSink, LoggingSink};

#[derive(Debug, thiserror::Error)]
pub enum AnnouncementError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("invalid state: {0}")]
    InvalidState(&'static str),
    #[error("invalid input: {0}")]
    Invalid(String),
}

impl AnnouncementError {
    /// Stable machine code the HTTP layer surfaces.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Db(_) => "internal_error",
            Self::NotFound(_) => "not_found",
            Self::InvalidState(_) => "invalid_state",
            Self::Invalid(_) => "invalid_input",
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            Self::Db(_) => 500,
            Self::NotFound(_) => 404,
            Self::InvalidState(_) | Self::Invalid(_) => 422,
        }
    }
}

/// A new draft announcement.
pub struct NewAnnouncement {
    pub title: String,
    pub body: String,
    /// "company" | "branch" | "department"
    pub audience_type: String,
    pub audience_unit_id: Uuid,
    /// The visibility start. Now = publishable immediately; future = the tick
    /// publishes it when the moment arrives.
    pub publish_from: DateTime<Utc>,
    pub publish_until: Option<DateTime<Utc>>,
    pub pinned: bool,
    pub created_by: Option<Uuid>,
}

/// What the publish verb did.
pub struct PublishOutcome {
    pub announcement_id: Uuid,
    pub targeted_count: i64,
    /// False when the row was already published (idempotent re-run).
    pub already: bool,
}

pub struct AnnouncementWriteService {
    pool: PgPool,
    events: std::sync::RwLock<Arc<dyn AnnouncementEventSink>>,
}

impl AnnouncementWriteService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            events: std::sync::RwLock::new(Arc::new(LoggingSink)),
        }
    }

    /// Wire the event sink (default logs). A durable composition stages the
    /// publish event into its outbox here.
    pub fn set_event_sink(&self, sink: Arc<dyn AnnouncementEventSink>) {
        *self.events.write().expect("announcements event sink lock poisoned") = sink;
    }

    fn events(&self) -> Arc<dyn AnnouncementEventSink> {
        self.events.read().expect("announcements event sink lock poisoned").clone()
    }

    async fn bind_ambient(&self, tx: &mut sqlx::PgConnection) -> Result<(), sqlx::Error> {
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            backbone_orm::org_scope::bind_org_scope_on(tx, &scope).await?;
        }
        Ok(())
    }

    /// Create a draft. The window must be well-formed (until after from).
    pub async fn create_draft(&self, n: NewAnnouncement) -> Result<Uuid, AnnouncementError> {
        if n.title.trim().is_empty() {
            return Err(AnnouncementError::Invalid("title must not be empty".into()));
        }
        if n.body.trim().is_empty() {
            return Err(AnnouncementError::Invalid("body must not be empty".into()));
        }
        if !matches!(n.audience_type.as_str(), "company" | "branch" | "department") {
            return Err(AnnouncementError::Invalid(
                "audience_type must be company, branch or department".into(),
            ));
        }
        if let Some(until) = n.publish_until {
            if until <= n.publish_from {
                return Err(AnnouncementError::Invalid(
                    "publish_until must be after publish_from".into(),
                ));
            }
        }
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        self.bind_ambient(&mut tx).await?;
        sqlx::query(
            r#"INSERT INTO announcements.announcements
                   (id, title, body, audience_type, audience_unit_id,
                    publish_from, publish_until, status, pinned, targeted_count, created_by)
               VALUES ($1, $2, $3, $4::announcement_audience_type, $5, $6, $7,
                       'draft', $8, 0, $9)"#,
        )
        .bind(id)
        .bind(&n.title)
        .bind(&n.body)
        .bind(&n.audience_type)
        .bind(n.audience_unit_id)
        .bind(n.publish_from)
        .bind(n.publish_until)
        .bind(n.pinned)
        .bind(n.created_by)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Edit a draft (title, body, window, pin, audience). Only drafts are
    /// editable — everything after is history.
    pub async fn update_draft(
        &self,
        id: Uuid,
        title: Option<String>,
        body: Option<String>,
        publish_from: Option<DateTime<Utc>>,
        publish_until: Option<DateTime<Utc>>,
        pinned: Option<bool>,
    ) -> Result<(), AnnouncementError> {
        let mut tx = self.pool.begin().await?;
        self.bind_ambient(&mut tx).await?;
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status::text FROM announcements.announcements WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        match status.as_deref() {
            None => return Err(AnnouncementError::NotFound("announcement")),
            Some("draft") => {}
            Some(other) => {
                return Err(AnnouncementError::InvalidState(match other {
                    "scheduled" => "a scheduled announcement is frozen — cancel and redraft",
                    _ => "only a draft may be edited",
                }))
            }
        }
        sqlx::query(
            r#"UPDATE announcements.announcements
                  SET title = COALESCE($2, title),
                      body = COALESCE($3, body),
                      publish_from = COALESCE($4, publish_from),
                      publish_until = $5,
                      pinned = COALESCE($6, pinned)
                WHERE id = $1"#,
        )
        .bind(id)
        .bind(title)
        .bind(body)
        .bind(publish_from)
        .bind(publish_until)
        .bind(pinned)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Schedule a draft for its `publish_from` moment (the tick publishes it).
    pub async fn schedule(&self, id: Uuid) -> Result<(), AnnouncementError> {
        let moved = self
            .status_move(id, "draft", "scheduled", "UPDATE announcements.announcements SET status = 'scheduled' WHERE id = $1 AND status = 'draft'")
            .await?;
        if !moved {
            return Err(AnnouncementError::InvalidState("only a draft may be scheduled"));
        }
        Ok(())
    }

    /// Archive a published announcement (leaves the employee lists, keeps the
    /// read history).
    pub async fn archive(&self, id: Uuid) -> Result<(), AnnouncementError> {
        let moved = self
            .status_move(id, "published", "archived", "UPDATE announcements.announcements SET status = 'archived' WHERE id = $1 AND status = 'published'")
            .await?;
        if !moved {
            return Err(AnnouncementError::InvalidState("only a published announcement may be archived"));
        }
        Ok(())
    }

    async fn status_move(
        &self,
        id: Uuid,
        _from: &str,
        _to: &str,
        stmt: &str,
    ) -> Result<bool, AnnouncementError> {
        let mut tx = self.pool.begin().await?;
        self.bind_ambient(&mut tx).await?;
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT status::text FROM announcements.announcements WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            return Err(AnnouncementError::NotFound("announcement"));
        }
        let moved = sqlx::query(stmt).bind(id).execute(&mut *tx).await?.rows_affected();
        tx.commit().await?;
        Ok(moved == 1)
    }

    /// The audience size under one unit: distinct linked users on employees
    /// under the unit's subtree. Runs on a dedicated connection whose scope
    /// var holds exactly that subtree, so the composer's fence admits the
    /// read whatever the caller's ambient scope is.
    async fn count_audience(&self, audience_unit: Uuid) -> Result<i64, AnnouncementError> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query("SELECT set_config('app.scope_unit_ids', $1, false)")
            .bind(
                // The subtree itself, as a comma list (the producer pattern).
                sqlx::query_scalar::<_, String>(
                    r#"WITH RECURSIVE subtree AS (
                           SELECT id FROM organization.org_units WHERE id = $1
                           UNION ALL
                           SELECT u.id FROM organization.org_units u JOIN subtree s ON u.parent_id = s.id
                       )
                       SELECT COALESCE(string_agg(id::text, ','), '') FROM subtree"#,
                )
                .bind(audience_unit)
                .fetch_one(&mut *conn)
                .await?,
            )
            .execute(&mut *conn)
            .await?;
        let count = sqlx::query_scalar::<_, i64>(
            r#"WITH RECURSIVE subtree AS (
                   SELECT id FROM organization.org_units WHERE id = $1
                   UNION ALL
                   SELECT u.id FROM organization.org_units u JOIN subtree s ON u.parent_id = s.id
               )
               SELECT COUNT(DISTINCT e.user_id)
                 FROM employee.employees e
                WHERE e.org_unit_id IN (SELECT id FROM subtree)
                  AND e.user_id IS NOT NULL
                  AND (e.metadata->>'deleted_at') IS NULL"#,
        )
        .bind(audience_unit)
        .fetch_one(&mut *conn)
        .await?;
        Ok(count)
    }

    /// Publish now (a draft or a scheduled row whose moment the operator
    /// pulls forward). Idempotent: an already-published row answers `already`.
    pub async fn publish_now(&self, id: Uuid) -> Result<PublishOutcome, AnnouncementError> {
        let mut tx = self.pool.begin().await?;
        self.bind_ambient(&mut tx).await?;
        let row = sqlx::query(
            r#"SELECT title, audience_unit_id, status::text AS status
                 FROM announcements.announcements WHERE id = $1 FOR UPDATE"#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        use sqlx::Row;
        let row = match row {
            Some(r) => r,
            None => return Err(AnnouncementError::NotFound("announcement")),
        };
        let title: String = row.try_get("title")?;
        let audience_unit_id: Uuid = row.try_get("audience_unit_id")?;
        let status: String = row.try_get("status")?;
        if status == "published" {
            let count: i64 = sqlx::query_scalar(
                "SELECT targeted_count::bigint FROM announcements.announcements WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(PublishOutcome { announcement_id: id, targeted_count: count, already: true });
        }
        if status != "draft" && status != "scheduled" {
            return Err(AnnouncementError::InvalidState(
                "only a draft or scheduled announcement may be published",
            ));
        }
        tx.commit().await?;

        // The snapshot: count under the audience's own subtree scope.
        let targeted = self.count_audience(audience_unit_id).await?;
        let published_at = Utc::now();

        let mut tx = self.pool.begin().await?;
        self.bind_ambient(&mut tx).await?;
        let moved = sqlx::query(
            r#"UPDATE announcements.announcements
                  SET status = 'published', targeted_count = $2, publish_from = $3
                WHERE id = $1 AND status IN ('draft', 'scheduled')"#,
        )
        .bind(id)
        .bind(targeted as i32)
        .bind(published_at)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        if moved != 1 {
            return Err(AnnouncementError::InvalidState(
                "the announcement changed state under the publish — retry",
            ));
        }

        self.events().publish(AnnouncementEvent::Published {
            announcement_id: id,
            audience_unit_id,
            title,
            targeted_count: targeted,
            published_at,
        });
        Ok(PublishOutcome { announcement_id: id, targeted_count: targeted, already: false })
    }

    /// The scheduler tick: publish every scheduled row whose moment arrived.
    /// SKIP-locked so concurrent ticks divide the rows, never double-publish.
    pub async fn publish_due(&self, now: DateTime<Utc>) -> Result<Vec<PublishOutcome>, AnnouncementError> {
        // The scan rides a scope-bound transaction: the fence would read a
        // bare-pool SELECT as empty (fail closed), and the tick's caller
        // wraps this whole call in an ambient org request scope.
        let mut tx = self.pool.begin().await?;
        self.bind_ambient(&mut tx).await?;
        let ids: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM announcements.announcements
                WHERE status = 'scheduled' AND publish_from <= $1
                ORDER BY publish_from
                LIMIT 100
                FOR UPDATE SKIP LOCKED"#,
        )
        .bind(now)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            out.push(self.publish_now(id).await?);
        }
        Ok(out)
    }

    /// Mark one user's read — idempotent per (announcement, user); refuses an
    /// announcement that is not currently visible.
    pub async fn mark_read(&self, announcement_id: Uuid, user_id: Uuid) -> Result<bool, AnnouncementError> {
        let mut tx = self.pool.begin().await?;
        self.bind_ambient(&mut tx).await?;
        let row = sqlx::query(
            r#"SELECT status::text, publish_until FROM announcements.announcements
                WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(announcement_id)
        .fetch_optional(&mut *tx)
        .await?;
        use sqlx::Row;
        let row = match row {
            None => return Err(AnnouncementError::NotFound("announcement")),
            Some(r) => r,
        };
        let status: String = row.try_get("status")?;
        let until: Option<DateTime<Utc>> = row.try_get("publish_until")?;
        if status != "published" {
            return Err(AnnouncementError::InvalidState("only a published announcement can be read"));
        }
        if let Some(until) = until {
            if until <= Utc::now() {
                return Err(AnnouncementError::InvalidState("the publish window has closed"));
            }
        }
        sqlx::query(
            r#"INSERT INTO announcements.announcement_reads (id, announcement_id, user_id, read_at)
               VALUES ($1, $2, $3, $4)
                    ON CONFLICT (announcement_id, user_id) DO NOTHING"#,
        )
        .bind(Uuid::new_v4())
        .bind(announcement_id)
        .bind(user_id)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }
}
