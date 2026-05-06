use crate::contracts::{MutationContract, MutationKind, MutationPlan};

const GENERATED_TEST_TARGET: &str = "tests/evolution_generated_tests.rs";

pub fn generate_add_unit_test(plan: &MutationPlan) -> MutationContract {
    MutationContract {
        id: format!("mutation:{}", plan.id),
        kind: MutationKind::AddUnitTest,
        target_file: GENERATED_TEST_TARGET.to_string(),
        search: None,
        replace: None,
        append: Some(format!(
            "#[test]\nfn eva_generated_{}_deterministic() {{\n    let digest = \"{}\";\n    assert_eq!(digest.len(), 64);\n    assert!(digest.chars().all(|ch| ch.is_ascii_hexdigit()));\n}}\n",
            test_suffix(plan),
            fixed_hex(plan)
        )),
        reason: format!("add deterministic generated unit test for {}", plan.id),
        expected_gain: plan.expected_gain.clamp(0.0, 1.0),
        risk: plan.estimated_risk.clamp(0.0, 1.0),
    }
}

pub fn generate_add_replay_assertion(plan: &MutationPlan) -> MutationContract {
    MutationContract {
        id: format!("mutation:{}", plan.id),
        kind: MutationKind::AddReplayAssertion,
        target_file: GENERATED_TEST_TARGET.to_string(),
        search: None,
        replace: None,
        append: Some(format!(
            "#[test]\nfn eva_generated_{}_replay_assertion() {{\n    let fixture = [(\"candidate\", true), (\"replay\", true), (\"failed\", false)];\n    let passed = fixture.iter().filter(|(_, ok)| *ok).count();\n    assert_eq!(passed, 2);\n    assert!(fixture.iter().any(|(name, _)| *name == \"replay\"));\n}}\n",
            test_suffix(plan)
        )),
        reason: format!("add deterministic replay assertion for {}", plan.id),
        expected_gain: plan.expected_gain.clamp(0.0, 1.0),
        risk: plan.estimated_risk.clamp(0.0, 1.0),
    }
}

pub fn generate_add_learning_summary_field(plan: &MutationPlan) -> MutationContract {
    MutationContract {
        id: format!("mutation:{}", plan.id),
        kind: MutationKind::AddLearningSummaryField,
        target_file: "src/evolution/metrics.rs".to_string(),
        search: Some(
            "total regression patterns: {}\\ntotal success patterns: {}\\nmutation dedup count: {}\\ntop risky files: {}\\ntop successful files: {}\",\n        regressions.len(),\n        successes.len(),\n        dedup_entries.len(),\n        risky,\n        successful".to_string(),
        ),
        replace: Some(
            "total regression patterns: {}\\ntotal success patterns: {}\\nmutation dedup count: {}\\ntop risky files: {}\\ntop successful files: {}\\nlearning source count: {}\",\n        regressions.len(),\n        successes.len(),\n        dedup_entries.len(),\n        risky,\n        successful,\n        regressions.len() + successes.len()".to_string(),
        ),
        append: None,
        reason: format!("extend learning summary output for {}", plan.id),
        expected_gain: plan.expected_gain.clamp(0.0, 1.0),
        risk: plan.estimated_risk.clamp(0.0, 1.0),
    }
}

pub fn generate_add_metric_update(plan: &MutationPlan) -> MutationContract {
    MutationContract {
        id: format!("mutation:{}", plan.id),
        kind: MutationKind::AddMetricUpdate,
        target_file: "src/evolution/metrics.rs".to_string(),
        search: Some(
            "pub const DEFAULT_METRICS_PATH: &str = \"memory/metrics.json\";\n".to_string(),
        ),
        replace: Some(
            "pub const DEFAULT_METRICS_PATH: &str = \"memory/metrics.json\";\npub const EVA_REPORTS_DIR: &str = \"memory/reports\";\n".to_string(),
        ),
        append: None,
        reason: format!("add compact metric/report constant for {}", plan.id),
        expected_gain: plan.expected_gain.clamp(0.0, 1.0),
        risk: plan.estimated_risk.clamp(0.0, 1.0),
    }
}

fn test_suffix(plan: &MutationPlan) -> String {
    plan.id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_ascii_lowercase()
}

fn fixed_hex(plan: &MutationPlan) -> String {
    let seed = format!(
        "{:x}",
        plan.id.len() * 7919 + plan.target_file.len() * 104_729
    );
    seed.repeat(64 / seed.len().max(1) + 1)
        .chars()
        .take(64)
        .collect()
}
