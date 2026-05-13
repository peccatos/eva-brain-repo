use crate::contracts::TuiState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiScreen {
    Dashboard,
    Runs,
    Candidates,
    Metrics,
    Release,
    Logs,
    Help,
}

impl TuiScreen {
    pub fn from_key(key: &str) -> Self {
        match key {
            "1" => Self::Dashboard,
            "2" => Self::Runs,
            "3" => Self::Candidates,
            "4" => Self::Metrics,
            "5" => Self::Release,
            "6" => Self::Logs,
            "7" | "h" | "H" => Self::Help,
            _ => Self::Dashboard,
        }
    }
}

pub fn render_screen(state: &TuiState, screen: TuiScreen, width: usize) -> String {
    let mut output = String::new();
    output.push_str("EVA Operator TUI\n");
    output.push_str("1 Dashboard | 2 Runs | 3 Candidates | 4 Metrics | 5 Release | 6 Logs | 7 Help | r refresh | q quit\n\n");
    match screen {
        TuiScreen::Dashboard => render_dashboard(state, &mut output),
        TuiScreen::Runs => render_runs(state, &mut output, width),
        TuiScreen::Candidates => render_candidates(state, &mut output, width),
        TuiScreen::Metrics => render_metrics(state, &mut output),
        TuiScreen::Release => render_release(state, &mut output),
        TuiScreen::Logs => render_logs(state, &mut output, width),
        TuiScreen::Help => render_help(&mut output),
    }
    output
}

fn render_dashboard(state: &TuiState, output: &mut String) {
    let dashboard = &state.dashboard;
    output.push_str("Dashboard\n");
    output.push_str(&format!(
        "runtime_status={} runtime_validation={} autonomy={} next={} campaign_allowed={}\n",
        dashboard.runtime_status,
        dashboard.runtime_validation_status,
        dashboard.autonomy_level,
        dashboard.allowed_next_autonomy_level,
        dashboard.campaign_mode_allowed
    ));
    output.push_str(&format!(
        "latest_run={} last_replay={} candidates={} ready={} blocked={} quarantined={} duplicate={} unreplayable={} sandbox_leaks={}\n",
        dashboard.latest_run_id.as_deref().unwrap_or("missing"),
        dashboard.last_replay_status,
        dashboard.candidate_count,
        dashboard.ready_candidates,
        dashboard.blocked_candidates,
        dashboard.quarantined_candidates,
        dashboard.duplicate_candidates,
        dashboard.unreplayable_candidates,
        dashboard.sandbox_leak_count
    ));
    output.push_str(&format!("release_status={}\n", dashboard.release_status));
    output.push_str(&format!("warnings={}\n", join_or_none(&dashboard.warnings)));
    output.push_str(&format!(
        "missing_green_conditions={}\n",
        join_or_none(&dashboard.missing_green_conditions)
    ));
    output.push_str(&format!("blockers={}\n", join_or_none(&dashboard.blockers)));
}

fn render_runs(state: &TuiState, output: &mut String, width: usize) {
    output.push_str("Runs\n");
    if state.runs.is_empty() {
        output.push_str("missing run history\n");
        return;
    }
    for run in &state.runs {
        output.push_str(&truncate(
            &format!(
                "{} status={} replay={} cargo_test={:?} cargo_run={:?} duplicate={} candidate={} promoted={} reason={}",
                run.run_id,
                run.status,
                run.replay_status,
                run.cargo_test_ok,
                run.cargo_run_ok,
                run.duplicate_rejected,
                run.candidate,
                run.promoted,
                run.reason
            ),
            width,
        ));
        output.push('\n');
    }
}

fn render_candidates(state: &TuiState, output: &mut String, width: usize) {
    output.push_str("Candidates\n");
    if state.candidates.is_empty() {
        output.push_str("empty candidate queue\n");
        return;
    }
    for candidate in &state.candidates {
        output.push_str(&truncate(
            &format!(
                "{} state={} kind={} class={} target={} score={:.2} risk={:.2} promotion={} allowed={} replay={} cargo_test={:?} cargo_run={:?} duplicate={} promoted={} reason={} updated={}",
                candidate.run_id,
                candidate.state,
                candidate.mutation_kind,
                candidate.mutation_class,
                candidate.target_file,
                candidate.score,
                candidate.risk,
                candidate.promotion_eligibility,
                candidate.promotion_allowed,
                candidate.replay_status,
                candidate.cargo_test_ok,
                candidate.cargo_run_ok,
                candidate.duplicate_rejected,
                candidate.promoted,
                candidate.block_reason,
                candidate.updated_at
            ),
            width,
        ));
        output.push('\n');
    }
}

fn render_metrics(state: &TuiState, output: &mut String) {
    let metrics = &state.metrics;
    output.push_str("Metrics\n");
    output.push_str(&format!(
        "total_runs={} passed_runs={} failed_runs={} safety_rejected={} duplicate_rejected={}\n",
        metrics.total_runs,
        metrics.passed_runs,
        metrics.failed_runs,
        metrics.safety_rejected_runs,
        metrics.duplicate_rejected_runs
    ));
    output.push_str(&format!(
        "replay_passed={} replay_failed={} candidates={} promoted={} average_score={:.2}\n",
        metrics.replay_passed,
        metrics.replay_failed,
        metrics.candidate_count,
        metrics.promoted_count,
        metrics.average_score
    ));
    output.push_str(&format!(
        "pass_ratio={:.2} replay_pass_ratio={:.2}\n",
        metrics.pass_ratio, metrics.replay_pass_ratio
    ));
}

fn render_release(state: &TuiState, output: &mut String) {
    let release = &state.release;
    output.push_str("Release\n");
    output.push_str(&format!(
        "approved_candidate_exists={} release_bundle_exists={} latest_candidate={}\n",
        release.approved_release_candidate_exists,
        release.release_bundle_exists,
        release
            .latest_release_candidate
            .as_deref()
            .unwrap_or("missing")
    ));
    output.push_str(&format!(
        "operator_approval={} preflight={} release_health={} green_gate={}\n",
        release.operator_approval_state,
        release.preflight_gate_status,
        release.release_health,
        release.green_gate_readiness
    ));
    output.push_str(&format!("warnings={}\n", join_or_none(&release.warnings)));
    output.push_str(&format!(
        "missing_green_conditions={}\n",
        join_or_none(&release.missing_green_conditions)
    ));
    output.push_str(&format!("blockers={}\n", join_or_none(&release.blockers)));
}

fn render_logs(state: &TuiState, output: &mut String, width: usize) {
    output.push_str("Logs\n");
    if state.logs.is_empty() {
        output.push_str("no recent events\n");
        return;
    }
    for line in &state.logs {
        output.push_str(&truncate(line, width));
        output.push('\n');
    }
}

fn render_help(output: &mut String) {
    output.push_str("Help\n");
    output.push_str("q / Esc = quit\n");
    output.push_str(
        "1 = Dashboard\n2 = Runs\n3 = Candidates\n4 = Metrics\n5 = Release\n6 = Logs\n7 = Help\n",
    );
    output.push_str("r = refresh\nh = help\n");
}

fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(",")
    }
}

fn truncate(value: &str, width: usize) -> String {
    let max = width.max(40).saturating_sub(1);
    if value.chars().count() <= max {
        return value.to_string();
    }
    value
        .chars()
        .take(max.saturating_sub(3))
        .collect::<String>()
        + "..."
}
