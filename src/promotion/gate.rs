use serde::{Deserialize, Serialize};

use crate::contracts::MutationContract;
use crate::evolution::memory::{PROMOTION_RISK_LIMIT, PROMOTION_THRESHOLD};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionDecision {
    pub allowed: bool,
    pub reason: String,
}

pub fn check_promotion_gate(mutation: &MutationContract, score: f32) -> PromotionDecision {
    if matches!(mutation.kind, crate::contracts::MutationKind::AppendComment) {
        return reject("append comment mutations are cosmetic and cannot be promoted");
    }
    if score < PROMOTION_THRESHOLD {
        return reject("candidate score below promotion threshold");
    }
    if mutation.risk > PROMOTION_RISK_LIMIT {
        return reject("candidate risk above promotion limit");
    }
    if mutation.target_file.contains("src/core/")
        || mutation.target_file == "src/main.rs"
        || mutation.target_file == "src/lib.rs"
    {
        return reject("core/main/lib promotion is forbidden");
    }
    PromotionDecision {
        allowed: true,
        reason: "promotion gate passed".to_string(),
    }
}

fn reject(reason: &str) -> PromotionDecision {
    PromotionDecision {
        allowed: false,
        reason: reason.to_string(),
    }
}
