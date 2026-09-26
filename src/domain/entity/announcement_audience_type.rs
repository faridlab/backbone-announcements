use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "announcement_audience_type", rename_all = "snake_case")]
pub enum AnnouncementAudienceType {
    Company,
    Branch,
    Department,
}

impl std::fmt::Display for AnnouncementAudienceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Company => write!(f, "company"),
            Self::Branch => write!(f, "branch"),
            Self::Department => write!(f, "department"),
        }
    }
}

impl FromStr for AnnouncementAudienceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "company" => Ok(Self::Company),
            "branch" => Ok(Self::Branch),
            "department" => Ok(Self::Department),
            _ => Err(format!("Unknown AnnouncementAudienceType variant: {}", s)),
        }
    }
}

impl Default for AnnouncementAudienceType {
    fn default() -> Self {
        Self::Company
    }
}
