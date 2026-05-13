use crate::agent::propose::{load_proposal, save_proposal, validate_patch_proposal};
use crate::agent::storage::{id, memory_path, now_unix, save_json_pretty};
use crate::agent::task::{load_task, update_task};
use crate::contracts::{AgentApproval, AgentTaskStatus, ProposalStatus};

pub fn approve_proposal(memory_root: &str, proposal_id: &str) -> Result<AgentApproval, String> {
    let mut proposal = load_proposal(memory_root, proposal_id)?;
    validate_patch_proposal(&mut proposal);
    if !proposal.blockers.is_empty() || proposal.status == ProposalStatus::Refused {
        return Err(format!(
            "approval refused\nproposal_id={proposal_id}\nreason=proposal_has_blockers\nblockers={}",
            proposal.blockers.join(",")
        ));
    }
    let approval = AgentApproval {
        approval_id: id("approval"),
        proposal_id: proposal_id.into(),
        task_id: proposal.task_id.clone(),
        approved: true,
        approved_at: now_unix(),
        approved_by: "operator".into(),
        reason: "operator approved governed proposal".into(),
        warnings: Vec::new(),
        blockers: Vec::new(),
    };
    proposal.approved = true;
    proposal.approved_at = Some(approval.approved_at);
    proposal.status = ProposalStatus::Approved;
    proposal.updated_at = now_unix();
    save_proposal(memory_root, &proposal)?;
    save_json_pretty(
        &memory_path(
            memory_root,
            &["approvals", &format!("{}.json", approval.approval_id)],
        ),
        &approval,
    )?;
    let mut task = load_task(memory_root, &proposal.task_id)?;
    task.status = AgentTaskStatus::Approved;
    task.approval_id = Some(approval.approval_id.clone());
    update_task(memory_root, task)?;
    Ok(approval)
}

pub fn print_approve_proposal(memory_root: &str, proposal_id: &str) -> Result<String, String> {
    let approval = approve_proposal(memory_root, proposal_id)?;
    Ok(format!(
        "proposal approved\nproposal_id={}\napproval_id={}",
        approval.proposal_id, approval.approval_id
    ))
}
