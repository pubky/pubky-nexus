//! Per-epoch translators between homeserver resources and handler calls, one module each.

pub(super) mod legacy_v0;

pub(super) use legacy_v0::{translate_del, translate_put, TranslatedDel, TranslatedPut};
