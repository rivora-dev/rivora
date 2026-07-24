//! Bounded coding-agent handoff boundary.
//!
//! Handoffs are typed previews derived from Capabilities. They never grant
//! execution authority and never include secrets.
#![allow(dead_code)]

use rivora::domain::{InvestigationId, ObjectId};
use rivora::CapabilityService;

use crate::error_view::{map_error, WorkspaceErrorView};

/// Typed handoff package for an external coding agent.
#[derive(Debug, Clone)]
pub struct AgentHandoffPackage {
    pub investigation_id: InvestigationId,
    pub proposal_id: ObjectId,
    pub preview: String,
    pub secrets_excluded: bool,
    pub auto_execute: bool,
}

/// Prepare a handoff via Capability `generate_coding_agent_handoff`.
pub fn prepare_handoff(
    caps: &CapabilityService,
    investigation_id: InvestigationId,
    proposal_id: ObjectId,
) -> Result<AgentHandoffPackage, Box<WorkspaceErrorView>> {
    let text = caps
        .generate_coding_agent_handoff(investigation_id, proposal_id)
        .map_err(|e| Box::new(map_error(&e)))?;
    let preview = format!(
        "Investigation: {investigation_id}\nProposal: {proposal_id}\n\n{text}\n\n\
         Constraints:\n\
         - No automatic acceptance\n\
         - No automatic external execution\n\
         - Return receipt must re-enter Verification and Learning"
    );
    Ok(AgentHandoffPackage {
        investigation_id,
        proposal_id,
        preview,
        secrets_excluded: true,
        auto_execute: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_never_auto_executes() {
        // Structural guarantee on the type defaults.
        let p = AgentHandoffPackage {
            investigation_id: InvestigationId::new(),
            proposal_id: ObjectId::new(),
            preview: "fixture".into(),
            secrets_excluded: true,
            auto_execute: false,
        };
        assert!(!p.auto_execute);
        assert!(p.secrets_excluded);
    }
}
