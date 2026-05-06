use std::path::{Component, Path};

use crate::contracts::mutation::{MutationContract, MutationKind};

pub const MAX_MUTATION_PAYLOAD_BYTES: usize = 2 * 1024;
pub const MAX_TARGET_FILE_BYTES: u64 = 256 * 1024;

pub fn validate_mutation(mutation: &MutationContract) -> Result<(), String> {
    if mutation.id.trim().is_empty() {
        return Err("mutation id must not be empty".to_string());
    }
    if mutation.risk > 0.5 {
        return Err("mutation risk too high".to_string());
    }
    if !(0.0..=1.0).contains(&mutation.expected_gain) {
        return Err("expected gain must be between 0.0 and 1.0".to_string());
    }
    validate_target_path(&mutation.target_file, mutation.kind)?;
    validate_payload_size(mutation)?;

    match mutation.kind {
        MutationKind::AppendComment => {
            let append = mutation
                .append
                .as_ref()
                .ok_or_else(|| "append mutation requires append payload".to_string())?;
            if append.len() > 300 {
                return Err("append payload too large".to_string());
            }
            if !append.trim_start().starts_with("//") && !append.trim_start().starts_with("/*") {
                return Err("append mutation only allows Rust comments".to_string());
            }
        }
        MutationKind::ReplaceText => {
            require_search_replace(mutation, "replace mutation requires search and replace")?;
        }
        MutationKind::ParameterTune => {
            require_search_replace(mutation, "parameter tuning requires search and replace")?;
        }
        MutationKind::AddTestSkeleton => {
            let append = mutation
                .append
                .as_ref()
                .ok_or_else(|| "test skeleton requires append payload".to_string())?;
            if !mutation.target_file.starts_with("tests/") {
                return Err("test skeleton target must be inside tests/".to_string());
            }
            if !append.contains("#[test]") || !append.contains("fn ") {
                return Err("test skeleton must contain a test function".to_string());
            }
        }
        MutationKind::AddMetricField => {
            let append = mutation
                .append
                .as_ref()
                .ok_or_else(|| "metric field mutation requires append payload".to_string())?;
            if !append.trim_start().starts_with("//") {
                return Err("metric field mutation is comment-only in this phase".to_string());
            }
        }
        MutationKind::AddUnitTest | MutationKind::AddReplayAssertion => {
            let append = mutation
                .append
                .as_ref()
                .ok_or_else(|| "generated test mutation requires append payload".to_string())?;
            if !mutation.target_file.starts_with("tests/") {
                return Err("generated test target must be inside tests/".to_string());
            }
            if !append.contains("#[test]") || !append.contains("fn ") {
                return Err("generated test mutation must contain a test function".to_string());
            }
        }
        MutationKind::AddLearningSummaryField => {
            require_search_replace(
                mutation,
                "learning summary field mutation requires search and replace",
            )?;
            if !mutation.target_file.contains("learning")
                && !mutation.target_file.contains("report")
                && mutation.target_file != "src/evolution/metrics.rs"
            {
                return Err(
                    "learning summary field target must be learning/report related".to_string(),
                );
            }
        }
        MutationKind::AddMetricUpdate => {
            require_search_replace(
                mutation,
                "metric update mutation requires search and replace",
            )?;
            if mutation.target_file == "src/evolution/metrics.rs"
                || mutation.target_file.starts_with("src/evolution/")
                || mutation.target_file.starts_with("src/runtime")
            {
                // allowed
            } else {
                return Err(
                    "metric update target must be metrics or safe evolution/runtime file"
                        .to_string(),
                );
            }
        }
    }

    Ok(())
}

fn validate_target_path(target_file: &str, kind: MutationKind) -> Result<(), String> {
    let path = Path::new(target_file);
    if path.is_absolute() {
        return Err("target file must be relative".to_string());
    }
    if target_file == "Cargo.toml" || target_file.ends_with("/Cargo.toml") {
        return Err("Cargo.toml mutation is forbidden in this phase".to_string());
    }
    let allowed_prefix = match kind {
        MutationKind::AddTestSkeleton
        | MutationKind::AddUnitTest
        | MutationKind::AddReplayAssertion => target_file.starts_with("tests/"),
        _ => target_file.starts_with("src/"),
    };
    if !allowed_prefix {
        return Err(match kind {
            MutationKind::AddTestSkeleton
            | MutationKind::AddUnitTest
            | MutationKind::AddReplayAssertion => "target file must be inside tests/".to_string(),
            _ => "target file must be inside an allowed source directory".to_string(),
        });
    }
    if target_file.contains("src/core/")
        || target_file == "src/lib.rs"
        || target_file == "src/main.rs"
    {
        return Err("core mutation is forbidden".to_string());
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("target file must not escape the project".to_string());
    }
    if path.exists() {
        let size = std::fs::metadata(path)
            .map_err(|error| format!("failed to inspect target file: {error}"))?
            .len();
        if size > MAX_TARGET_FILE_BYTES {
            return Err("target file too large".to_string());
        }
    }
    Ok(())
}

fn validate_payload_size(mutation: &MutationContract) -> Result<(), String> {
    let size = mutation
        .search
        .as_ref()
        .map(|value| value.len())
        .unwrap_or(0)
        + mutation
            .replace
            .as_ref()
            .map(|value| value.len())
            .unwrap_or(0)
        + mutation
            .append
            .as_ref()
            .map(|value| value.len())
            .unwrap_or(0);
    if size > MAX_MUTATION_PAYLOAD_BYTES {
        return Err("mutation payload too large".to_string());
    }
    Ok(())
}

fn require_search_replace(mutation: &MutationContract, message: &str) -> Result<(), String> {
    if mutation
        .search
        .as_ref()
        .map(|value| value.is_empty())
        .unwrap_or(true)
        || mutation.replace.is_none()
    {
        return Err(message.to_string());
    }
    Ok(())
}
