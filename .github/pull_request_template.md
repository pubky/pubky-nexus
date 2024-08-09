## Pre-submission Checklist

> For tests to work you need a working neo4j instance with the example dataset in `docker/db-migration`

- [ ] **Code Quality**: Clippy has been run with no warnings, `cargo clippy`
- [ ] **Testing**: Implement and pass new tests for all new code, while maintaining existing test suite, `cargo test`.
- [ ] **Performance**: Ensure new code has relevant performance benchmarks, `cargo bench`
