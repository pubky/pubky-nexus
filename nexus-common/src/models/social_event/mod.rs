//! Native social calendar, attendance and rating projections. No resource-feed writes.
mod availability;
pub mod calendar;
mod collections;
mod store;
mod views;
pub use availability::{availability_stream, AvailabilityView};
pub use collections::{calendar_stream, calendar_view, CalendarQuery, CalendarView};

pub use store::{
    flush_notifications, maintain_occurrences, prune_orphan_rsvps, remove, sync, sync_with_parent,
    validate_parent,
};
pub use views::{
    attendees, event_view, event_view_with_options, rating_summary, stream, Attendee, EventCounts,
    EventQuery, EventView, EventViewOptions, RatingSummary,
};

use crate::models::{
    error::{ModelError, ModelResult},
    post::PostKind,
};
use chrono::{DateTime, Utc};
use pubky_social_specs::{
    PubkySocialAvailabilityContent, PubkySocialEventContent, PubkySocialRatingContent,
    PubkySocialRsvpContent,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug)]
pub enum SocialContent {
    Event(Box<PubkySocialEventContent>),
    Rsvp(PubkySocialRsvpContent),
    Rating(PubkySocialRatingContent),
    Availability(PubkySocialAvailabilityContent),
    Other,
}
impl SocialContent {
    pub fn read(kind: PostKind, content: &str, now: DateTime<Utc>) -> ModelResult<Self> {
        let bad = |e: serde_json::Error| ModelError::InvalidInput(e.to_string());
        Ok(match kind {
            PostKind::Event => {
                let event: PubkySocialEventContent = serde_json::from_str(content).map_err(bad)?;
                calendar::validate(&event, now).map_err(ModelError::InvalidInput)?;
                Self::Event(Box::new(event))
            }
            PostKind::Rsvp => Self::Rsvp(serde_json::from_str(content).map_err(bad)?),
            PostKind::Rating => Self::Rating(serde_json::from_str(content).map_err(bad)?),
            PostKind::Availability => {
                Self::Availability(serde_json::from_str(content).map_err(bad)?)
            }
            _ => Self::Other,
        })
    }
}

/// An indexed instance, keyed by master URI and the original local start. Times are UTC
/// milliseconds, following the Nexus API's timestamp convention. `post_uri` can be an override.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct Occurrence {
    pub master: String,
    pub recurrence_id: String,
    pub post_uri: String,
    pub author: String,
    pub post_id: String,
    pub start_at: i64,
    pub end_at: i64,
    pub status: String,
    pub all_day: bool,
    pub locations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct StoredEvent {
    uri: String,
    author: String,
    id: String,
    content: String,
}
