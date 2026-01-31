//! Unique timestamp offsets for deterministic test execution.
//!
//! Each test gets a unique offset (in microseconds) to add to the current timestamp.
//! This ensures unique post IDs across parallel test runs while keeping IDs valid.
//!
//! Offsets are spaced 1 second (1_000_000 microseconds) apart to ensure uniqueness.

// =============================================================================
// posts module offsets (starting at 1_000_000)
// =============================================================================

pub mod posts {
    pub mod raw {
        pub const POST: i64 = 1_000_000;
    }

    pub mod reply {
        pub const PARENT_POST: i64 = 2_000_000;
        pub const REPLY: i64 = 3_000_000;
    }

    pub mod repost {
        pub const PARENT_POST: i64 = 4_000_000;
        pub const REPOST: i64 = 5_000_000;
    }

    pub mod reply_repost {
        pub const PARENT_POST: i64 = 6_000_000;
        pub const REPLY: i64 = 7_000_000;
        pub const REPOST: i64 = 8_000_000;
    }

    pub mod reply_engagement {
        pub const PARENT_POST: i64 = 10_000_000;
        pub const REPLY: i64 = 11_000_000;
        pub const REPLY_OF_REPLY: i64 = 12_000_000;
        pub const REPLY_REPOST: i64 = 13_000_000;
    }

    pub mod engagement {
        pub const ALICE_POST: i64 = 20_000_000;
        pub const BOB_REPLY: i64 = 21_000_000;
        pub const BOB_REPOST: i64 = 22_000_000;
    }

    pub mod influencer {
        pub const ALICE_POST: i64 = 30_000_000;
        pub const BOB_REPLY: i64 = 31_000_000;
        pub const BOB_REPOST: i64 = 32_000_000;
    }

    pub mod attachments {
        pub const POST: i64 = 40_000_000;
    }

    pub mod moderated {
        pub const POST: i64 = 50_000_000;
    }

    pub mod del_without_relations {
        pub const TEST_DELETE_POST: i64 = 60_000_000;
        pub const TEST_DELETE_REPOST_POST: i64 = 61_000_000;
        pub const TEST_DELETE_REPOST_REPOST: i64 = 62_000_000;
        pub const TEST_DELETE_REPLY_POST: i64 = 63_000_000;
        pub const TEST_DELETE_REPLY_REPLY: i64 = 64_000_000;
    }

    pub mod del_with_relations {
        pub const POST: i64 = 70_000_000;
    }

    pub mod del_with_attachments {
        pub const POST: i64 = 80_000_000;
    }

    pub mod reply_notification {
        pub const ALICE_POST: i64 = 90_000_000;
        pub const ALICE_REPLY: i64 = 91_000_000;
        pub const BOB_REPLY: i64 = 92_000_000;
    }

    pub mod repost_notification {
        pub const ALICE_POST: i64 = 100_000_000;
        pub const ALICE_REPOST: i64 = 101_000_000;
        pub const BOB_REPOST: i64 = 102_000_000;
    }

    pub mod del_reply_notification {
        pub const POST: i64 = 110_000_000;
        pub const REPLY: i64 = 111_000_000;
    }

    pub mod del_repost_notification {
        pub const POST: i64 = 120_000_000;
        pub const REPOST: i64 = 121_000_000;
    }

    pub mod del_reply_parent_notification {
        pub const POST: i64 = 130_000_000;
        pub const REPLY: i64 = 131_000_000;
    }

    pub mod del_reposted_notification {
        pub const POST: i64 = 140_000_000;
        pub const REPOST: i64 = 141_000_000;
    }

    pub mod del_tagged_notification {
        pub const POST: i64 = 150_000_000;
    }

    pub mod del_bookmarked_notification {
        pub const POST: i64 = 160_000_000;
    }

    pub mod edit_reply_parent_notification {
        pub const POST: i64 = 170_000_000;
        pub const REPLY: i64 = 171_000_000;
    }

    pub mod edit_reposted_notification {
        pub const POST: i64 = 180_000_000;
        pub const REPOST: i64 = 181_000_000;
    }

    pub mod edit_tagged_notification {
        pub const POST: i64 = 190_000_000;
    }

    pub mod edit_bookmarked_notification {
        pub const POST: i64 = 200_000_000;
    }

    pub mod fail_reply {
        pub const POST: i64 = 210_000_000;
        pub const REPLY: i64 = 211_000_000;
    }

    pub mod fail_repost {
        pub const POST: i64 = 220_000_000;
        pub const REPOST: i64 = 221_000_000;
    }

    pub mod fail_user {
        pub const POST: i64 = 230_000_000;
    }

    pub mod retry_post {
        pub const POST: i64 = 240_000_000;
    }

    pub mod retry_reply {
        pub const REPLY: i64 = 250_000_000;
    }

    pub mod retry_repost {
        pub const REPOST: i64 = 260_000_000;
    }

    pub mod retry_all {
        pub const REPOST_REPLY: i64 = 270_000_000;
    }
}

// =============================================================================
// tags module offsets (starting at 1_000_000_000)
// =============================================================================

pub mod tags {
    pub mod post_put {
        pub const TEST_POST_TAG_CREATE: i64 = 1_000_000_000;
        pub const TEST_SAME_POST_TAG_TWICE: i64 = 1_001_000_000;
    }

    pub mod post_del {
        pub const POST: i64 = 1_010_000_000;
    }

    pub mod post_notification {
        pub const POST: i64 = 1_020_000_000;
    }

    pub mod post_multi_user {
        pub const POST: i64 = 1_030_000_000;
    }

    pub mod fail_index {
        pub const POST: i64 = 1_040_000_000;
    }
}

// =============================================================================
// bookmarks module offsets (starting at 2_000_000_000)
// =============================================================================

pub mod bookmarks {
    pub mod raw {
        pub const POST: i64 = 2_000_000_000;
    }

    pub mod del {
        pub const POST: i64 = 2_010_000_000;
    }

    pub mod viewer {
        pub const POST: i64 = 2_020_000_000;
    }

    pub mod fail_index {
        pub const POST: i64 = 2_030_000_000;
    }
}

// =============================================================================
// mentions module offsets (starting at 3_000_000_000)
// =============================================================================

pub mod mentions {
    pub mod raw {
        pub const POST: i64 = 3_000_000_000;
    }

    pub mod notification {
        pub const POST: i64 = 3_010_000_000;
    }
}

// =============================================================================
// homeserver module offsets (starting at 4_000_000_000)
// =============================================================================

pub mod homeserver {
    pub mod ingest_from_post_events {
        pub const TEST_REPLY: i64 = 4_000_000_000;
        pub const TEST_REPOST: i64 = 4_001_000_000;
        pub const TEST_POST: i64 = 4_002_000_000;
    }
}

// =============================================================================
// users module offsets (starting at 5_000_000_000)
// =============================================================================

pub mod users {
    pub mod del_with_relations {
        pub const TEST_DELETE_USER_A_POST: i64 = 5_000_000_000;
        pub const TEST_DELETE_USER_B_POST: i64 = 5_001_000_000;
        /// Base offset for Carol's posts in the loop (add index * 1_000_000)
        pub const TEST_DELETE_USER_C_POST_BASE: i64 = 5_010_000_000;
    }
}

// =============================================================================
// network module offsets (starting at 6_000_000_000)
// =============================================================================

pub mod network {
    pub mod counts {
        /// Base offset for posts in the loop (add post_counter * 1_000_000)
        pub const POST_BASE: i64 = 6_000_000_000;
    }
}
