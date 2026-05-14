use crate::{
    build_project_phase_runtime_output, CycleInput, ModelChatMessage, ModelChatOptions,
    OpenAiModelClient, OpenAiModelConfig, RuntimeCycleRunner, DEFAULT_MODEL_ID, DEFAULT_MODEL_NAME,
    DEFAULT_MODEL_URL,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8765";
pub const DEFAULT_RUNTIME_CONFIG_PATH: &str = "eva.runtime.json";
pub const RUNTIME_CLI_HELP: &str = r#"EVA runtime commands:
  cargo run
      Show this command list.

  cargo run -- --once
      Run one local deterministic runtime cycle and print JSON.

  cargo run -- tui
  cargo run -- --tui
      Open the read-only EVA operator TUI. In non-interactive mode, print one dashboard snapshot.

  cargo run -- status
      Print the current runtime validation status.

  cargo run -- task <GOAL>
      Create a governed production-agent task.

  cargo run -- tasks
      List governed production-agent tasks.

  cargo run -- task-show <TASK_ID>
      Show one governed production-agent task.

  cargo run -- inspect
      Inspect the local workspace for the production-agent loop.

  cargo run -- repo-map
      Build a deterministic local repo map for agent planning.

  cargo run -- fix <TARGET_PATH>
      Detect one actionable local repair, create a governed proposal, store evidence, and print a concise fix report.

  cargo run -- fix <TARGET_PATH> --dry-run
  cargo run -- fix <TARGET_PATH> --apply
  cargo run -- fix <TARGET_PATH> --only cargo-check|ci|tests|docs
      Run the product-facing safe fix facade in dry-run or apply mode.

  cargo run -- plan <TASK_ID>
      Create a deterministic agent plan.

  cargo run -- propose <TASK_ID>
      Create or refuse a structured patch proposal.

  cargo run -- proposal-show <PROPOSAL_ID>
      Print a structured proposal preview without applying it.

  cargo run -- apply --dry-run <PROPOSAL_ID>
      Preview safe apply without mutating files or creating snapshots.

  cargo run -- approve <PROPOSAL_ID>
      Operator-approve a safe proposal.

  cargo run -- apply <PROPOSAL_ID>
      Apply an approved proposal with snapshot metadata.

  cargo run -- validate
      Run allowlisted validation commands.

  cargo run -- report <TASK_ID>
      Generate a local agent evidence report.

  cargo run -- pr-summary <TASK_ID>
      Generate a local PR summary without push or PR creation.

  cargo run -- agent-readiness
      Print production-agent readiness.

  cargo run -- agent-v2-readiness
      Print production-agent v2 readiness.

  cargo run -- llm-health
      Print LLM provider health without exposing secrets.

  cargo run -- task-outcomes
  cargo run -- task-outcome <TASK_ID>
      Inspect real task outcome memory.

  cargo run -- outcome-analyze
  cargo run -- patterns
  cargo run -- strategy-memory
  cargo run -- fitness [TASK_ID]
  cargo run -- strategy-select <GOAL>
      Analyze outcomes, extract patterns, score fitness, and select safe strategies.

  cargo run -- self-improve propose
      Create a governed self-improvement proposal only; never apply automatically.

  cargo run -- --evolve
      Run one bounded self-evolution cycle in a disposable sandbox.

  cargo run -- --plan-evolution
      Print graph-guided plans without mutating or creating a sandbox.

  cargo run -- --evolve-planned
      Run one graph-guided bounded evolution cycle in a disposable sandbox.

  cargo run -- --evolve-planned-n <N>
      Run N graph-guided bounded evolution cycles in disposable sandboxes.

  cargo run -- --evolution-benchmark <N>
      Run N planned cycles and write aggregate benchmark reports.

  cargo run -- --autonomy-status
      Print current lightweight autonomy gate status.

  cargo run -- --metrics
      Print compact evolution metrics.

  cargo run -- --metrics-refresh
      Recompute compact evolution metrics from logs and memory files.

  cargo run -- --portfolio
      Print mutation portfolio summary with saturation state.

  cargo run -- --portfolio-refresh
      Rebuild mutation portfolio from stored memory artifacts.

  cargo run -- --strategy-portfolio
      Print strategy portfolio summary.

  cargo run -- --strategy-portfolio-refresh
      Rebuild strategy portfolio from stored memory artifacts.

  cargo run -- --evolution-policy
      Print current deterministic evolution policy.

  cargo run -- --quality-report <RUN_ID>
      Print quality metrics v2 for a stored run.

  cargo run -- --evolution-hygiene
      Print latest evolution hygiene report and persist it under memory/hygiene/.

  cargo run -- --hygiene-plan
      Print recommended safe cleanup actions from hygiene analysis.

  cargo run -- --hygiene-fix-generated-tests
      Rename only long eva_generated_* tests in tests/evolution_generated_tests.rs with rollback on validation failure.

  cargo run -- --ingest-corpus <PATH>
      Read-only ingest a local corpus folder with the default corpus contract.

  cargo run -- --ingest-corpus-contract <PATH>
      Ingest a local corpus from an explicit JSON corpus contract.

  cargo run -- --corpus-summary <CORPUS_ID>
      Print stored local corpus summary JSON.

  cargo run -- --list-corpora
      List stored local corpus ids.

  cargo run -- --suggest-strategy-tasks <CORPUS_ID>
      Generate safe suggested strategy tasks from stored corpus patterns.

  cargo run -- --list-suggested-tasks
      List stored suggested task ids.

  cargo run -- --learning-summary
      Print compact learning memory summary.

  cargo run -- --last-report
      Print the latest Russian evolution report.

  cargo run -- --report <RUN_ID>
      Print a specific Russian evolution report.

  cargo run -- --report-refresh <RUN_ID>
      Rebuild a Russian evolution report from stored artifacts.

  cargo run -- --review-candidate <RUN_ID>
      Print candidate review with Russian summary and promotion readiness.

  cargo run -- --candidate-diff <RUN_ID>
      Print the bounded candidate payload or search/replace diff.

  cargo run -- --list-candidates
      List stored manual-promotion candidates.

  cargo run -- --replay <RUN_ID>
      Replay a stored candidate in a fresh sandbox.

  cargo run -- --promote <RUN_ID>
      Manually promote a gated candidate into the real project.

  cargo run -- --ingest-repo <PATH>
      Read local Rust repo patterns into memory/graph.json without mutating the repo.

  cargo run -- --run-task <PATH_TO_TASK_JSON>
      Validate, persist, and run a bounded evolution campaign from a task contract.

  cargo run -- --campaign <TASK_ID>
      Run a stored bounded evolution campaign by task id.

  cargo run -- --last-campaign-report
      Print the latest Russian campaign report.

  cargo run -- --campaign-report <CAMPAIGN_ID>
      Print or rebuild a specific Russian campaign report by campaign id.

  cargo run -- --adjust-task-from-campaign <CAMPAIGN_ID>
      Create a safe adjusted task draft from a zero-yield campaign.

  cargo run -- --last-task-adjustment
      Print the latest Russian task-adjustment report.

  cargo run -- --list-adjusted-tasks
      List stored adjusted task ids.

  cargo run -- --campaign-recombine-preview <TASK_PATH>
      Preview task-compatible recombination fallback diagnostics without mutation or sandbox.

  cargo run -- --evolve-bounded --task <PATH> --cycles <N>
      Run bounded campaign evolution loop with policy, replay/review, feedback, and adjustment drafts.

  cargo run -- --last-bounded-run
      Print the latest Russian bounded-run report.

  cargo run -- --bounded-run-report <BOUNDED_RUN_ID>
      Print a specific Russian bounded-run report.

  cargo run -- --list-bounded-runs
      List stored bounded run ids.

  cargo run -- --refresh-promotion-queue
      Rebuild the deterministic promotion queue from local candidate memory.

  cargo run -- --promotion-queue
      Print the current Russian promotion queue summary.

  cargo run -- --promotion-ready
      Print promotion-queue items that are ready for manual review.

  cargo run -- --promotion-blocked
      Print promotion-queue items that are not ready for manual review.

  cargo run -- --candidate-lifecycle <RUN_ID>
      Print deterministic lifecycle state for one candidate.

  cargo run -- --supervise-task <PATH_TO_TASK_JSON> [--max-rounds N]
      Run supervised operator rounds on top of bounded evolution without auto-promotion.

  cargo run -- --last-supervised-run
      Print the latest Russian supervised-run report.

  cargo run -- --supervised-run-report <SUPERVISED_RUN_ID>
      Print a specific Russian supervised-run report.

  cargo run -- --list-supervised-runs
      List stored supervised run ids.

  cargo run -- --eva-status
      Print compact local EVA operator status.

  cargo run -- --proof-report
      Print the current Russian proof/demo report.

  cargo run -- --proof-json
      Print deterministic proof/demo JSON.

  cargo run -- --demo
      Safely refresh local operator proof artifacts and print a repeatable demo snapshot.

  cargo run -- --approve-candidate <RUN_ID> --reason <TEXT>
      Append an operator approval record for a candidate that already satisfies governance safety.

  cargo run -- --reject-candidate <RUN_ID> --reason <TEXT>
      Append an operator rejection record for a candidate.

  cargo run -- --defer-candidate <RUN_ID> --reason <TEXT>
      Append an operator deferred record for a candidate.

  cargo run -- --approval-status <RUN_ID>
      Print the latest operator approval state for one candidate.

  cargo run -- --approval-log
      Print the combined append-only governance approval log.

  cargo run -- --governance-status
      Print compact governance runtime status.

  cargo run -- --promotion-ready-approved
      Print approved candidates that still pass the governance trust gate.

  cargo run -- --promote-approved <RUN_ID>
      Run manual promotion only after operator approval and governance trust gate pass.

  cargo run -- --release-proposal
      Build and print a deterministic local release proposal.

  cargo run -- --release-proposal-json
      Build and print a deterministic local release proposal JSON.

  cargo run -- --proof-snapshot
      Capture and print a governance proof snapshot markdown.

  cargo run -- --proof-snapshot-json
      Capture and print a governance proof snapshot JSON.

  cargo run -- --release-preflight <RUN_ID>
      Validate a governance-approved replay-verified candidate for metadata-only release.

  cargo run -- --release-bundle <RUN_ID>
      Build a deterministic local release bundle for a safe approved candidate.

  cargo run -- --release-approve <RUN_ID>
      Operator-approve a ready replay-ok candidate for release metadata.

  cargo run -- --release-manifest <RELEASE_ID>
      Print a stored release manifest JSON.

  cargo run -- --release-changelog <RELEASE_ID>
      Print a stored Russian release changelog.

  cargo run -- --rollback-manifest <RELEASE_ID>
      Print a stored rollback manifest JSON.

  cargo run -- --list-releases
      List stored release ids.

  cargo run -- --last-release
      Print the latest stored release report and changelog.

  cargo run -- --release-status
      Print compact release runtime status.

  cargo run -- --release-health
      Print compact release health index.

  cargo run -- --release-health-json
      Print release health JSON.

  cargo run -- --artifact-audit
      Print metadata-only runtime artifact audit.

  cargo run -- --artifact-audit-json
      Print runtime artifact audit JSON.

  cargo run -- --determinism-audit
      Print deterministic structure audit.

  cargo run -- --determinism-audit-json
      Print deterministic structure audit JSON.

  cargo run -- --preflight-gate
      Print combined local release gate v2.

  cargo run -- --preflight-gate-json
      Print combined local release gate v2 JSON.

  cargo run -- --release-ledger
      Print append-only release attempt ledger.

  cargo run -- --release-ledger-json
      Print append-only release attempt ledger JSON.

  cargo run -- --record-release-attempt <RELEASE_ID>
      Append a metadata-only release attempt record.

  cargo run -- --future-phases
      Print static future phase registry.

  cargo run -- --future-phases-json
      Print static future phase registry JSON.

  cargo run -- --operator-runbook
      Print concise Russian operator runbook.

  cargo run -- --ops-status
      Print combined local operations status.

  cargo run -- --ops-json
      Print deterministic local operations status JSON.

  cargo run -- --pr-package
      Build a metadata-only local PR package.

  cargo run -- --last-pr-package
      Print the latest PR package markdown.

  cargo run -- --list-pr-packages
      List stored PR package ids.

  cargo run -- --external-patch-package <REPO_PATH>
      Build a metadata-only external repository patch package.

  cargo run -- --last-external-patch-package
      Print the latest external patch package markdown.

  cargo run -- --list-external-patch-packages
      List stored external patch package ids.

  cargo run -- --self-review-package
      Build a controlled self-review package without self-apply.

  cargo run -- --last-self-review-package
      Print the latest self-review package markdown.

  cargo run -- --list-self-review-packages
      List stored self-review package ids.

  cargo run -- --operator-console
      Print the combined operator console.

  cargo run -- --capability-policy
      Print deterministic capability policy JSON.

  cargo run -- --trust-decision
      Print deterministic trust decision JSON.

  cargo run -- --workspace-snapshot
      Build and print a metadata-only workspace snapshot.

  cargo run -- --last-workspace-snapshot
      Print the latest workspace snapshot JSON.

  cargo run -- --list-workspace-snapshots
      List stored workspace snapshot ids.

  cargo run -- --evidence-bundle
      Build and print a metadata-only evidence bundle.

  cargo run -- --last-evidence-bundle
      Print the latest evidence bundle JSON.

  cargo run -- --list-evidence-bundles
      List stored evidence bundle ids.

  cargo run -- --recovery-manifest
      Build and print a metadata-only recovery manifest.

  cargo run -- --last-recovery-manifest
      Print the latest recovery manifest JSON.

  cargo run -- --list-recovery-manifests
      List stored recovery manifest ids.

  cargo run -- --preflight-gate-v3
      Print composed trust/recovery preflight gate v3.

  cargo run -- --trust-proof-report
      Print unified Phase 14 trust proof report.

  cargo run -- --runtime-candidate
      Build and print the Phase 15 runtime v1.0 candidate manifest JSON.

  cargo run -- --runtime-validation
      Build and print the Phase 15 runtime validation JSON.

  cargo run -- --runtime-service
      Print local runtime service metadata JSON.

  cargo run -- --runtime-cli-contract
      Print stable runtime CLI contract JSON.

  cargo run -- --final-rc-report
      Print the final EVA Runtime v1.0 candidate markdown report.

  cargo run -- --distill-patterns
      Distill local-only successful and risky evolution patterns into memory/patterns/.

  cargo run -- --recombine-patterns
      Print top deterministic recombined hypotheses without mutation or sandbox creation.

  cargo run -- --evolve-recombined
      Run one recombined bounded evolution cycle in a disposable sandbox.

  cargo run -- --serve [--config eva.runtime.json]
      Start the HTTP runtime daemon. Defaults to 127.0.0.1:8765.

  cargo run -- --repo <REPO_URL>
      Run repo patch mode and write eva_output/report.md + summary.json.

  cargo run -- --serve --model-endpoint ID=MODEL@URL
      Add a local OpenAI-compatible model endpoint.

  cargo run -- --serve --model-file ID=/path/to/model.gguf --model-endpoint ID=MODEL@URL
      Guard an external local model endpoint by requiring a model file.

  cargo run --bin github_repo_discover -- --fixture fixtures/github_search_fixture.json
      Run offline benchmark discovery from the fixture.
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCliCommand {
    Help,
    Once,
    Tui,
    Status,
    Evolve,
    PlanEvolution,
    EvolvePlanned,
    EvolvePlannedN(usize),
    EvolutionBenchmark(usize),
    AutonomyStatus,
    Metrics,
    MetricsRefresh,
    Portfolio,
    PortfolioRefresh,
    StrategyPortfolio,
    StrategyPortfolioRefresh,
    EvolutionPolicy,
    QualityReport(String),
    EvolutionHygiene,
    HygienePlan,
    HygieneFixGeneratedTests,
    IngestCorpus(String),
    IngestCorpusContract(String),
    CorpusSummary(String),
    ListCorpora,
    SuggestStrategyTasks(String),
    ListSuggestedTasks,
    LearningSummary,
    LastReport,
    Report(String),
    ReportRefresh(String),
    ReviewCandidate(String),
    CandidateDiff(String),
    ListCandidates,
    Replay(String),
    Promote(String),
    IngestRepo(String),
    RunTask(String),
    Campaign(String),
    LastCampaignReport,
    CampaignReport(String),
    AdjustTaskFromCampaign(String),
    LastTaskAdjustment,
    ListAdjustedTasks,
    CampaignRecombinePreview(String),
    EvolveBounded {
        task_path: String,
        cycles: usize,
    },
    LastBoundedRun,
    BoundedRunReport(String),
    ListBoundedRuns,
    RefreshPromotionQueue,
    PromotionQueue,
    PromotionReady,
    PromotionBlocked,
    CandidateLifecycle(String),
    SuperviseTask {
        task_path: String,
        max_rounds: usize,
    },
    LastSupervisedRun,
    SupervisedRunReport(String),
    ListSupervisedRuns,
    EvaStatus,
    ProofReport,
    ProofJson,
    Demo,
    ApproveCandidate {
        run_id: String,
        reason: String,
    },
    RejectCandidate {
        run_id: String,
        reason: String,
    },
    DeferCandidate {
        run_id: String,
        reason: String,
    },
    ApprovalStatus(String),
    ApprovalLog,
    GovernanceStatus,
    PromotionReadyApproved,
    PromoteApproved(String),
    ReleasePreflight(String),
    ReleaseBundle(String),
    ReleaseApprove(String),
    ReleaseManifest(String),
    ReleaseChangelog(String),
    RollbackManifest(String),
    ListReleases,
    LastRelease,
    ReleaseStatus,
    ReleaseHealth,
    ReleaseHealthJson,
    ArtifactAudit,
    ArtifactAuditJson,
    DeterminismAudit,
    DeterminismAuditJson,
    PreflightGate,
    PreflightGateJson,
    ReleaseLedger,
    ReleaseLedgerJson,
    RecordReleaseAttempt(String),
    FuturePhases,
    FuturePhasesJson,
    OperatorRunbook,
    OpsStatus,
    OpsJson,
    PrPackage,
    LastPrPackage,
    ListPrPackages,
    ExternalPatchPackage(String),
    LastExternalPatchPackage,
    ListExternalPatchPackages,
    SelfReviewPackage,
    LastSelfReviewPackage,
    ListSelfReviewPackages,
    OperatorConsole,
    CapabilityPolicy,
    TrustDecision,
    WorkspaceSnapshot,
    LastWorkspaceSnapshot,
    ListWorkspaceSnapshots,
    EvidenceBundle,
    LastEvidenceBundle,
    ListEvidenceBundles,
    RecoveryManifest,
    LastRecoveryManifest,
    ListRecoveryManifests,
    PreflightGateV3,
    TrustProofReport,
    RuntimeCandidate,
    RuntimeValidation,
    RuntimeService,
    RuntimeCliContract,
    FinalRcReport,
    ReleaseProposal,
    ReleaseProposalJson,
    ProofSnapshot,
    ProofSnapshotJson,
    DistillPatterns,
    RecombinePatterns,
    EvolveRecombined,
    Serve(RuntimeDaemonConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDaemonConfig {
    pub listen_addr: String,
    pub default_model_id: String,
    pub models: Vec<OpenAiModelConfig>,
    pub managed_servers: Vec<ManagedServerConfig>,
}

impl Default for RuntimeDaemonConfig {
    fn default() -> Self {
        Self {
            listen_addr: DEFAULT_LISTEN_ADDR.to_string(),
            default_model_id: DEFAULT_MODEL_ID.to_string(),
            models: vec![OpenAiModelConfig::default()],
            managed_servers: Vec::new(),
        }
    }
}

fn parse_config_path(args: &[String]) -> Result<Option<String>, String> {
    let mut index = 0;
    let mut explicit_model_config = false;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--config requires a file path".to_string());
                };
                return Ok(Some(path.clone()));
            }
            value if value.starts_with("--config=") => {
                return Ok(Some(value.trim_start_matches("--config=").to_string()));
            }
            "--model-url" | "--model" | "--model-endpoint" | "--model-file" | "--start-server" => {
                explicit_model_config = true;
                index += 2;
            }
            value
                if value.starts_with("--model-url=")
                    || value.starts_with("--model=")
                    || value.starts_with("--model-endpoint=")
                    || value.starts_with("--model-file=")
                    || value.starts_with("--start-server=") =>
            {
                explicit_model_config = true;
                index += 1;
            }
            _ => index += 1,
        }
    }

    if let Ok(path) = std::env::var("EVA_RUNTIME_CONFIG") {
        if !path.trim().is_empty() {
            return Ok(Some(path));
        }
    }
    if !explicit_model_config && Path::new(DEFAULT_RUNTIME_CONFIG_PATH).exists() {
        return Ok(Some(DEFAULT_RUNTIME_CONFIG_PATH.to_string()));
    }
    Ok(None)
}

