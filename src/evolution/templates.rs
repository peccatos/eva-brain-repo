use crate::contracts::sha256_digest;
use crate::contracts::{MutationContract, MutationKind, MutationPlan};

const GENERATED_TEST_TARGET: &str = "tests/evolution_generated_tests.rs";
const MAX_TEST_NAME_LEN: usize = 80;

pub fn generate_add_unit_test(plan: &MutationPlan) -> MutationContract {
    MutationContract {
        id: format!("mutation:{}", plan.id),
        kind: MutationKind::AddUnitTest,
        target_file: GENERATED_TEST_TARGET.to_string(),
        search: None,
        replace: None,
        append: Some(format!(
            "#[test]\nfn {}() {{\n    let digest = \"{}\";\n    assert_eq!(digest.len(), 64);\n    assert!(digest.chars().all(|ch| ch.is_ascii_hexdigit()));\n}}\n",
            normalized_generated_test_name(plan, "deterministic"),
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
            "#[test]\nfn {}() {{\n    let fixture = [(\"candidate\", true), (\"replay\", true), (\"failed\", false)];\n    let passed = fixture.iter().filter(|(_, ok)| *ok).count();\n    assert_eq!(passed, 2);\n    assert!(fixture.iter().any(|(name, _)| *name == \"replay\"));\n}}\n",
            normalized_generated_test_name(plan, "replay")
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

pub fn normalized_generated_test_name(plan: &MutationPlan, flavor: &str) -> String {
    let semantic = semantic_fragment(&plan.id);
    let digest = stable_short_digest(plan, flavor);
    let flavor = normalize_identifier_fragment(flavor, 16);
    let fixed_prefix = "eva_generated__";
    let fixed_separators = "__";
    let reserved = fixed_prefix.len() + fixed_separators.len() + digest.len() + flavor.len();
    let max_semantic_len = MAX_TEST_NAME_LEN.saturating_sub(reserved).max(1);
    let semantic = normalize_identifier_fragment(&semantic, max_semantic_len);
    let mut name = format!("eva_generated_{}_{}_{}", semantic, digest, flavor);
    if name.len() > MAX_TEST_NAME_LEN {
        name.truncate(MAX_TEST_NAME_LEN);
        while name.ends_with('_') {
            name.pop();
        }
    }
    name
}

fn semantic_fragment(plan_id: &str) -> String {
    let normalized = plan_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_ascii_lowercase();
    let first = normalized.split('_').find(|segment| !segment.is_empty());
    match first {
        Some("recombined") => "recombined".to_string(),
        Some(segment) => segment.to_string(),
        None => "generated".to_string(),
    }
}

fn normalize_identifier_fragment(value: &str, max_len: usize) -> String {
    let mut normalized = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .to_ascii_lowercase();
    if normalized.is_empty() {
        normalized = "generated".to_string();
    }
    if !normalized
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
    {
        normalized.insert(0, 'g');
    }
    normalized.truncate(max_len);
    normalized.trim_matches('_').to_string()
}

fn stable_short_digest(plan: &MutationPlan, flavor: &str) -> String {
    sha256_digest(&format!(
        "{}:{}:{:?}:{}",
        plan.id, plan.target_file, plan.mutation_kind, flavor
    ))
    .chars()
    .take(6)
    .collect()
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
