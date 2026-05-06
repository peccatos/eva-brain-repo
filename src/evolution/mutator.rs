use std::fs;
use std::path::Path;

use crate::contracts::mutation::{MutationContract, MutationKind};
use crate::evolution::validator::{validate_mutation, MAX_TARGET_FILE_BYTES};

pub fn apply_mutation(base_dir: &str, mutation: &MutationContract) -> Result<(), String> {
    validate_mutation(mutation)?;

    let path = Path::new(base_dir).join(&mutation.target_file);
    if !path.exists()
        && !matches!(
            mutation.kind,
            MutationKind::AddTestSkeleton
                | MutationKind::AddUnitTest
                | MutationKind::AddReplayAssertion
        )
    {
        return Err(format!(
            "target file does not exist: {}",
            mutation.target_file
        ));
    }
    if path.exists() {
        let size = fs::metadata(&path)
            .map_err(|error| format!("failed to inspect target file: {error}"))?
            .len();
        if size > MAX_TARGET_FILE_BYTES {
            return Err("target file too large".to_string());
        }
    }

    let mut content = if path.exists() {
        fs::read_to_string(&path).map_err(|error| format!("failed to read target file: {error}"))?
    } else {
        String::new()
    };

    match mutation.kind {
        MutationKind::AppendComment
        | MutationKind::AddMetricField
        | MutationKind::AddTestSkeleton
        | MutationKind::AddUnitTest
        | MutationKind::AddReplayAssertion => {
            let append = mutation
                .append
                .as_ref()
                .ok_or_else(|| "missing append payload".to_string())?;
            content.push('\n');
            content.push_str(append);
            content.push('\n');
        }
        MutationKind::ReplaceText
        | MutationKind::ParameterTune
        | MutationKind::AddLearningSummaryField
        | MutationKind::AddMetricUpdate => {
            let search = mutation
                .search
                .as_ref()
                .ok_or_else(|| "missing search pattern".to_string())?;
            let replace = mutation
                .replace
                .as_ref()
                .ok_or_else(|| "missing replacement".to_string())?;
            if !content.contains(search) {
                return Err("search pattern not found".to_string());
            }
            content = content.replacen(search, replace, 1);
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create target parent: {error}"))?;
    }
    fs::write(&path, content).map_err(|error| format!("failed to write target file: {error}"))?;
    Ok(())
}
