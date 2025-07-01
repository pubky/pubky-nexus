# Pre-submission Checklist

> For tests to work you need a working neo4j and redis instance with the example dataset in `docker/db-graph`

- [ ] **Testing**: Implement and pass new tests for the new features/fixes, `cargo nextest run`.
- [ ] **Performance**: Ensure new code has relevant performance benchmarks, `cargo bench -p nexus-webapi`
