use crate::contracts::{ReleaseBundle, ReleaseManifest, ReleasePreflightReport, RollbackManifest};

pub fn render_release_changelog(
    bundle: &ReleaseBundle,
    preflight: &ReleasePreflightReport,
    manifest: &ReleaseManifest,
    rollback: &RollbackManifest,
) -> String {
    let notes = if bundle.safety_notes.is_empty() {
        "(none)".to_string()
    } else {
        bundle
            .safety_notes
            .iter()
            .map(|note| format!("- {note}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let blockers = if preflight.blockers.is_empty() {
        "(none)".to_string()
    } else {
        preflight
            .blockers
            .iter()
            .map(|blocker| format!("- {blocker}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let rollback_notes = if rollback.notes.is_empty() {
        "(none)".to_string()
    } else {
        rollback
            .notes
            .iter()
            .map(|note| format!("- {note}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "# EVA Release Changelog\n\nrelease_id={}\nsource_run_id={}\ntarget_file={}\nmutation_kind={}\nmutation_class={}\nscore={:.2}\nrisk={:.2}\nreplay_status={}\napproval_status={}\npromotion_queue_state={}\nallowed={}\nreason_ru={}\n\n## Безопасность\n{}\n\n## Preflight Blockers\n{}\n\n## Manifest\napproved={}\nauto_promote={}\nsource_mutated={}\nrollback_available={}\nchangelog_available={}\n\n## Rollback\nrollback_type={}\nrollback_available={}\noriginal_candidate_report_path={}\n{}\n",
        bundle.release_id,
        bundle.source_run_id,
        bundle.target_file,
        bundle.mutation_kind,
        bundle.mutation_class,
        bundle.score,
        bundle.risk,
        bundle.replay_status,
        bundle.approval_status,
        bundle.promotion_queue_state,
        preflight.allowed,
        preflight.reason_ru,
        notes,
        blockers,
        manifest.approved,
        manifest.auto_promote,
        manifest.source_mutated,
        manifest.rollback_available,
        manifest.changelog_available,
        rollback.rollback_type,
        rollback.rollback_available,
        rollback
            .original_candidate_report_path
            .as_deref()
            .unwrap_or("none"),
        rollback_notes,
    )
}
