//! Behaviour tests for the write service's database routing and the
//! scheduled publish pass.
//!
//! Each test provisions its own scratch databases on the server named by
//! `DATABASE_URL` (dropped and recreated per run, named after the test), and
//! applies the module's own migrations plus the two foreign tables the
//! audience count reads. The tests SKIP when no server is reachable; set
//! `ANNOUNCEMENTS_REQUIRE_DB=1` to turn a skip into a failure.

use backbone_announcements::application::service::AnnouncementWriteService;
use backbone_announcements::request_pool;
use chrono::{DateTime, Duration, TimeZone, Utc};
use sqlx::PgPool;
use uuid::Uuid;

fn db_required() -> bool {
    std::env::var("ANNOUNCEMENTS_REQUIRE_DB").map(|v| v != "0").unwrap_or(false)
}

/// A fresh, migrated scratch database named `announcements_<test>_<suffix>`.
async fn scratch(suffix: &str) -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let (prefix, _) = url.trim_end_matches('/').rsplit_once('/')?;
    let test = std::thread::current()
        .name()
        .unwrap_or("main")
        .rsplit("::")
        .next()
        .unwrap_or("main")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .take(40)
        .collect::<String>();
    let name = format!("announcements_{test}_{suffix}");
    let admin = match PgPool::connect(&format!("{prefix}/postgres")).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skip: no admin connection ({e})");
            assert!(!db_required(), "a database is required");
            return None;
        }
    };
    let _ = sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{name}" WITH (FORCE)"#))
        .execute(&admin)
        .await;
    sqlx::query(&format!(r#"CREATE DATABASE "{name}""#))
        .execute(&admin)
        .await
        .expect("create scratch database");
    admin.close().await;
    let pool = PgPool::connect(&format!("{prefix}/{name}")).await.expect("connect scratch");
    for ddl in [
        include_str!("../migrations/20260426220000_create_enums.up.sql"),
        include_str!("../migrations/20260426220001_create_announcement_table.up.sql"),
        include_str!("../migrations/20260426220002_create_announcement_read_table.up.sql"),
        // The audience count reads the org tree and the employee links.
        r#"CREATE SCHEMA IF NOT EXISTS organization;
           CREATE TABLE organization.org_units (id UUID PRIMARY KEY, parent_id UUID);
           CREATE SCHEMA IF NOT EXISTS employee;
           CREATE TABLE employee.employees (
               id UUID PRIMARY KEY, org_unit_id UUID, user_id UUID,
               metadata JSONB NOT NULL DEFAULT '{}'::jsonb);"#,
    ] {
        sqlx::raw_sql(ddl).execute(&pool).await.expect("apply ddl");
    }
    Some(pool)
}

async fn seed_unit_with_one_user(pool: &PgPool) -> Uuid {
    let unit = Uuid::new_v4();
    sqlx::query("INSERT INTO organization.org_units (id) VALUES ($1)")
        .bind(unit)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO employee.employees (id, org_unit_id, user_id) VALUES ($1, $2, $3)")
        .bind(Uuid::new_v4())
        .bind(unit)
        .bind(Uuid::new_v4())
        .execute(pool)
        .await
        .unwrap();
    unit
}

async fn seed_announcement(
    pool: &PgPool,
    unit: Uuid,
    status: &str,
    publish_from: DateTime<Utc>,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO announcements.announcements
               (id, title, body, audience_type, audience_unit_id, publish_from, status)
           VALUES ($1, 'Notice', 'Body', 'company', $2, $3, $4::announcement_status)"#,
    )
    .bind(id)
    .bind(unit)
    .bind(publish_from)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn status_of(pool: &PgPool, id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT status::text FROM announcements.announcements WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

/// A service composed on one database acts on the database bound as the
/// request pool, as it does under a tenant mount, and leaves the composed
/// database alone.
#[tokio::test]
async fn verbs_run_on_the_bound_request_pool() {
    let Some(boot) = scratch("boot").await else { return };
    let Some(tenant) = scratch("tenant").await else { return };
    let svc = AnnouncementWriteService::new(boot.clone());

    let unit = seed_unit_with_one_user(&tenant).await;
    let draft = seed_announcement(&tenant, unit, "draft", Utc::now()).await;

    // Unbound, the service only sees the composed database.
    assert!(svc.schedule(draft).await.is_err(), "the composed database has no such row");

    request_pool::with_pool_scope(tenant.clone(), svc.schedule(draft))
        .await
        .expect("schedule on the tenant database");
    assert_eq!(status_of(&tenant, draft).await.as_deref(), Some("scheduled"));

    let outcome = request_pool::with_pool_scope(tenant.clone(), svc.publish_now(draft))
        .await
        .expect("publish on the tenant database");
    assert_eq!(outcome.targeted_count, 1, "the audience is counted in the tenant database");
    assert_eq!(status_of(&tenant, draft).await.as_deref(), Some("published"));

    let created = request_pool::with_pool_scope(
        tenant.clone(),
        svc.create_draft(backbone_announcements::application::service::NewAnnouncement {
            title: "Second".into(),
            body: "Body".into(),
            audience_type: "company".into(),
            audience_unit_id: unit,
            publish_from: Utc::now(),
            publish_until: None,
            pinned: false,
            created_by: None,
        }),
    )
    .await
    .expect("create on the tenant database");
    assert_eq!(status_of(&tenant, created).await.as_deref(), Some("draft"));

    let boot_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM announcements.announcements")
        .fetch_one(&boot)
        .await
        .unwrap();
    assert_eq!(boot_rows, 0, "nothing reached the composed database");
}

/// The publish pass given a start point publishes what came due from that
/// point on and leaves older scheduled notices where they are.
#[tokio::test]
async fn the_publish_pass_leaves_notices_due_before_its_start() {
    let Some(pool) = scratch("db").await else { return };
    let svc = AnnouncementWriteService::new(pool.clone());
    let unit = seed_unit_with_one_user(&pool).await;

    let start = Utc.with_ymd_and_hms(2026, 10, 4, 0, 0, 0).unwrap();
    let now = start + Duration::hours(9);
    let backlog = seed_announcement(&pool, unit, "scheduled", start - Duration::days(5)).await;
    let due = seed_announcement(&pool, unit, "scheduled", start + Duration::hours(8)).await;
    let future = seed_announcement(&pool, unit, "scheduled", now + Duration::days(1)).await;

    let published = svc.publish_due_from(now, Some(start)).await.expect("publish pass");
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].announcement_id, due);
    assert_eq!(status_of(&pool, due).await.as_deref(), Some("published"));
    assert_eq!(status_of(&pool, backlog).await.as_deref(), Some("scheduled"), "the backlog stays put");
    assert_eq!(status_of(&pool, future).await.as_deref(), Some("scheduled"));

    // A second pass finds nothing new.
    assert!(svc.publish_due_from(now, Some(start)).await.unwrap().is_empty());

    // Without a start point every due row publishes, as before.
    let all = svc.publish_due(now).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].announcement_id, backlog);
}
