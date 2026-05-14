use std::path::Path;

#[test]
fn eve_smoke_cargo_toml_exists() {
    assert!(Path::new("Cargo.toml").exists());
}
