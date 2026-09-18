//! Concrete jobs the daemon can register. The engine does not depend on these types.

mod hot_tags;
mod influencers;
mod trust_recompute;

pub use hot_tags::HotTagsCacheJob;
pub use influencers::InfluencersCacheJob;
pub use trust_recompute::TrustRecomputeJob;
