use crate::sandbox::manager;

pub fn rollback_sandbox(path: &str) -> Result<(), String> {
    manager::destroy_sandbox(path)
}
