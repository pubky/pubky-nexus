use std::collections::HashMap;

use pubky::{EventCursor, PublicKey};
use tokio::sync::Mutex;

/// Where each homeserver's last run stopped at its budget, so the next run starts from there
/// and every user is reached in turn, however long a full pass over the homeserver takes.
///
/// Kept in memory only: after a restart, each homeserver starts again from its first user.
#[derive(Default)]
pub struct UserResumePoints {
    /// The user each homeserver's next run starts from, by HS ID.
    inner: Mutex<HashMap<String, String>>,
}

impl UserResumePoints {
    /// Orders `users` by key, starting from the homeserver's resume point and wrapping around.
    pub async fn order(&self, hs_id: &str, users: &mut [(PublicKey, EventCursor)]) {
        users.sort_by_cached_key(|(user_pk, _)| user_pk.z32());
        let Some(resume_at) = self.inner.lock().await.get(hs_id).cloned() else {
            return;
        };
        let start = users.partition_point(|(user_pk, _)| user_pk.z32() < resume_at);
        users.rotate_left(start);
    }

    /// Records that the homeserver's run stopped after `users[index]`, with `users` in the turn
    /// order [`Self::order`] gave. The next run starts from the user after it, wrapping around.
    pub async fn record_stop_after(
        &self,
        hs_id: &str,
        users: &[(PublicKey, EventCursor)],
        index: usize,
    ) {
        let (next_user_pk, _) = &users[(index + 1) % users.len()];
        self.inner
            .lock()
            .await
            .insert(hs_id.to_string(), next_user_pk.z32());
    }

    /// Records that the homeserver's run reached all its users, so the next run starts from
    /// the first.
    pub async fn record_full_pass(&self, hs_id: &str) {
        self.inner.lock().await.remove(hs_id);
    }
}

#[cfg(test)]
mod tests {
    use nexus_common::utils::test_utils::random_pk;

    use super::*;

    fn sorted_users(count: usize) -> Vec<(PublicKey, EventCursor)> {
        let mut users: Vec<_> = (0..count)
            .map(|_| (random_pk(), EventCursor::new(0)))
            .collect();
        users.sort_by_cached_key(|(user_pk, _)| user_pk.z32());
        users
    }

    fn keys(users: &[(PublicKey, EventCursor)]) -> Vec<String> {
        users.iter().map(|(user_pk, _)| user_pk.z32()).collect()
    }

    #[tokio::test]
    async fn starts_from_the_first_user_without_a_resume_point() {
        let sorted = sorted_users(3);
        let mut users = sorted.clone();
        users.reverse();

        UserResumePoints::default().order("hs", &mut users).await;

        assert_eq!(keys(&users), keys(&sorted));
    }

    #[tokio::test]
    async fn starts_after_the_user_the_last_run_stopped_at_and_wraps_around() {
        let sorted = sorted_users(3);
        let points = UserResumePoints::default();
        points.record_stop_after("hs", &sorted, 0).await;

        let mut users = sorted.clone();
        points.order("hs", &mut users).await;
        let expected = [&sorted[1], &sorted[2], &sorted[0]].map(|(user_pk, _)| user_pk.z32());
        assert_eq!(keys(&users), expected);

        // Stopped on the last user of the turn: the next turn starts where this one did
        points.record_stop_after("hs", &users, 2).await;
        let mut next = sorted.clone();
        points.order("hs", &mut next).await;
        assert_eq!(keys(&next), expected);

        points.record_stop_after("hs", &users, 0).await;
        let mut next = sorted.clone();
        points.order("hs", &mut next).await;
        assert_eq!(keys(&next)[0], sorted[2].0.z32());
    }

    #[tokio::test]
    async fn a_full_pass_starts_the_next_run_from_the_first_user() {
        let sorted = sorted_users(3);
        let points = UserResumePoints::default();
        points.record_stop_after("hs", &sorted, 1).await;
        points.record_full_pass("hs").await;

        let mut users = sorted.clone();
        points.order("hs", &mut users).await;

        assert_eq!(keys(&users), keys(&sorted));
    }
}
