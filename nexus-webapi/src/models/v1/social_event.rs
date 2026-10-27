//! The social event views on `/v1`, the only version that serves them: each embeds the `/v1`
//! post view, so an event's post reads as the post route serves it.

use nexus_common::models::social_event::{self, Attendee, EventCounts, Occurrence};
use serde::Serialize;
use utoipa::ToSchema;

use super::PostView;

/// A social event with its attendance, as the stored [`social_event::EventView`] with the
/// `/v1` post view.
#[derive(Serialize, ToSchema, Debug)]
pub struct EventView {
    pub post: PostView,
    pub start_at: i64,
    pub end_at: i64,
    pub all_day: bool,
    pub status: String,
    pub policy: String,
    pub capacity: Option<i64>,
    pub counts: EventCounts,
    pub occurrences: Vec<Occurrence>,
    pub attendees: Vec<Attendee>,
}

impl From<social_event::EventView> for EventView {
    fn from(view: social_event::EventView) -> Self {
        let social_event::EventView {
            post,
            start_at,
            end_at,
            all_day,
            status,
            policy,
            capacity,
            counts,
            occurrences,
            attendees,
        } = view;
        Self {
            post: post.into(),
            start_at,
            end_at,
            all_day,
            status,
            policy,
            capacity,
            counts,
            occurrences,
            attendees,
        }
    }
}
