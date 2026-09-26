use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::AnnouncementAudienceType;
use super::AnnouncementStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Announcement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AnnouncementId(pub Uuid);

impl AnnouncementId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for AnnouncementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AnnouncementId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for AnnouncementId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<AnnouncementId> for Uuid {
    fn from(id: AnnouncementId) -> Self { id.0 }
}

impl AsRef<Uuid> for AnnouncementId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for AnnouncementId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Announcement {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub audience_type: AnnouncementAudienceType,
    pub audience_unit_id: Uuid,
    pub publish_from: DateTime<Utc>,
    pub publish_until: Option<DateTime<Utc>>,
    pub status: AnnouncementStatus,
    pub pinned: bool,
    pub targeted_count: i32,
    pub created_by: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Announcement {
    /// Create a builder for Announcement
    pub fn builder() -> AnnouncementBuilder {
        <AnnouncementBuilder as Default>::default()
    }

    /// Create a new Announcement with required fields
    pub fn new(title: String, body: String, audience_type: AnnouncementAudienceType, audience_unit_id: Uuid, publish_from: DateTime<Utc>, status: AnnouncementStatus, pinned: bool, targeted_count: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            body,
            audience_type,
            audience_unit_id,
            publish_from,
            publish_until: None,
            status,
            pinned,
            targeted_count,
            created_by: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> AnnouncementId {
        AnnouncementId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }

    /// Get the current status
    pub fn status(&self) -> &AnnouncementStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the publish_until field (chainable)
    pub fn with_publish_until(mut self, value: DateTime<Utc>) -> Self {
        self.publish_until = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "title" => {
                    if let Ok(v) = serde_json::from_value(value) { self.title = v; }
                }
                "body" => {
                    if let Ok(v) = serde_json::from_value(value) { self.body = v; }
                }
                "audience_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.audience_type = v; }
                }
                "audience_unit_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.audience_unit_id = v; }
                }
                "publish_from" => {
                    if let Ok(v) = serde_json::from_value(value) { self.publish_from = v; }
                }
                "publish_until" => {
                    if let Ok(v) = serde_json::from_value(value) { self.publish_until = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "pinned" => {
                    if let Ok(v) = serde_json::from_value(value) { self.pinned = v; }
                }
                "targeted_count" => {
                    if let Ok(v) = serde_json::from_value(value) { self.targeted_count = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Announcement {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Announcement"
    }
}

impl backbone_core::PersistentEntity for Announcement {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for Announcement {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("audience_unit_id".to_string(), "uuid".to_string());
        m.insert("audience_type".to_string(), "announcement_audience_type".to_string());
        m.insert("status".to_string(), "announcement_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["title", "body"]
    }
}

/// Builder for Announcement entity
///
/// Provides a fluent API for constructing Announcement instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct AnnouncementBuilder {
    title: Option<String>,
    body: Option<String>,
    audience_type: Option<AnnouncementAudienceType>,
    audience_unit_id: Option<Uuid>,
    publish_from: Option<DateTime<Utc>>,
    publish_until: Option<DateTime<Utc>>,
    status: Option<AnnouncementStatus>,
    pinned: Option<bool>,
    targeted_count: Option<i32>,
}

impl AnnouncementBuilder {
    /// Set the title field (required)
    pub fn title(mut self, value: String) -> Self {
        self.title = Some(value);
        self
    }

    /// Set the body field (required)
    pub fn body(mut self, value: String) -> Self {
        self.body = Some(value);
        self
    }

    /// Set the audience_type field (required)
    pub fn audience_type(mut self, value: AnnouncementAudienceType) -> Self {
        self.audience_type = Some(value);
        self
    }

    /// Set the audience_unit_id field (required)
    pub fn audience_unit_id(mut self, value: Uuid) -> Self {
        self.audience_unit_id = Some(value);
        self
    }

    /// Set the publish_from field (required)
    pub fn publish_from(mut self, value: DateTime<Utc>) -> Self {
        self.publish_from = Some(value);
        self
    }

    /// Set the publish_until field (optional)
    pub fn publish_until(mut self, value: DateTime<Utc>) -> Self {
        self.publish_until = Some(value);
        self
    }

    /// Set the status field (default: `AnnouncementStatus::default()`)
    pub fn status(mut self, value: AnnouncementStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the pinned field (default: `false`)
    pub fn pinned(mut self, value: bool) -> Self {
        self.pinned = Some(value);
        self
    }

    /// Set the targeted_count field (default: `0`)
    pub fn targeted_count(mut self, value: i32) -> Self {
        self.targeted_count = Some(value);
        self
    }

    /// Build the Announcement entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Announcement, String> {
        let title = self.title.ok_or_else(|| "title is required".to_string())?;
        let body = self.body.ok_or_else(|| "body is required".to_string())?;
        let audience_type = self.audience_type.ok_or_else(|| "audience_type is required".to_string())?;
        let audience_unit_id = self.audience_unit_id.ok_or_else(|| "audience_unit_id is required".to_string())?;
        let publish_from = self.publish_from.ok_or_else(|| "publish_from is required".to_string())?;

        Ok(Announcement {
            id: Uuid::new_v4(),
            title,
            body,
            audience_type,
            audience_unit_id,
            publish_from,
            publish_until: self.publish_until,
            status: self.status.unwrap_or_default(),
            pinned: self.pinned.unwrap_or(false),
            targeted_count: self.targeted_count.unwrap_or(0),
            created_by: None,
            metadata: AuditMetadata::default(),
        })
    }
}
