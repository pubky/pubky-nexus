use indexmap::IndexMap;

use anyhow::{Error, Result};
use nexus_watcher::service::NexusWatcher;

use crate::service::utils::MockEventProcessor;

pub const HS_IDS: [&str; 5] = [
    "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy",
    "8rsrmfrn1anbrzuxiffwy1174o58emf4qgbfk5h7s8a33r3bd8dy",
    "984orjzbusofbqhsqz9axpez3uuwd3hbpqztd6rtx3pr78y9s1my",
    "mamtihagiptrngan9y6cdj1xu7yb8yc7us9uerytaewc13ejqy9y",
    "8x93apuue6kjyqosu1wp9xye45j9noq8y3pmuwmhfo3o95eimgoo",
];

pub async fn setup() -> Result<IndexMap<String, MockEventProcessor>> {
    // Initialize the test stack
    if let Err(e) = NexusWatcher::builder().init_test_stack().await {
        return Err(Error::msg(format!("could not initialise the stack, {e:?}")));
    }

    Ok(IndexMap::new())
}
