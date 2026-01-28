use std::collections::HashSet;

/// A HashSet with a maximum capacity. Inserts beyond the limit are silently ignored.
#[derive(Debug)]
pub struct BoundedHashSet {
    inner: HashSet<String>,
    pub(crate) capacity: usize,
}

impl BoundedHashSet {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: HashSet::with_capacity(capacity),
            capacity,
        }
    }

    /// Inserts a value. Returns `true` if inserted, `false` if at capacity or duplicate.
    pub fn insert(&mut self, value: String) -> bool {
        if self.inner.len() >= self.capacity {
            return false;
        }
        self.inner.insert(value)
    }

    pub fn contains(&self, value: &str) -> bool {
        self.inner.contains(value)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.inner.len() >= self.capacity
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.inner.iter()
    }

    pub fn into_inner(self) -> HashSet<String> {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_within_capacity() {
        let mut set = BoundedHashSet::new(3);

        assert!(set.insert("a".into()));
        assert!(set.insert("b".into()));
        assert!(set.insert("c".into()));

        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_insert_beyond_capacity() {
        let mut set = BoundedHashSet::new(2);

        assert!(set.insert("a".into()));
        assert!(set.insert("b".into()));
        assert!(!set.insert("c".into()));

        assert_eq!(set.len(), 2);
        assert!(!set.contains("c"));
    }

    #[test]
    fn test_insert_duplicate() {
        let mut set = BoundedHashSet::new(3);

        assert!(set.insert("a".into()));
        assert!(!set.insert("a".into()));

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_is_full() {
        let mut set = BoundedHashSet::new(2);

        assert!(!set.is_full());
        set.insert("a".into());
        assert!(!set.is_full());
        set.insert("b".into());
        assert!(set.is_full());
    }

    #[test]
    fn test_is_empty() {
        let mut set = BoundedHashSet::new(5);

        assert!(set.is_empty());
        set.insert("a".into());
        assert!(!set.is_empty());
    }

    #[test]
    fn test_zero_capacity() {
        let mut set = BoundedHashSet::new(0);

        assert!(!set.insert("a".into()));
        assert!(set.is_empty());
        assert!(set.is_full());
    }

    #[test]
    fn test_contains() {
        let mut set = BoundedHashSet::new(5);

        set.insert("hello".into());

        assert!(set.contains("hello"));
        assert!(!set.contains("world"));
    }

    #[test]
    fn test_into_inner() {
        let mut set = BoundedHashSet::new(5);
        set.insert("a".into());
        set.insert("b".into());

        let inner = set.into_inner();
        assert_eq!(inner.len(), 2);
        assert!(inner.contains("a"));
        assert!(inner.contains("b"));
    }
}