fn load_optional_runtime_config(path: Option<&str>) -> Result<Option<RuntimeDaemonConfig>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read runtime config {}: {}", path, error))?;
    let config = serde_json::from_str::<RuntimeDaemonConfig>(&contents)
        .map_err(|error| format!("failed to parse runtime config {}: {}", path, error))?;
    Ok(Some(config))
}

impl RuntimeCliCommand {
    pub fn parse_from_iter<I, S>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let raw_args = args.into_iter().map(Into::into).collect::<Vec<_>>();
        if raw_args.is_empty()
            || raw_args
                .iter()
                .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
        {
            return Ok(Self::Help);
        }
        if raw_args == ["--once"] {
            return Ok(Self::Once);
        }
        if raw_args == ["tui"] || raw_args == ["--tui"] {
            return Ok(Self::Tui);
        }
        if raw_args == ["status"] || raw_args == ["--status"] {
            return Ok(Self::Status);
        }
        if raw_args == ["--evolve"] {
            return Ok(Self::Evolve);
        }
        if raw_args == ["--plan-evolution"] {
            return Ok(Self::PlanEvolution);
        }
        if raw_args == ["--evolve-planned"] {
            return Ok(Self::EvolvePlanned);
        }
        if raw_args == ["--autonomy-status"] {
            return Ok(Self::AutonomyStatus);
        }
        if raw_args == ["--metrics"] {
            return Ok(Self::Metrics);
        }
        if raw_args == ["--metrics-refresh"] {
            return Ok(Self::MetricsRefresh);
        }
        if raw_args == ["--portfolio"] {
            return Ok(Self::Portfolio);
        }
        if raw_args == ["--portfolio-refresh"] {
            return Ok(Self::PortfolioRefresh);
        }
        if raw_args == ["--strategy-portfolio"] {
            return Ok(Self::StrategyPortfolio);
        }
        if raw_args == ["--strategy-portfolio-refresh"] {
            return Ok(Self::StrategyPortfolioRefresh);
        }
        if raw_args == ["--evolution-policy"] {
            return Ok(Self::EvolutionPolicy);
        }
        if raw_args == ["--evolution-hygiene"] {
            return Ok(Self::EvolutionHygiene);
        }
        if raw_args == ["--hygiene-plan"] {
            return Ok(Self::HygienePlan);
        }
        if raw_args == ["--hygiene-fix-generated-tests"] {
            return Ok(Self::HygieneFixGeneratedTests);
        }
        if raw_args == ["--list-corpora"] {
            return Ok(Self::ListCorpora);
        }
        if raw_args == ["--list-suggested-tasks"] {
            return Ok(Self::ListSuggestedTasks);
        }
        if raw_args == ["--learning-summary"] {
            return Ok(Self::LearningSummary);
        }
        if raw_args == ["--last-report"] {
            return Ok(Self::LastReport);
        }
        if raw_args == ["--list-candidates"] {
            return Ok(Self::ListCandidates);
        }
        if raw_args == ["--last-campaign-report"] {
            return Ok(Self::LastCampaignReport);
        }
        if raw_args == ["--last-task-adjustment"] {
            return Ok(Self::LastTaskAdjustment);
        }
        if raw_args == ["--list-adjusted-tasks"] {
            return Ok(Self::ListAdjustedTasks);
        }
        if raw_args == ["--last-bounded-run"] {
            return Ok(Self::LastBoundedRun);
        }
        if raw_args == ["--list-bounded-runs"] {
            return Ok(Self::ListBoundedRuns);
        }
        if raw_args == ["--refresh-promotion-queue"] {
            return Ok(Self::RefreshPromotionQueue);
        }
        if raw_args == ["--promotion-queue"] {
            return Ok(Self::PromotionQueue);
        }
        if raw_args == ["--promotion-ready"] {
            return Ok(Self::PromotionReady);
        }
        if raw_args == ["--promotion-blocked"] {
            return Ok(Self::PromotionBlocked);
        }
        if raw_args == ["--last-supervised-run"] {
            return Ok(Self::LastSupervisedRun);
        }
        if raw_args == ["--list-supervised-runs"] {
            return Ok(Self::ListSupervisedRuns);
        }
        if raw_args == ["--eva-status"] {
            return Ok(Self::EvaStatus);
        }
        if raw_args == ["--proof-report"] {
            return Ok(Self::ProofReport);
        }
        if raw_args == ["--proof-json"] {
            return Ok(Self::ProofJson);
        }
        if raw_args == ["--demo"] {
            return Ok(Self::Demo);
        }
        if raw_args == ["--approval-log"] {
            return Ok(Self::ApprovalLog);
        }
        if raw_args == ["--governance-status"] {
            return Ok(Self::GovernanceStatus);
        }
        if raw_args == ["--promotion-ready-approved"] {
            return Ok(Self::PromotionReadyApproved);
        }
        if raw_args == ["--list-releases"] {
            return Ok(Self::ListReleases);
        }
        if raw_args == ["--last-release"] {
            return Ok(Self::LastRelease);
        }
        if raw_args == ["--release-status"] {
            return Ok(Self::ReleaseStatus);
        }
        if raw_args == ["--release-health"] {
            return Ok(Self::ReleaseHealth);
        }
        if raw_args == ["--release-health-json"] {
            return Ok(Self::ReleaseHealthJson);
        }
        if raw_args == ["--artifact-audit"] {
            return Ok(Self::ArtifactAudit);
        }
        if raw_args == ["--artifact-audit-json"] {
            return Ok(Self::ArtifactAuditJson);
        }
        if raw_args == ["--determinism-audit"] {
            return Ok(Self::DeterminismAudit);
        }
        if raw_args == ["--determinism-audit-json"] {
            return Ok(Self::DeterminismAuditJson);
        }
        if raw_args == ["--preflight-gate"] {
            return Ok(Self::PreflightGate);
        }
        if raw_args == ["--preflight-gate-json"] {
            return Ok(Self::PreflightGateJson);
        }
        if raw_args == ["--release-ledger"] {
            return Ok(Self::ReleaseLedger);
        }
        if raw_args == ["--release-ledger-json"] {
            return Ok(Self::ReleaseLedgerJson);
        }
        if raw_args.len() == 2 && raw_args[0] == "--record-release-attempt" {
            return Ok(Self::RecordReleaseAttempt(raw_args[1].clone()));
        }
        if raw_args == ["--future-phases"] {
            return Ok(Self::FuturePhases);
        }
        if raw_args == ["--future-phases-json"] {
            return Ok(Self::FuturePhasesJson);
        }
        if raw_args == ["--operator-runbook"] {
            return Ok(Self::OperatorRunbook);
        }
        if raw_args == ["--ops-status"] {
            return Ok(Self::OpsStatus);
        }
        if raw_args == ["--ops-json"] {
            return Ok(Self::OpsJson);
        }
        if raw_args == ["--pr-package"] {
            return Ok(Self::PrPackage);
        }
        if raw_args == ["--last-pr-package"] {
            return Ok(Self::LastPrPackage);
        }
        if raw_args == ["--list-pr-packages"] {
            return Ok(Self::ListPrPackages);
        }
        if raw_args.len() == 2 && raw_args[0] == "--external-patch-package" {
            return Ok(Self::ExternalPatchPackage(raw_args[1].clone()));
        }
        if raw_args == ["--last-external-patch-package"] {
            return Ok(Self::LastExternalPatchPackage);
        }
        if raw_args == ["--list-external-patch-packages"] {
            return Ok(Self::ListExternalPatchPackages);
        }
        if raw_args == ["--self-review-package"] {
            return Ok(Self::SelfReviewPackage);
        }
        if raw_args == ["--last-self-review-package"] {
            return Ok(Self::LastSelfReviewPackage);
        }
        if raw_args == ["--list-self-review-packages"] {
            return Ok(Self::ListSelfReviewPackages);
        }
        if raw_args == ["--operator-console"] {
            return Ok(Self::OperatorConsole);
        }
        if raw_args == ["--capability-policy"] {
            return Ok(Self::CapabilityPolicy);
        }
        if raw_args == ["--trust-decision"] {
            return Ok(Self::TrustDecision);
        }
        if raw_args == ["--workspace-snapshot"] {
            return Ok(Self::WorkspaceSnapshot);
        }
        if raw_args == ["--last-workspace-snapshot"] {
            return Ok(Self::LastWorkspaceSnapshot);
        }
        if raw_args == ["--list-workspace-snapshots"] {
            return Ok(Self::ListWorkspaceSnapshots);
        }
        if raw_args == ["--evidence-bundle"] {
            return Ok(Self::EvidenceBundle);
        }
        if raw_args == ["--last-evidence-bundle"] {
            return Ok(Self::LastEvidenceBundle);
        }
        if raw_args == ["--list-evidence-bundles"] {
            return Ok(Self::ListEvidenceBundles);
        }
        if raw_args == ["--recovery-manifest"] {
            return Ok(Self::RecoveryManifest);
        }
        if raw_args == ["--last-recovery-manifest"] {
            return Ok(Self::LastRecoveryManifest);
        }
        if raw_args == ["--list-recovery-manifests"] {
            return Ok(Self::ListRecoveryManifests);
        }
        if raw_args == ["--preflight-gate-v3"] {
            return Ok(Self::PreflightGateV3);
        }
        if raw_args == ["--trust-proof-report"] {
            return Ok(Self::TrustProofReport);
        }
        if raw_args == ["--runtime-candidate"] {
            return Ok(Self::RuntimeCandidate);
        }
        if raw_args == ["--runtime-validation"] {
            return Ok(Self::RuntimeValidation);
        }
        if raw_args == ["--runtime-service"] {
            return Ok(Self::RuntimeService);
        }
        if raw_args == ["--runtime-cli-contract"] {
            return Ok(Self::RuntimeCliContract);
        }
        if raw_args == ["--final-rc-report"] {
            return Ok(Self::FinalRcReport);
        }
        if raw_args == ["--release-proposal"] {
            return Ok(Self::ReleaseProposal);
        }
        if raw_args == ["--release-proposal-json"] {
            return Ok(Self::ReleaseProposalJson);
        }
        if raw_args.len() == 2 && raw_args[0] == "--release-preflight" {
            return Ok(Self::ReleasePreflight(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--release-bundle" {
            return Ok(Self::ReleaseBundle(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--release-approve" {
            return Ok(Self::ReleaseApprove(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--release-manifest" {
            return Ok(Self::ReleaseManifest(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--release-changelog" {
            return Ok(Self::ReleaseChangelog(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--rollback-manifest" {
            return Ok(Self::RollbackManifest(raw_args[1].clone()));
        }
        if raw_args == ["--proof-snapshot"] {
            return Ok(Self::ProofSnapshot);
        }
        if raw_args == ["--proof-snapshot-json"] {
            return Ok(Self::ProofSnapshotJson);
        }
        if raw_args.len() == 2 && raw_args[0] == "--campaign-report" {
            return Ok(Self::CampaignReport(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--adjust-task-from-campaign" {
            return Ok(Self::AdjustTaskFromCampaign(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--campaign-recombine-preview" {
            return Ok(Self::CampaignRecombinePreview(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--bounded-run-report" {
            return Ok(Self::BoundedRunReport(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--candidate-lifecycle" {
            return Ok(Self::CandidateLifecycle(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--supervised-run-report" {
            return Ok(Self::SupervisedRunReport(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--approval-status" {
            return Ok(Self::ApprovalStatus(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--promote-approved" {
            return Ok(Self::PromoteApproved(raw_args[1].clone()));
        }
        if raw_args.len() == 4 && raw_args[0] == "--approve-candidate" && raw_args[2] == "--reason"
        {
            return Ok(Self::ApproveCandidate {
                run_id: raw_args[1].clone(),
                reason: raw_args[3].clone(),
            });
        }
        if raw_args.len() == 4 && raw_args[0] == "--reject-candidate" && raw_args[2] == "--reason" {
            return Ok(Self::RejectCandidate {
                run_id: raw_args[1].clone(),
                reason: raw_args[3].clone(),
            });
        }
        if raw_args.len() == 4 && raw_args[0] == "--defer-candidate" && raw_args[2] == "--reason" {
            return Ok(Self::DeferCandidate {
                run_id: raw_args[1].clone(),
                reason: raw_args[3].clone(),
            });
        }
        if raw_args.len() == 3
            && raw_args[0] == "--supervise-task"
            && raw_args[2].starts_with("--") == false
        {
            return Ok(Self::SuperviseTask {
                task_path: raw_args[1].clone(),
                max_rounds: 3,
            });
        }
        if raw_args.len() == 4 && raw_args[0] == "--supervise-task" && raw_args[2] == "--max-rounds"
        {
            return Ok(Self::SuperviseTask {
                task_path: raw_args[1].clone(),
                max_rounds: raw_args[3]
                    .parse::<usize>()
                    .map_err(|_| "--supervise-task requires integer --max-rounds".to_string())?,
            });
        }
        if raw_args.len() == 5
            && raw_args[0] == "--evolve-bounded"
            && raw_args[1] == "--task"
            && raw_args[3] == "--cycles"
        {
            return Ok(Self::EvolveBounded {
                task_path: raw_args[2].clone(),
                cycles: raw_args[4]
                    .parse::<usize>()
                    .map_err(|_| "--evolve-bounded requires integer cycles".to_string())?,
            });
        }
        if raw_args == ["--distill-patterns"] {
            return Ok(Self::DistillPatterns);
        }
        if raw_args == ["--recombine-patterns"] {
            return Ok(Self::RecombinePatterns);
        }
        if raw_args == ["--evolve-recombined"] {
            return Ok(Self::EvolveRecombined);
        }
        if raw_args.len() == 2 && raw_args[0] == "--report" {
            return Ok(Self::Report(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--report-refresh" {
            return Ok(Self::ReportRefresh(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--review-candidate" {
            return Ok(Self::ReviewCandidate(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--candidate-diff" {
            return Ok(Self::CandidateDiff(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--quality-report" {
            return Ok(Self::QualityReport(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--ingest-corpus" {
            return Ok(Self::IngestCorpus(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--ingest-corpus-contract" {
            return Ok(Self::IngestCorpusContract(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--corpus-summary" {
            return Ok(Self::CorpusSummary(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--suggest-strategy-tasks" {
            return Ok(Self::SuggestStrategyTasks(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--evolve-planned-n" {
            return Ok(Self::EvolvePlannedN(raw_args[1].parse::<usize>().map_err(
                |_| "--evolve-planned-n requires a positive integer".to_string(),
            )?));
        }
        if raw_args.len() == 2 && raw_args[0] == "--evolution-benchmark" {
            return Ok(Self::EvolutionBenchmark(
                raw_args[1]
                    .parse::<usize>()
                    .map_err(|_| "--evolution-benchmark requires a positive integer".to_string())?,
            ));
        }
        if raw_args.len() == 2 && raw_args[0] == "--replay" {
            return Ok(Self::Replay(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--promote" {
            return Ok(Self::Promote(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--ingest-repo" {
            return Ok(Self::IngestRepo(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--run-task" {
            return Ok(Self::RunTask(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--campaign" {
            return Ok(Self::Campaign(raw_args[1].clone()));
        }

        let config_path = parse_config_path(&raw_args)?;
        let file_config = load_optional_runtime_config(config_path.as_deref())?;

        let mut serve = false;
        let mut once = false;
        let mut evolve = false;
        let mut listen_addr = file_config
            .as_ref()
            .map(|config| config.listen_addr.clone())
            .or_else(|| std::env::var("EVA_LISTEN_ADDR").ok())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_LISTEN_ADDR.to_string());
        let mut configured_default_model_id = file_config
            .as_ref()
            .map(|config| config.default_model_id.clone())
            .filter(|value| !value.trim().is_empty());
        let configured_models = file_config
            .as_ref()
            .map(|config| config.models.clone())
            .unwrap_or_default();
        let mut managed_servers = file_config
            .as_ref()
            .map(|config| config.managed_servers.clone())
            .unwrap_or_default();
        let env_model_endpoint = std::env::var("EVA_MODEL_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let env_model_name = std::env::var("EVA_MODEL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let mut model_endpoint = env_model_endpoint
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL_URL.to_string());
        let mut model_name = env_model_name
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL_NAME.to_string());
        let api_key = std::env::var("EVA_MODEL_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let base_model_file = std::env::var("EVA_MODEL_FILE")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let mut model_specs = std::env::var("EVA_MODEL_ENDPOINTS")
            .ok()
            .map(|value| {
                value
                    .split(';')
                    .filter(|entry| !entry.trim().is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut model_file_specs = std::env::var("EVA_MODEL_FILES")
            .ok()
            .map(|value| {
                value
                    .split(';')
                    .filter(|entry| !entry.trim().is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut server_specs = std::env::var("EVA_MODEL_SERVER_COMMANDS")
            .ok()
            .map(|value| {
                value
                    .split(';')
                    .filter(|entry| !entry.trim().is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut daemon_flag_used = false;
        let mut base_model_overridden = env_model_endpoint.is_some() || env_model_name.is_some();
        let mut args = raw_args.into_iter().peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => return Ok(Self::Help),
                "--once" => once = true,
                "--evolve" => evolve = true,
                "--serve" => serve = true,
                "--config" => {
                    daemon_flag_used = true;
                    let _ = args
                        .next()
                        .ok_or_else(|| "--config requires a file path".to_string())?;
                }
                "--listen" => {
                    daemon_flag_used = true;
                    listen_addr = args
                        .next()
                        .ok_or_else(|| "--listen requires an address".to_string())?;
                }
                "--model-url" => {
                    daemon_flag_used = true;
                    base_model_overridden = true;
                    model_endpoint = args
                        .next()
                        .ok_or_else(|| "--model-url requires a URL".to_string())?;
                }
                "--model" => {
                    daemon_flag_used = true;
                    base_model_overridden = true;
                    model_name = args
                        .next()
                        .ok_or_else(|| "--model requires a model name".to_string())?;
                }
                "--model-endpoint" => {
                    daemon_flag_used = true;
                    model_specs.push(
                        args.next()
                            .ok_or_else(|| "--model-endpoint requires ID=MODEL@URL".to_string())?,
                    );
                }
                "--model-file" => {
                    daemon_flag_used = true;
                    model_file_specs.push(
                        args.next()
                            .ok_or_else(|| "--model-file requires ID=PATH".to_string())?,
                    );
                }
                "--start-server" => {
                    daemon_flag_used = true;
                    server_specs.push(
                        args.next()
                            .ok_or_else(|| "--start-server requires ID=COMMAND".to_string())?,
                    );
                }
                value if value.starts_with("--config=") => {
                    daemon_flag_used = true;
                }
                value if value.starts_with("--listen=") => {
                    daemon_flag_used = true;
                    listen_addr = value.trim_start_matches("--listen=").to_string();
                }
                value if value.starts_with("--model-url=") => {
                    daemon_flag_used = true;
                    base_model_overridden = true;
                    model_endpoint = value.trim_start_matches("--model-url=").to_string();
                }
                value if value.starts_with("--model=") => {
                    daemon_flag_used = true;
                    base_model_overridden = true;
                    model_name = value.trim_start_matches("--model=").to_string();
                }
                value if value.starts_with("--model-endpoint=") => {
                    daemon_flag_used = true;
                    model_specs.push(value.trim_start_matches("--model-endpoint=").to_string());
                }
                value if value.starts_with("--model-file=") => {
                    daemon_flag_used = true;
                    model_file_specs.push(value.trim_start_matches("--model-file=").to_string());
                }
                value if value.starts_with("--start-server=") => {
                    daemon_flag_used = true;
                    server_specs.push(value.trim_start_matches("--start-server=").to_string());
                }
                unknown => return Err(format!("unsupported runtime argument: {unknown}")),
            }
        }

        if once && serve {
            return Err("--once cannot be used with --serve".to_string());
        }
        if evolve && serve {
            return Err("--evolve cannot be used with --serve".to_string());
        }
        if once && evolve {
            return Err("--once cannot be used with --evolve".to_string());
        }

        if !serve {
            if daemon_flag_used {
                return Err("model daemon flags require --serve".to_string());
            }
            return Ok(if once {
                Self::Once
            } else if evolve {
                Self::Evolve
            } else {
                Self::Help
            });
        }

        if listen_addr.trim().is_empty() {
            return Err("--listen must not be empty".to_string());
        }
        if model_endpoint.trim().is_empty() {
            return Err("--model-url must not be empty".to_string());
        }
        if model_name.trim().is_empty() {
            return Err("--model must not be empty".to_string());
        }

        let base_model = OpenAiModelConfig {
            id: "default".to_string(),
            endpoint: model_endpoint,
            model: model_name,
            local_model_path: base_model_file,
            api_key: api_key.clone(),
            timeout_secs: 30,
        };
        let mut models = if !configured_models.is_empty() {
            configured_models
        } else if model_specs.is_empty() && !base_model_overridden {
            vec![OpenAiModelConfig::default()]
        } else if base_model_overridden {
            vec![base_model.clone()]
        } else {
            Vec::new()
        };
        if base_model_overridden && !models.iter().any(|model| model.id == "default") {
            models.insert(0, base_model);
        }
        for spec in model_specs {
            models.push(parse_model_endpoint_spec(&spec, api_key.clone())?);
        }
        apply_model_file_specs(&mut models, model_file_specs)?;
        if models.iter().all(|model| model.id != DEFAULT_MODEL_ID) {
            models.push(OpenAiModelConfig::default());
        }
        ensure_unique_model_ids(&models)?;
        let fallback_default_model_id = models
            .first()
            .map(|model| model.id.clone())
            .ok_or_else(|| "at least one model endpoint is required".to_string())?;
        for spec in server_specs {
            managed_servers.push(parse_managed_server_spec(&spec)?);
        }
        let default_model_id = configured_default_model_id
            .take()
            .filter(|model_id| models.iter().any(|model| model.id == *model_id))
            .unwrap_or(fallback_default_model_id);

        Ok(Self::Serve(RuntimeDaemonConfig {
            listen_addr,
            default_model_id,
            models,
            managed_servers,
        }))
    }
}

impl RuntimeDaemonConfig {
    pub fn default_model(&self) -> Result<&OpenAiModelConfig, String> {
        self.model_by_id(&self.default_model_id)
    }

    pub fn model_by_id(&self, model_id: &str) -> Result<&OpenAiModelConfig, String> {
        self.models
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| format!("unknown model endpoint id: {model_id}"))
    }

    pub fn selected_model(&self, model_id: Option<&str>) -> Result<&OpenAiModelConfig, String> {
        match model_id {
            Some(value) if !value.trim().is_empty() => self.model_by_id(value),
            _ => self.default_model(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedServerConfig {
    pub id: String,
    pub command: String,
}

pub fn parse_model_endpoint_spec(
    spec: &str,
    api_key: Option<String>,
) -> Result<OpenAiModelConfig, String> {
    let (id, rest) = spec
        .split_once('=')
        .ok_or_else(|| "model endpoint must use ID=MODEL@URL".to_string())?;
    let (model, endpoint) = rest
        .split_once('@')
        .ok_or_else(|| "model endpoint must use ID=MODEL@URL".to_string())?;
    if id.trim().is_empty() || model.trim().is_empty() || endpoint.trim().is_empty() {
        return Err("model endpoint ID, MODEL, and URL must not be empty".to_string());
    }
    Ok(OpenAiModelConfig {
        id: id.trim().to_string(),
        endpoint: endpoint.trim().to_string(),
        model: model.trim().to_string(),
        local_model_path: None,
        api_key,
        timeout_secs: 30,
    })
}

pub fn apply_model_file_specs(
    models: &mut [OpenAiModelConfig],
    specs: Vec<String>,
) -> Result<(), String> {
    let model_ids = models
        .iter()
        .map(|model| model.id.clone())
        .collect::<std::collections::HashSet<_>>();
    for spec in specs {
        let (id, path) = spec
            .split_once('=')
            .ok_or_else(|| "model file must use ID=PATH".to_string())?;
        let id = id.trim();
        let path = path.trim();
        if id.is_empty() || path.is_empty() {
            return Err("model file ID and PATH must not be empty".to_string());
        }
        if !model_ids.contains(id) {
            return Err(format!(
                "model file references unknown model endpoint id: {id}"
            ));
        }
        if let Some(model) = models.iter_mut().find(|model| model.id == id) {
            model.local_model_path = Some(path.to_string());
        }
    }
    Ok(())
}

pub fn parse_managed_server_spec(spec: &str) -> Result<ManagedServerConfig, String> {
    let (id, command) = spec
        .split_once('=')
        .ok_or_else(|| "managed server must use ID=COMMAND".to_string())?;
    if id.trim().is_empty() || command.trim().is_empty() {
        return Err("managed server ID and COMMAND must not be empty".to_string());
    }
    Ok(ManagedServerConfig {
        id: id.trim().to_string(),
        command: command.trim().to_string(),
    })
}

struct ManagedServerChild {
    id: String,
    child: Child,
}

impl Drop for ManagedServerChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_managed_servers(
    configs: &[ManagedServerConfig],
) -> Result<Vec<ManagedServerChild>, String> {
    configs
        .iter()
        .map(|config| {
            let parts = config.command.split_whitespace().collect::<Vec<_>>();
            let Some(program) = parts.first() else {
                return Err(format!("managed server {} has an empty command", config.id));
            };
            let child = Command::new(program)
                .args(&parts[1..])
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|error| {
                    format!(
                        "failed to start managed server {} with '{}': {}",
                        config.id, config.command, error
                    )
                })?;
            Ok(ManagedServerChild {
                id: config.id.clone(),
                child,
            })
        })
        .collect()
}

fn ensure_unique_model_ids(models: &[OpenAiModelConfig]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for model in models {
        if !seen.insert(model.id.clone()) {
            return Err(format!("duplicate model endpoint id: {}", model.id));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCycleHttpRequest {
    pub goal: String,
    pub context: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelChatHttpRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default)]
    pub messages: Vec<ModelChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeModelAdvisory {
    pub status: String,
    pub model_id: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCycleHttpResponse {
    pub project_report_ru: crate::ProjectPhaseReport,
    pub runtime_audit: crate::RuntimeAudit,
    pub model_advisory: RuntimeModelAdvisory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonHealthResponse {
    pub daemon_status: String,
    pub listen_addr: String,
    pub default_model_id: String,
    pub managed_servers: Vec<ManagedServerConfig>,
    pub backends: Vec<ModelBackendHealth>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRegistryResponse {
    pub default_model_id: String,
    pub models: Vec<OpenAiModelConfig>,
    pub managed_servers: Vec<ManagedServerConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelBackendHealth {
    pub id: String,
    pub endpoint: String,
    pub model: String,
    pub backend: crate::ModelHealth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status_code: u16,
    pub reason: &'static str,
    pub body: String,
}

pub fn serve(config: RuntimeDaemonConfig) -> Result<(), String> {
    let managed_children = start_managed_servers(&config.managed_servers)?;
    let listener = TcpListener::bind(&config.listen_addr)
        .map_err(|error| format!("failed to bind {}: {}", config.listen_addr, error))?;
    println!(
        "eva_runtime_daemon listening on {} with {} model endpoint(s), {} managed server(s); default={}",
        config.listen_addr,
        config.models.len(),
        managed_children.len(),
        config.default_model_id
    );
    for child in &managed_children {
        println!("managed model server started: {}", child.id);
    }

    let _managed_children = managed_children;
    let config = Arc::new(config);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = Arc::clone(&config);
                thread::spawn(move || {
                    let mut stream = stream;
                    if let Err(error) = handle_stream(&mut stream, &config) {
                        eprintln!("daemon_request_error: {error}");
                    }
                });
            }
            Err(error) => eprintln!("daemon_accept_error: {error}"),
        }
    }

    Ok(())
}

pub fn handle_http_request(
    method: &str,
    path: &str,
    body: &str,
    config: &RuntimeDaemonConfig,
) -> HttpResponse {
    match (method, path) {
        ("GET", "/health") => json_response(
            200,
            "OK",
            &build_health_response(config).unwrap_or_else(|error| DaemonHealthResponse {
                daemon_status: "degraded".to_string(),
                listen_addr: config.listen_addr.clone(),
                default_model_id: config.default_model_id.clone(),
                managed_servers: config.managed_servers.clone(),
                backends: vec![ModelBackendHealth {
                    id: config.default_model_id.clone(),
                    endpoint: String::new(),
                    model: String::new(),
                    backend: crate::ModelHealth {
                        reachable: false,
                        status: None,
                        models: Vec::new(),
                        error: Some(error),
                    },
                }],
            }),
        ),
        ("GET", "/models") => json_response(
            200,
            "OK",
            &ModelRegistryResponse {
                default_model_id: config.default_model_id.clone(),
                models: config.models.clone(),
                managed_servers: config.managed_servers.clone(),
            },
        ),
        ("POST", "/runtime/cycle") => match serde_json::from_str::<RuntimeCycleHttpRequest>(body) {
            Ok(request) => match run_runtime_cycle_http(request, config) {
                Ok(response) => json_response(200, "OK", &response),
                Err(error) => error_response(500, "Internal Server Error", error),
            },
            Err(error) => error_response(
                400,
                "Bad Request",
                format!("invalid runtime request: {error}"),
            ),
        },
        ("POST", "/model/chat") => match serde_json::from_str::<ModelChatHttpRequest>(body) {
            Ok(request) => match run_model_chat_http(request, config) {
                Ok(response) => json_response(200, "OK", &response),
                Err(error) => error_response(500, "Internal Server Error", error),
            },
            Err(error) => error_response(
                400,
                "Bad Request",
                format!("invalid model request: {error}"),
            ),
        },
        _ => error_response(404, "Not Found", "route not found".to_string()),
    }
}

fn build_health_response(config: &RuntimeDaemonConfig) -> Result<DaemonHealthResponse, String> {
    let backends = config
        .models
        .iter()
        .map(|model| {
            let backend = OpenAiModelClient::new(model.clone())
                .map(|client| client.health())
                .unwrap_or_else(|error| crate::ModelHealth {
                    reachable: false,
                    status: None,
                    models: Vec::new(),
                    error: Some(error),
                });
            ModelBackendHealth {
                id: model.id.clone(),
                endpoint: model.endpoint.clone(),
                model: model.model.clone(),
                backend,
            }
        })
        .collect::<Vec<_>>();
    let any_reachable = backends.iter().any(|entry| entry.backend.reachable);
    Ok(DaemonHealthResponse {
        daemon_status: if any_reachable {
            "ok".to_string()
        } else {
            "degraded".to_string()
        },
        listen_addr: config.listen_addr.clone(),
        default_model_id: config.default_model_id.clone(),
        managed_servers: config.managed_servers.clone(),
        backends,
    })
}

fn run_runtime_cycle_http(
    request: RuntimeCycleHttpRequest,
    config: &RuntimeDaemonConfig,
) -> Result<RuntimeCycleHttpResponse, String> {
    let mut runner = RuntimeCycleRunner::new();
    let report = runner.run_cycle_report(CycleInput {
        goal: request.goal.clone(),
        external_state: request.context.clone(),
    })?;
    let output = build_project_phase_runtime_output(&report);
    let prompt = format!(
        "Goal: {}\nContext: {}\nRuntime audit JSON: {}\nReturn a concise operational recommendation.",
        request.goal,
        request.context,
        serde_json::to_string(&output.runtime_audit).expect("serialize audit")
    );
    let advisory = model_advisory(
        config,
        vec![
            ModelChatMessage::system(
                "You are a local runtime advisor. Be concrete, concise, and do not claim file mutations.",
            ),
            ModelChatMessage::user(prompt),
        ],
        request.model_id.as_deref(),
        ModelChatOptions::default(),
    )?;

    Ok(RuntimeCycleHttpResponse {
        project_report_ru: output.project_report_ru,
        runtime_audit: output.runtime_audit,
        model_advisory: advisory,
    })
}

fn run_model_chat_http(
    request: ModelChatHttpRequest,
    config: &RuntimeDaemonConfig,
) -> Result<RuntimeModelAdvisory, String> {
    let messages = if request.messages.is_empty() {
        vec![ModelChatMessage::user(
            request
                .prompt
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "model request requires prompt or messages".to_string())?,
        )]
    } else {
        request.messages
    };
    let options = ModelChatOptions {
        temperature: request.temperature.unwrap_or(0.2),
        max_tokens: request.max_tokens.unwrap_or(512),
    };

    model_advisory(config, messages, request.model_id.as_deref(), options)
}

fn model_advisory(
    config: &RuntimeDaemonConfig,
    messages: Vec<ModelChatMessage>,
    model_id: Option<&str>,
    options: ModelChatOptions,
) -> Result<RuntimeModelAdvisory, String> {
    let selected = config.selected_model(model_id)?.clone();
    let client = match OpenAiModelClient::new(selected.clone()) {
        Ok(client) => client,
        Err(error) => {
            return Ok(RuntimeModelAdvisory {
                status: "error".to_string(),
                model_id: selected.id,
                model: selected.model,
                content: None,
                error: Some(error),
            });
        }
    };
    Ok(match client.chat(messages, options) {
        Ok(output) => RuntimeModelAdvisory {
            status: "ok".to_string(),
            model_id: client.config().id.clone(),
            model: client.config().model.clone(),
            content: Some(output.content),
            error: None,
        },
        Err(error) => RuntimeModelAdvisory {
            status: "error".to_string(),
            model_id: client.config().id.clone(),
            model: client.config().model.clone(),
            content: None,
            error: Some(error),
        },
    })
}

fn handle_stream(stream: &mut TcpStream, config: &RuntimeDaemonConfig) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("failed to set read timeout: {error}"))?;
    let request = read_http_request(stream)?;
    let response = handle_http_request(&request.method, &request.path, &request.body, config);
    write_http_response(stream, response)
}

struct HttpRequest {
    method: String,
    path: String,
    body: String,
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let mut header_end = None;
    let mut content_length = 0_usize;

    loop {
        let bytes_read = stream
            .read(&mut chunk)
            .map_err(|error| format!("failed to read request: {error}"))?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if buffer.len() > 1_048_576 {
            return Err("request exceeds 1 MiB limit".to_string());
        }
        if header_end.is_none() {
            if let Some(index) = find_header_end(&buffer) {
                header_end = Some(index);
                let headers = String::from_utf8_lossy(&buffer[..index]).to_string();
                content_length = parse_headers(&headers)
                    .remove("content-length")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
            }
        }
        if let Some(index) = header_end {
            if buffer.len() >= index + 4 + content_length {
                break;
            }
        }
    }

    let header_end = header_end.ok_or_else(|| "request missing HTTP headers".to_string())?;
    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "request missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "request missing method".to_string())?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| "request missing path".to_string())?
        .to_string();
    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    let body =
        String::from_utf8_lossy(buffer.get(body_start..body_end).unwrap_or_default()).to_string();

    Ok(HttpRequest { method, path, body })
}

fn write_http_response(stream: &mut TcpStream, response: HttpResponse) -> Result<(), String> {
    let bytes = response.body.as_bytes();
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status_code,
        response.reason,
        bytes.len()
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|_| stream.write_all(bytes))
        .map_err(|error| format!("failed to write response: {error}"))
}

fn json_response<T: Serialize>(status_code: u16, reason: &'static str, value: &T) -> HttpResponse {
    HttpResponse {
        status_code,
        reason,
        body: serde_json::to_string_pretty(value).expect("serialize HTTP response"),
    }
}

fn error_response(status_code: u16, reason: &'static str, error: String) -> HttpResponse {
    json_response(status_code, reason, &serde_json::json!({ "error": error }))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_headers(headers: &str) -> HashMap<String, String> {
    headers
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static RUNTIME_TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_runtime_test_root(name: &str) -> std::path::PathBuf {
        let sanitized = name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let counter = RUNTIME_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".eva-runtime-tests")
            .join(format!(
                "eva-runtime-{}-{}-{}-{}",
                if sanitized.is_empty() {
                    "test"
                } else {
                    &sanitized
                },
                std::process::id(),
                nanos,
                counter
            ));
        std::fs::create_dir_all(&root).expect("create runtime test root");
        root
    }

    #[test]
    fn runtime_cli_defaults_to_help() {
        assert_eq!(
            RuntimeCliCommand::parse_from_iter(Vec::<String>::new()).unwrap(),
            RuntimeCliCommand::Help
        );
        assert_eq!(
            RuntimeCliCommand::parse_from_iter(["--help"]).unwrap(),
            RuntimeCliCommand::Help
        );
    }

    #[test]
    fn runtime_cli_parses_once() {
        assert_eq!(
            RuntimeCliCommand::parse_from_iter(["--once"]).unwrap(),
            RuntimeCliCommand::Once
        );
    }

    #[test]
    fn runtime_cli_parses_evolve() {
        assert_eq!(
            RuntimeCliCommand::parse_from_iter(["--evolve"]).unwrap(),
            RuntimeCliCommand::Evolve
        );
    }

    #[test]
    fn runtime_cli_parses_serve_config() {
        let command = RuntimeCliCommand::parse_from_iter([
            "--serve",
            "--listen",
            "127.0.0.1:9999",
            "--model-url",
            "http://127.0.0.1:1234/v1/chat/completions",
            "--model",
            "demo",
        ])
        .expect("parse serve");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(config.listen_addr, "127.0.0.1:9999");
                assert_eq!(config.default_model().unwrap().model, "demo");
                assert!(config.model_by_id(DEFAULT_MODEL_ID).is_ok());
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn daemon_flags_require_serve() {
        let error =
            RuntimeCliCommand::parse_from_iter(["--model", "demo"]).expect_err("missing serve");

        assert!(error.contains("--serve"));
    }

    #[test]
    fn unknown_route_returns_404() {
        let config = RuntimeDaemonConfig::default();
        let response = handle_http_request("GET", "/missing", "", &config);

        assert_eq!(response.status_code, 404);
    }

    #[test]
    fn runtime_cli_parses_multiple_model_endpoints() {
        let command = RuntimeCliCommand::parse_from_iter([
            "--serve",
            "--model-endpoint",
            "fast=tiny@http://127.0.0.1:1234/v1/chat/completions",
            "--model-endpoint",
            "deep=larger@http://127.0.0.1:8080/v1/chat/completions",
        ])
        .expect("parse multi model");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(config.default_model_id, "fast");
                assert_eq!(config.models.len(), 3);
                assert_eq!(config.model_by_id("deep").unwrap().model, "larger");
                assert_eq!(
                    config.model_by_id(DEFAULT_MODEL_ID).unwrap().model,
                    "eva-lite"
                );
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn runtime_cli_attaches_local_model_file_guard() {
        let command = RuntimeCliCommand::parse_from_iter([
            "--serve",
            "--model-endpoint",
            "fast=tiny@http://127.0.0.1:1234/v1/chat/completions",
            "--model-file",
            "fast=/models/tiny.gguf",
        ])
        .expect("parse guarded model");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(
                    config
                        .model_by_id("fast")
                        .unwrap()
                        .local_model_path
                        .as_deref(),
                    Some("/models/tiny.gguf")
                );
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn guarded_external_model_reports_error_without_network() {
        let command = RuntimeCliCommand::parse_from_iter([
            "--serve",
            "--model-endpoint",
            "fast=tiny@http://127.0.0.1:1234/v1/chat/completions",
            "--model-file",
            "fast=/tmp/eva_missing_model.gguf",
        ])
        .expect("parse guarded model");
        let RuntimeCliCommand::Serve(config) = command else {
            panic!("expected serve command");
        };

        let response = handle_http_request(
            "POST",
            "/model/chat",
            r#"{"prompt":"check","model_id":"fast"}"#,
            &config,
        );
        let advisory =
            serde_json::from_str::<RuntimeModelAdvisory>(&response.body).expect("advisory json");

        assert_eq!(response.status_code, 200);
        assert_eq!(advisory.status, "error");
        assert!(advisory.error.unwrap().contains("local model file"));
    }

    #[test]
    fn runtime_cli_plain_serve_uses_local_runtime_config_when_present() {
        let command = RuntimeCliCommand::parse_from_iter(["--serve"]).expect("parse serve");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(config.default_model_id, "qwen3-local");
                assert_eq!(config.default_model().unwrap().model, "qwen3:1.7b");
                assert!(config.model_by_id(DEFAULT_MODEL_ID).is_ok());
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn runtime_cli_parses_managed_server_command() {
        let command = RuntimeCliCommand::parse_from_iter([
            "--serve",
            "--start-server",
            "lm=python3 /tmp/openai_mock.py",
        ])
        .expect("parse managed server");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(config.managed_servers.len(), 1);
                assert_eq!(config.managed_servers[0].id, "lm");
                assert_eq!(
                    config.managed_servers[0].command,
                    "python3 /tmp/openai_mock.py"
                );
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn runtime_cli_loads_json_config() {
        let root = unique_runtime_test_root("runtime_cli_loads_json_config");
        let path = root.join("eva_runtime_config_test_config.json");
        std::fs::write(
            &path,
            r#"{
  "listen_addr": "127.0.0.1:9998",
  "default_model_id": "eva-lite",
  "models": [
    {
      "id": "eva-lite",
      "endpoint": "builtin://eva-lite",
      "model": "eva-lite",
      "timeout_secs": 30
    }
  ],
  "managed_servers": []
}"#,
        )
        .expect("write config");

        let command = RuntimeCliCommand::parse_from_iter([
            "--serve".to_string(),
            "--config".to_string(),
            path.display().to_string(),
        ])
        .expect("parse config");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(config.listen_addr, "127.0.0.1:9998");
                assert_eq!(config.default_model_id, DEFAULT_MODEL_ID);
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }

        let _ = std::fs::remove_dir_all(root);
    }
}
