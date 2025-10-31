use nexus_watcher::events::moderation::Moderation;
use pubky_app_specs::PubkyId;

pub mod watcher;

/// Default Moderation settings for tests
pub fn default_moderation_tests() -> Moderation {
    let id = PubkyId::try_from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko")
        .expect("Hardcoded test moderation key should be valid");
    let tags = Vec::from(["label_to_moderate".to_string()]);
    Moderation { id, tags }
}
