# Run tests serially since we use db calls
RUST_TEST_THREADS=1 cargo test