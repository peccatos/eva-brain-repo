use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    AppendComment,
    ReplaceText,
    ParameterTune,
    AddTestSkeleton,
    AddMetricField,
    AddUnitTest,
    AddReplayAssertion,
    AddLearningSummaryField,
    AddMetricUpdate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationContract {
    pub id: String,
    pub kind: MutationKind,
    pub target_file: String,
    pub search: Option<String>,
    pub replace: Option<String>,
    pub append: Option<String>,
    pub reason: String,
    pub expected_gain: f32,
    pub risk: f32,
}
