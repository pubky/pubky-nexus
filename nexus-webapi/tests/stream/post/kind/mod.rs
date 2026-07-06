pub mod all;
pub mod collection;
pub mod exclude;
pub mod file;
pub mod image;
pub mod link;
pub mod long;
pub mod video;

pub const DETROIT: &str = "7w4hmktqa7gia5thmk7zki8px7ttwpwjtgaaaou4tbqx64re8d1o";
pub const EIXAMPLE: &str = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";

// Collection fixtures shared by collection.rs and exclude.rs.
pub const COL_BOGOTA_1: &str = "COLW1TGL5BKG1";
pub const COL_BOGOTA_2: &str = "COLW1TGL5BKG2";
pub const COL_CAIRO: &str = "COLW1TGL5BKG3";
// Debug-fixture Collections seeded for `?source=collection` tests.
// They participate in the global Collection set just like any other.
pub const COL_BOGOTA_MALF: &str = "MALF1TGL5BKG7";
pub const COL_BOGOTA_NEST: &str = "NEST1TGL5BKG8";
pub const SHORT_BOGOTA: &str = "00000039YD9BM";

pub const ALL_COLLECTIONS: &[&str] = &[
    COL_BOGOTA_1,
    COL_BOGOTA_2,
    COL_CAIRO,
    COL_BOGOTA_MALF,
    COL_BOGOTA_NEST,
];
