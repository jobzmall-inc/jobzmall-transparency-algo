use super::claim::{Claim, ClaimHead};
use super::judge::Judgement;
use jm_common::{Disposition, GhostLevel, SourceLiveness, WarrantNode};

/// Build a public proof tree from a judgement: disposition → head →
/// surviving claim → fact. Defeated lines stay visible.
pub fn build_warrant(j: &Judgement) -> WarrantNode {
    let mut premises = vec![
        head_node(
            format!(
                "source={} conf={:.2}",
                j.source.kind.as_label(),
                j.source.confidence
            ),
            "SourceAgreement",
            j.source.confidence,
            j,
            ClaimHead::Source,
        ),
        head_node(
            format!(
                "ghost={} ({})",
                format_ghost(j.ghost.level),
                j.ghost.fired_of_total()
            ),
            "GhostTiers",
            ghost_force(j.ghost.level),
            j,
            ClaimHead::Ghost,
        ),
        head_node(
            format!("liveness={}", j.liveness.as_str()),
            "HttpAndSync",
            liveness_force(j.liveness),
            j,
            ClaimHead::Liveness,
        ),
        head_node(
            format!(
                "fraud max={:.2} noisy_or={:.2}",
                j.fraud.max(),
                j.fraud.combined
            ),
            "LogOddsHeads",
            j.fraud.combined,
            j,
            ClaimHead::Fraud,
        ),
    ];
    premises.extend(claim_nodes(j, ClaimHead::Drop));

    WarrantNode {
        conclusion: format!("{:?} by {}", j.obligation.disposition, j.obligation.by),
        rule: "LatticeJoin".into(),
        force: match j.obligation.disposition {
            Disposition::Show => 0.2,
            Disposition::Disclose => 0.6,
            Disposition::Restrict => 0.85,
            Disposition::Drop => 1.0,
        },
        premises,
        defeated: j.defeated_notes.clone(),
    }
}

fn head_node(
    conclusion: String,
    rule: &str,
    force: f64,
    j: &Judgement,
    head: ClaimHead,
) -> WarrantNode {
    WarrantNode {
        conclusion,
        rule: rule.into(),
        force,
        premises: claim_nodes(j, head),
        defeated: Vec::new(),
    }
}

fn claim_nodes(j: &Judgement, head: ClaimHead) -> Vec<WarrantNode> {
    j.surviving_claims
        .iter()
        .filter(|c| c.kind.head() == head)
        .map(claim_node)
        .collect()
}

fn claim_node(c: &Claim) -> WarrantNode {
    WarrantNode {
        conclusion: c.note.clone(),
        rule: format!("{}/{}", c.analyzer, c.tier.as_str()),
        force: c.force,
        premises: c
            .premises
            .iter()
            .map(|f| WarrantNode::leaf(f.0.clone(), "fact", c.force))
            .collect(),
        defeated: Vec::new(),
    }
}

fn format_ghost(level: GhostLevel) -> &'static str {
    match level {
        GhostLevel::None => "none",
        GhostLevel::Possible => "possible",
        GhostLevel::Likely => "likely",
    }
}

fn ghost_force(level: GhostLevel) -> f64 {
    match level {
        GhostLevel::None => 0.0,
        GhostLevel::Possible => 0.55,
        GhostLevel::Likely => 0.85,
    }
}

fn liveness_force(state: SourceLiveness) -> f64 {
    match state {
        SourceLiveness::LiveAtSource => 0.9,
        SourceLiveness::ClosedAtSource => 0.95,
        SourceLiveness::SourceNotFound | SourceLiveness::StatusUncertain => 0.4,
    }
}
