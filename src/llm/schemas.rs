use serde_json::{json, Value};

pub const AGENT_PLAN_SCHEMA: &str = "AgentPlan";
pub const PATCH_PROPOSAL_SCHEMA: &str = "PatchProposal";
pub const AGENT_REPORT_SCHEMA: &str = "AgentReport";
pub const PR_SUMMARY_SCHEMA: &str = "PrSummary";

pub fn schema_for(name: &str) -> Value {
    match name {
        AGENT_PLAN_SCHEMA => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["steps", "likely_files", "risk_level", "warnings", "blockers"],
            "properties": {
                "steps": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["title", "detail", "expected_files", "risk"],
                        "properties": {
                            "title": { "type": "string" },
                            "detail": { "type": "string" },
                            "expected_files": {
                                "type": "array",
                                "items": { "type": "string" }
                            },
                            "risk": { "type": "string" }
                        }
                    }
                },
                "likely_files": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "risk_level": { "type": "string" },
                "warnings": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "blockers": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            }
        }),
        PATCH_PROPOSAL_SCHEMA => json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["summary", "files_to_change", "risk_level", "patch_ops"],
            "properties": {
                "summary": { "type": "string" },
                "files_to_change": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "risk_level": { "type": "string" },
                "patch_ops": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["path", "op", "description", "content", "find", "replace"],
                        "properties": {
                            "path": { "type": "string" },
                            "op": {
                                "type": "string",
                                "enum": ["CreateFile", "AppendFile", "ReplaceFileIfExists", "ReplaceExactText"]
                            },
                            "description": { "type": "string" },
                            "content": { "type": ["string", "null"] },
                            "find": { "type": ["string", "null"] },
                            "replace": { "type": ["string", "null"] }
                        }
                    }
                }
            }
        }),
        _ => json!({
            "type": "object",
            "additionalProperties": true
        }),
    }
}
