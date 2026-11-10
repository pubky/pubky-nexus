use pubky_social_specs::PubkyId;

/// Which resource an event names: the user whose storage holds it and its id there, however the
/// epoch spells the address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceKey {
    pub owner_id: PubkyId,
    pub resource_id: String,
}
