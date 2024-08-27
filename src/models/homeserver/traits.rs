use base32::{encode, Alphabet};
use blake3::Hasher;

/// Trait for generating an ID based on the struct's data.
pub trait GenerateId {
    fn get_id_data(&self) -> String;

    /// Creates a unique identifier for bookmarks and tag homeserver paths instance.
    ///
    /// The ID is generated by:
    /// 1. Concatenating the `uri` and `label` fields of the `HomeserverTag` with a colon (`:`) separator.
    /// 2. Hashing the concatenated string using the `blake3` hashing algorithm.
    /// 3. Taking the first half of the bytes from the resulting `blake3` hash.
    /// 4. Encoding those bytes using the Z-base32 alphabet (Base32 variant).
    ///
    /// The resulting Base32-encoded string is returned as the tag ID.
    ///
    /// # Returns
    /// - A `String` representing the Base32-encoded tag ID derived from the `blake3` hash of the concatenated `uri` and `label`.
    fn create_id(&self) -> String {
        let data = self.get_id_data();

        // Create a Blake3 hash of the input data
        let mut hasher = Hasher::new();
        hasher.update(data.as_bytes());
        let blake3_hash = hasher.finalize();

        // Get the first half of the hash bytes
        let half_hash_length = blake3_hash.as_bytes().len() / 2;
        let half_hash = &blake3_hash.as_bytes()[..half_hash_length];

        // Encode the first half of the hash in Base32 using the Z-base32 alphabet
        encode(Alphabet::Z, half_hash)
    }
}
