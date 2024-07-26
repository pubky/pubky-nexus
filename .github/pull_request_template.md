## Pre-submission Checklist

Before submitting this pull request, please ensure the following:

**Code Quality**
- [] Cargo Clippy has been run with no warnings:
```
cargo clippy
```

**Testing**
- [] Implement new tests for all new code and pass successfully all of them, including existing tests:
```bash
# For tests to work you need a working instance with example dataset like docker/db-migration)
cargo test
```

**Performance**
- [] Validate that appropriate benchmarks have been created for the new code to measure its performance.
```
cargo bench
```