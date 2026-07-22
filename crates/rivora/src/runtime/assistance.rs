//! Explainable Engineering Assistance (RFC-019).

use chrono::Utc;

use crate::domain::{
    AssessmentType, Confidence, DeploymentReadiness, EngineeringReport, Hypothesis,
    HypothesisStatus, InvestigationId, InvestigationSummary, KnowledgeKind, ObjectId,
    PrioritizedRecommendation, Provenance, RankingFactor, ReadinessDimension, ReadinessStatus,
    ReportSection, RiskCategory, RiskForecast, RiskItem, RootCauseGuidance, Severity,
    SummaryCounts, VerificationFeasibility, VerificationResult, VerificationSuggestion,
};
use crate::error::RivoraResult;
use crate::runtime::Runtime;

impl Runtime {
    /// Generate ranked hypotheses from current and historical evidence.
    pub fn generate_hypotheses(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<Hypothesis>> {
        let actor = actor.into();
        let _ = self.store.load_investigation(&investigation_id)?;
        let mut knowledge = self.store.list_knowledge(&investigation_id)?;
        if knowledge.is_empty() {
            knowledge = self.derive_knowledge(investigation_id, actor.clone())?;
        }
        let memory = self.store.list_memory(&investigation_id)?;
        let observations = self.store.list_observations(&investigation_id)?;
        let evaluations = self.store.list_evaluations(&investigation_id)?;
        let similar = self
            .find_similar_investigations(investigation_id, Some(3))
            .unwrap_or_default();
        let related_ids: Vec<_> = similar.iter().map(|s| s.investigation_id).collect();

        let risk_knowledge: Vec<_> = knowledge
            .iter()
            .filter(|k| matches!(k.kind, KnowledgeKind::RiskSignal))
            .collect();
        let failure_obs: Vec<_> = observations
            .iter()
            .filter(|o| {
                let t = format!(
                    "{} {}",
                    o.summary.to_lowercase(),
                    o.payload.to_string().to_lowercase()
                );
                t.contains("fail")
                    || t.contains("error")
                    || t.contains("crash")
                    || t.contains("timeout")
                    || matches!(
                        o.kind,
                        crate::domain::ObservationKind::CheckResult
                            | crate::domain::ObservationKind::TestOutput
                            | crate::domain::ObservationKind::WorkflowRun
                            | crate::domain::ObservationKind::Observability
                    ) && (t.contains("fail") || t.contains("error") || t.contains("alert"))
            })
            .collect();

        let provenance = Provenance::now(actor, "runtime").with_capability("generate_hypotheses");

        let mut hypotheses = Vec::new();
        let mut rank = 1u32;

        if !risk_knowledge.is_empty() || !failure_obs.is_empty() {
            let supporting: Vec<ObjectId> = risk_knowledge
                .iter()
                .map(|k| k.id)
                .chain(failure_obs.iter().map(|o| o.id))
                .chain(memory.iter().take(5).map(|m| m.id))
                .collect();
            let contradicting: Vec<ObjectId> = evaluations
                .iter()
                .filter(|e| {
                    e.assessment_type == AssessmentType::Risk
                        && matches!(e.severity, Severity::Low | Severity::Info)
                })
                .map(|e| e.id)
                .collect();
            let conf = if failure_obs.len() >= 2 {
                0.75
            } else if !failure_obs.is_empty() {
                0.6
            } else {
                0.45
            };
            let status = if contradicting.is_empty() {
                HypothesisStatus::Supported
            } else {
                HypothesisStatus::Inconclusive
            };
            hypotheses.push(Hypothesis::new(
                investigation_id,
                "A recent failure signal (CI, test, or operational error) is central to the problem.",
                status,
                Confidence::new(conf),
                supporting,
                contradicting,
                related_ids.clone(),
                "failure_signal_scan_v1",
                "unverified — requires Verification Receipt",
                rank,
                provenance.clone(),
            ));
            rank += 1;
        }

        if observations.iter().any(|o| {
            matches!(
                o.kind,
                crate::domain::ObservationKind::WorkflowRun
                    | crate::domain::ObservationKind::CheckResult
            )
        }) {
            let supporting: Vec<_> = observations
                .iter()
                .filter(|o| {
                    matches!(
                        o.kind,
                        crate::domain::ObservationKind::WorkflowRun
                            | crate::domain::ObservationKind::CheckResult
                    )
                })
                .map(|o| o.id)
                .collect();
            hypotheses.push(Hypothesis::new(
                investigation_id,
                "CI or delivery pipeline state is implicated and should be inspected before promotion.",
                HypothesisStatus::Proposed,
                Confidence::new(0.55),
                supporting,
                Vec::new(),
                related_ids.clone(),
                "ci_observation_presence_v1",
                "unverified",
                rank,
                provenance.clone(),
            ));
            rank += 1;
        }

        if observations.iter().any(|o| {
            matches!(
                o.kind,
                crate::domain::ObservationKind::Infrastructure
                    | crate::domain::ObservationKind::Observability
            )
        }) {
            let supporting: Vec<_> = observations
                .iter()
                .filter(|o| {
                    matches!(
                        o.kind,
                        crate::domain::ObservationKind::Infrastructure
                            | crate::domain::ObservationKind::Observability
                    )
                })
                .map(|o| o.id)
                .collect();
            hypotheses.push(Hypothesis::new(
                investigation_id,
                "Infrastructure or observability signals indicate an environmental contribution.",
                HypothesisStatus::Proposed,
                Confidence::new(0.5),
                supporting,
                Vec::new(),
                related_ids.clone(),
                "infra_obs_signal_v1",
                "unverified",
                rank,
                provenance.clone(),
            ));
            rank += 1;
        }

        if !related_ids.is_empty() {
            hypotheses.push(Hypothesis::new(
                investigation_id,
                "This situation resembles prior Investigations; historical outcomes may inform verification order.",
                HypothesisStatus::Proposed,
                Confidence::new(0.4),
                Vec::new(),
                Vec::new(),
                related_ids,
                "similar_investigation_recall_v1",
                "historical context only — not current fact",
                rank,
                provenance.clone(),
            ));
            rank += 1;
        }

        if hypotheses.is_empty() {
            let supporting: Vec<_> = memory.iter().take(3).map(|m| m.id).collect();
            hypotheses.push(Hypothesis::new(
                investigation_id,
                "Insufficient failure signals; the leading uncertainty is incomplete evidence rather than a known defect.",
                HypothesisStatus::Inconclusive,
                Confidence::new(0.35),
                supporting,
                Vec::new(),
                Vec::new(),
                "evidence_gap_v1",
                "unverified",
                rank,
                provenance,
            ));
        }

        for h in &hypotheses {
            self.store.append_hypothesis(h)?;
        }
        Ok(hypotheses)
    }

    /// Recommend next-best verification steps.
    pub fn recommend_next_verification(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<VerificationSuggestion>> {
        let actor = actor.into();
        let _ = self.store.load_investigation(&investigation_id)?;
        let mut hypotheses = self.store.list_hypotheses(&investigation_id)?;
        if hypotheses.is_empty() {
            hypotheses = self.generate_hypotheses(investigation_id, actor.clone())?;
        }
        let receipts = self.store.list_verifications(&investigation_id)?;
        let memory = self.store.list_memory(&investigation_id)?;
        let provenance =
            Provenance::now(actor, "runtime").with_capability("recommend_next_verification");

        let mut suggestions = Vec::new();
        let mut rank = 1u32;

        if receipts.is_empty() {
            suggestions.push(VerificationSuggestion {
                id: ObjectId::new(),
                investigation_id,
                hypothesis_id: hypotheses.first().map(|h| h.id),
                claim: "Current Evaluation conclusions require verification against Memory.".into(),
                expected_evidence: "Verification Receipt for primary Evaluation".into(),
                reason: "No Verification Receipts exist; confidence remains unvalidated.".into(),
                method: "verify_all Capability".into(),
                estimated_confidence_impact: 0.25,
                prerequisites: vec!["At least one Evaluation".into()],
                feasibility: VerificationFeasibility::Feasible,
                confirmation_required: false,
                supporting_evidence: memory.iter().take(3).map(|m| m.id).collect(),
                rank,
                generated_at: Utc::now(),
                provenance: provenance.clone(),
                metadata: crate::domain::empty_metadata(),
            });
            rank += 1;
        }

        for hyp in hypotheses.iter().take(3) {
            if matches!(
                hyp.status,
                HypothesisStatus::Verified | HypothesisStatus::Rejected
            ) {
                continue;
            }
            let method = if hyp.statement.to_lowercase().contains("ci")
                || hyp.statement.to_lowercase().contains("pipeline")
            {
                "Collect CI / GitHub Actions observations and re-verify"
            } else if hyp.statement.to_lowercase().contains("infrastructure")
                || hyp.statement.to_lowercase().contains("observability")
            {
                "Collect infrastructure or observability connector evidence"
            } else {
                "Inspect supporting Memory and re-run verify_conclusion"
            };
            suggestions.push(VerificationSuggestion {
                id: ObjectId::new(),
                investigation_id,
                hypothesis_id: Some(hyp.id),
                claim: hyp.statement.clone(),
                expected_evidence: format!(
                    "Evidence that supports or contradicts: {}",
                    hyp.statement
                ),
                reason: format!(
                    "Hypothesis rank {} has confidence {:.0}% and status {}; reducing uncertainty here has high value.",
                    hyp.rank,
                    hyp.confidence.value() * 100.0,
                    hyp.status.as_str()
                ),
                method: method.into(),
                estimated_confidence_impact: (0.35 * hyp.confidence.value()).clamp(0.1, 0.4),
                prerequisites: if hyp.supporting_evidence.is_empty() {
                    vec!["Collect more Observations".into()]
                } else {
                    vec!["Review supporting evidence ids".into()]
                },
                feasibility: if hyp.supporting_evidence.is_empty() {
                    VerificationFeasibility::RequiresHuman
                } else {
                    VerificationFeasibility::Feasible
                },
                confirmation_required: false,
                supporting_evidence: hyp.supporting_evidence.clone(),
                rank,
                generated_at: Utc::now(),
                provenance: provenance.clone(),
                metadata: crate::domain::empty_metadata(),
            });
            rank += 1;
        }

        if suggestions.is_empty() {
            suggestions.push(VerificationSuggestion {
                id: ObjectId::new(),
                investigation_id,
                hypothesis_id: None,
                claim: "Maintain verification coverage as new Observations arrive.".into(),
                expected_evidence: "Ongoing Verification Receipts".into(),
                reason: "No high-priority verification gaps detected.".into(),
                method: "verify_all after new evidence".into(),
                estimated_confidence_impact: 0.1,
                prerequisites: vec![],
                feasibility: VerificationFeasibility::Feasible,
                confirmation_required: false,
                supporting_evidence: Vec::new(),
                rank: 1,
                generated_at: Utc::now(),
                provenance,
                metadata: crate::domain::empty_metadata(),
            });
        }

        for s in &suggestions {
            self.store.append_verification_suggestion(s)?;
        }
        Ok(suggestions)
    }

    /// Assess deployment readiness from available evidence.
    pub fn assess_deployment_readiness(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<DeploymentReadiness> {
        let actor = actor.into();
        let _ = self.store.load_investigation(&investigation_id)?;
        let mut knowledge = self.store.list_knowledge(&investigation_id)?;
        if knowledge.is_empty() {
            knowledge = self.derive_knowledge(investigation_id, actor.clone())?;
        }
        let mut evaluations = self.store.list_evaluations(&investigation_id)?;
        if evaluations.is_empty() {
            evaluations = self.evaluate_investigation(investigation_id, actor.clone())?;
        }
        let receipts = self.store.list_verifications(&investigation_id)?;
        let observations = self.store.list_observations(&investigation_id)?;
        let memory = self.store.list_memory(&investigation_id)?;
        let hypotheses = self.store.list_hypotheses(&investigation_id)?;

        let has_risk = knowledge
            .iter()
            .any(|k| matches!(k.kind, KnowledgeKind::RiskSignal));
        let failed_checks = observations.iter().filter(|o| {
            let t = format!(
                "{} {}",
                o.summary.to_lowercase(),
                o.payload.to_string().to_lowercase()
            );
            matches!(
                o.kind,
                crate::domain::ObservationKind::CheckResult
                    | crate::domain::ObservationKind::WorkflowRun
                    | crate::domain::ObservationKind::TestOutput
            ) && (t.contains("fail") || t.contains("error") || t.contains("cancelled"))
        });
        let failed_count = failed_checks.count();
        let verify_fail = receipts
            .iter()
            .any(|r| r.result == VerificationResult::Fail);
        let verify_pass = receipts
            .iter()
            .any(|r| r.result == VerificationResult::Pass);
        let high_risk_hyps = hypotheses.iter().any(|h| {
            h.confidence.value() >= 0.6
                && matches!(
                    h.status,
                    HypothesisStatus::Supported | HypothesisStatus::Proposed
                )
        });

        let mut dimensions = Vec::new();
        let mut blockers = Vec::new();
        let mut warnings = Vec::new();
        let mut supporting = Vec::new();
        let mut contradicting = Vec::new();

        let ci_severity = if failed_count > 0 {
            Severity::High
        } else if observations.iter().any(|o| {
            matches!(
                o.kind,
                crate::domain::ObservationKind::CheckResult
                    | crate::domain::ObservationKind::WorkflowRun
            )
        }) {
            Severity::Low
        } else {
            Severity::Medium
        };
        dimensions.push(ReadinessDimension {
            name: "ci_status".into(),
            status: if failed_count > 0 {
                "failing".into()
            } else if ci_severity == Severity::Low {
                "passing_or_present".into()
            } else {
                "unknown".into()
            },
            severity: ci_severity,
            explanation: format!("{failed_count} failed CI/test observation(s) detected."),
            evidence_ids: observations
                .iter()
                .filter(|o| {
                    matches!(
                        o.kind,
                        crate::domain::ObservationKind::CheckResult
                            | crate::domain::ObservationKind::WorkflowRun
                            | crate::domain::ObservationKind::TestOutput
                    )
                })
                .map(|o| o.id)
                .collect(),
        });
        if failed_count > 0 {
            blockers.push(format!("{failed_count} CI/test failure observation(s)"));
        }

        let risk_severity = evaluations
            .iter()
            .find(|e| e.assessment_type == AssessmentType::Risk)
            .map(|e| e.severity)
            .unwrap_or(Severity::Medium);
        dimensions.push(ReadinessDimension {
            name: "risk_evaluation".into(),
            status: risk_severity.as_str().into(),
            severity: risk_severity,
            explanation: "Derived from current Risk Evaluation.".into(),
            evidence_ids: evaluations
                .iter()
                .filter(|e| e.assessment_type == AssessmentType::Risk)
                .map(|e| e.id)
                .collect(),
        });
        if matches!(risk_severity, Severity::High | Severity::Critical) || has_risk {
            blockers.push("Elevated risk evaluation or RiskSignal knowledge present".into());
            if let Some(e) = evaluations
                .iter()
                .find(|e| e.assessment_type == AssessmentType::Risk)
            {
                supporting.push(e.id);
            }
        }

        dimensions.push(ReadinessDimension {
            name: "verification_coverage".into(),
            status: if verify_pass && !verify_fail {
                "passing".into()
            } else if verify_fail {
                "failing".into()
            } else if receipts.is_empty() {
                "missing".into()
            } else {
                "mixed".into()
            },
            severity: if verify_fail {
                Severity::High
            } else if receipts.is_empty() {
                Severity::Medium
            } else {
                Severity::Low
            },
            explanation: format!(
                "{} Verification Receipt(s); pass present={verify_pass}; fail present={verify_fail}.",
                receipts.len()
            ),
            evidence_ids: receipts.iter().map(|r| r.id).collect(),
        });
        if verify_fail {
            blockers.push("Verification Failure receipts present".into());
        } else if receipts.is_empty() {
            warnings.push("No Verification Receipts yet".into());
        }

        dimensions.push(ReadinessDimension {
            name: "evidence_volume".into(),
            status: format!("{} memory records", memory.len()),
            severity: if memory.is_empty() {
                Severity::Medium
            } else {
                Severity::Info
            },
            explanation: "Readiness confidence scales with evidence volume.".into(),
            evidence_ids: memory.iter().take(5).map(|m| m.id).collect(),
        });
        if memory.is_empty() {
            warnings.push("No Memory records".into());
        }

        if high_risk_hyps {
            warnings.push("Unresolved high-confidence hypotheses remain".into());
        }

        let status = if !blockers.is_empty() {
            ReadinessStatus::Hold
        } else if !warnings.is_empty() || memory.len() < 2 {
            ReadinessStatus::Inspect
        } else if memory.is_empty() {
            ReadinessStatus::Unknown
        } else {
            ReadinessStatus::Ready
        };

        let confidence = match status {
            ReadinessStatus::Ready => Confidence::new(0.7),
            ReadinessStatus::Hold => Confidence::new(0.75),
            ReadinessStatus::Inspect => Confidence::new(0.55),
            ReadinessStatus::Unknown => Confidence::new(0.3),
        };

        if !has_risk && failed_count == 0 {
            contradicting.extend(
                evaluations
                    .iter()
                    .filter(|e| {
                        e.assessment_type == AssessmentType::Risk
                            && matches!(e.severity, Severity::Low | Severity::Info)
                    })
                    .map(|e| e.id),
            );
        }
        supporting.extend(memory.iter().take(3).map(|m| m.id));
        supporting.extend(receipts.iter().map(|r| r.id));

        let recommendation_summary = match status {
            ReadinessStatus::Ready => {
                "Proceed with caution; continue monitoring after deployment.".to_string()
            }
            ReadinessStatus::Hold => {
                "Hold deployment until blockers are resolved and re-verified.".to_string()
            }
            ReadinessStatus::Inspect => {
                "Inspect warnings and collect missing evidence before proceeding.".to_string()
            }
            ReadinessStatus::Unknown => "Insufficient evidence to assess readiness.".to_string(),
        };

        let readiness = DeploymentReadiness {
            id: ObjectId::new(),
            investigation_id,
            status,
            confidence,
            dimensions,
            blockers,
            warnings,
            supporting_evidence: supporting,
            contradicting_evidence: contradicting,
            required_verifications: if receipts.is_empty() {
                vec!["Run verify_all".into()]
            } else if verify_fail {
                vec!["Resolve failed verifications and re-verify".into()]
            } else {
                vec![]
            },
            recommendation_summary,
            assessed_at: Utc::now(),
            provenance: Provenance::now(actor, "runtime")
                .with_capability("assess_deployment_readiness"),
            metadata: crate::domain::empty_metadata(),
        };
        self.store.append_deployment_readiness(&readiness)?;
        Ok(readiness)
    }

    /// Forecast evidence-backed risks.
    pub fn forecast_risk(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<RiskForecast> {
        let actor = actor.into();
        let _ = self.store.load_investigation(&investigation_id)?;
        let knowledge = self.store.list_knowledge(&investigation_id)?;
        let evaluations = self.store.list_evaluations(&investigation_id)?;
        let receipts = self.store.list_verifications(&investigation_id)?;
        let observations = self.store.list_observations(&investigation_id)?;
        let memory = self.store.list_memory(&investigation_id)?;
        let trends = self.summarize_historical_trend(None).ok();
        let prior = self
            .recall_prior_outcomes(crate::runtime::search::OutcomeFilter {
                repository: None,
                similar_to: Some(investigation_id),
                disposition: None,
            })
            .unwrap_or_default();

        let mut items = Vec::new();
        let has_risk = knowledge
            .iter()
            .any(|k| matches!(k.kind, KnowledgeKind::RiskSignal));
        let risk_eval = evaluations
            .iter()
            .find(|e| e.assessment_type == AssessmentType::Risk);

        if has_risk
            || risk_eval.is_some_and(|e| matches!(e.severity, Severity::High | Severity::Critical))
        {
            items.push(RiskItem {
                category: RiskCategory::Regression,
                severity: Severity::High,
                confidence: Confidence::new(0.7),
                supporting_evidence: knowledge
                    .iter()
                    .filter(|k| matches!(k.kind, KnowledgeKind::RiskSignal))
                    .map(|k| k.id)
                    .chain(risk_eval.map(|e| e.id))
                    .collect(),
                historical_comparison: if prior.iter().any(|p| {
                    matches!(
                        p.outcome.disposition,
                        crate::domain::OutcomeDisposition::Unsuccessful
                    )
                }) {
                    "Similar prior Investigations had unsuccessful outcomes.".into()
                } else {
                    "No unsuccessful prior outcomes recalled for similar Investigations.".into()
                },
                mitigation: "Investigate failure signals and re-verify before promoting changes."
                    .into(),
                explanation:
                    "Failure-related Knowledge or high Risk Evaluation elevates regression risk."
                        .into(),
            });
        }

        if observations.iter().any(|o| {
            matches!(
                o.kind,
                crate::domain::ObservationKind::WorkflowRun
                    | crate::domain::ObservationKind::CheckResult
            ) && format!("{} {}", o.summary, o.payload)
                .to_lowercase()
                .contains("fail")
        }) {
            items.push(RiskItem {
                category: RiskCategory::Deployment,
                severity: Severity::High,
                confidence: Confidence::new(0.65),
                supporting_evidence: observations
                    .iter()
                    .filter(|o| {
                        matches!(
                            o.kind,
                            crate::domain::ObservationKind::WorkflowRun
                                | crate::domain::ObservationKind::CheckResult
                        )
                    })
                    .map(|o| o.id)
                    .collect(),
                historical_comparison: trends
                    .as_ref()
                    .map(|t| {
                        format!(
                            "Historical sample covers {} Investigation(s).",
                            t.investigation_count
                        )
                    })
                    .unwrap_or_else(|| "No trend sample available.".into()),
                mitigation: "Hold deployment until CI/workflow failures are green.".into(),
                explanation: "Failing CI/workflow observations increase deployment risk.".into(),
            });
        }

        if receipts
            .iter()
            .any(|r| r.result == VerificationResult::Fail)
            || receipts.is_empty()
        {
            items.push(RiskItem {
                category: RiskCategory::Verification,
                severity: if receipts
                    .iter()
                    .any(|r| r.result == VerificationResult::Fail)
                {
                    Severity::High
                } else {
                    Severity::Medium
                },
                confidence: Confidence::new(0.6),
                supporting_evidence: receipts.iter().map(|r| r.id).collect(),
                historical_comparison: "Verification risk is based on current receipts only."
                    .into(),
                mitigation: "Run or re-run verification after addressing gaps.".into(),
                explanation: "Missing or failed verification leaves conclusions unvalidated."
                    .into(),
            });
        }

        if memory.len() < 2 {
            items.push(RiskItem {
                category: RiskCategory::EvidenceQuality,
                severity: Severity::Medium,
                confidence: Confidence::new(0.55),
                supporting_evidence: memory.iter().map(|m| m.id).collect(),
                historical_comparison: "Evidence quality risk is local to this Investigation."
                    .into(),
                mitigation: "Collect more Observations via connectors before concluding.".into(),
                explanation: "Low Memory volume reduces confidence in all assistance outputs."
                    .into(),
            });
        }

        if prior.iter().any(|p| {
            matches!(
                p.outcome.disposition,
                crate::domain::OutcomeDisposition::Unsuccessful
            )
        }) {
            items.push(RiskItem {
                category: RiskCategory::Recurrence,
                severity: Severity::Medium,
                confidence: Confidence::new(0.45),
                supporting_evidence: Vec::new(),
                historical_comparison: format!(
                    "{} prior related outcome(s) include unsuccessful dispositions.",
                    prior.len()
                ),
                mitigation: "Review prior unsuccessful mitigations; do not auto-repeat them."
                    .into(),
                explanation: "Historical unsuccessful outcomes raise recurrence risk (labeled historical, not current fact)."
                    .into(),
            });
        }

        if items.is_empty() {
            items.push(RiskItem {
                category: RiskCategory::Operational,
                severity: Severity::Low,
                confidence: Confidence::new(0.5),
                supporting_evidence: memory.iter().take(3).map(|m| m.id).collect(),
                historical_comparison: "No elevated risk signals detected.".into(),
                mitigation: "Continue monitoring and keep verification current.".into(),
                explanation: "Baseline operational risk with no strong elevating evidence.".into(),
            });
        }

        let summary = format!(
            "Forecasted {} risk item(s); highest severity={}.",
            items.len(),
            items
                .iter()
                .map(|i| i.severity.as_str())
                .max_by_key(|s| match *s {
                    "critical" => 4,
                    "high" => 3,
                    "medium" => 2,
                    "low" => 1,
                    _ => 0,
                })
                .unwrap_or("info")
        );

        let forecast = RiskForecast {
            id: ObjectId::new(),
            investigation_id,
            items,
            summary,
            forecasted_at: Utc::now(),
            provenance: Provenance::now(actor, "runtime").with_capability("forecast_risk"),
            metadata: crate::domain::empty_metadata(),
        };
        self.store.append_risk_forecast(&forecast)?;
        Ok(forecast)
    }

    /// Generate probabilistic root-cause guidance.
    pub fn generate_root_cause_guidance(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<RootCauseGuidance> {
        let actor = actor.into();
        let mut hypotheses = self.store.list_hypotheses(&investigation_id)?;
        if hypotheses.is_empty() {
            hypotheses = self.generate_hypotheses(investigation_id, actor.clone())?;
        }
        let suggestions = self
            .store
            .list_verification_suggestions(&investigation_id)?;
        let memory = self.store.list_memory(&investigation_id)?;
        let prior = self
            .recall_prior_outcomes(crate::runtime::search::OutcomeFilter {
                repository: None,
                similar_to: Some(investigation_id),
                disposition: None,
            })
            .unwrap_or_default();

        let leading: Vec<_> = hypotheses.iter().take(3).cloned().collect();
        let leading_ids: Vec<_> = leading.iter().map(|h| h.id).collect();
        let supporting: Vec<_> = leading
            .iter()
            .flat_map(|h| h.supporting_evidence.iter().copied())
            .chain(memory.iter().take(3).map(|m| m.id))
            .collect();
        let contradicting: Vec<_> = leading
            .iter()
            .flat_map(|h| h.contradicting_evidence.iter().copied())
            .collect();
        let mut related: Vec<_> = leading
            .iter()
            .flat_map(|h| h.related_investigation_ids.iter().copied())
            .collect();
        related.sort_by_key(|id| id.to_string());
        related.dedup();

        let prior_notes: Vec<String> = prior
            .iter()
            .take(5)
            .map(|p| {
                format!(
                    "Historical: investigation {} disposition={} note={}",
                    p.investigation_id,
                    p.outcome.disposition.as_str(),
                    p.recommendation_summary
                        .as_deref()
                        .unwrap_or("(no recommendation summary)")
                )
            })
            .collect();

        let guidance = if let Some(top) = leading.first() {
            format!(
                "Leading (probabilistic) hypothesis: {} (confidence {:.0}%, status {}). \
                 This is not a verified root cause. Supporting and contradicting evidence remain inspectable. \
                 Historical mitigations must not be auto-applied.",
                top.statement,
                top.confidence.value() * 100.0,
                top.status.as_str()
            )
        } else {
            "No leading hypothesis; collect more evidence.".into()
        };

        let verification_order: Vec<String> = if !suggestions.is_empty() {
            suggestions.iter().map(|s| s.claim.clone()).collect()
        } else {
            leading.iter().map(|h| h.statement.clone()).collect()
        };

        let mut known_gaps = Vec::new();
        if memory.len() < 2 {
            known_gaps.push("Sparse Memory reduces root-cause confidence.".into());
        }
        if leading
            .iter()
            .all(|h| h.verification_summary.contains("unverified"))
        {
            known_gaps.push("No hypothesis has been verified yet.".into());
        }

        let conf = leading
            .first()
            .map(|h| Confidence::new(h.confidence.value() * 0.85))
            .unwrap_or_else(|| Confidence::new(0.3));

        let result = RootCauseGuidance {
            id: ObjectId::new(),
            investigation_id,
            leading_hypothesis_ids: leading_ids,
            guidance,
            supporting_evidence: supporting,
            contradicting_evidence: contradicting,
            related_investigation_ids: related,
            prior_mitigation_notes: prior_notes,
            confidence: conf,
            verification_order,
            known_gaps,
            generated_at: Utc::now(),
            provenance: Provenance::now(actor, "runtime")
                .with_capability("generate_root_cause_guidance"),
            metadata: crate::domain::empty_metadata(),
        };
        self.store.append_root_cause_guidance(&result)?;
        Ok(result)
    }

    /// Prioritize Recommendations with inspectable factors.
    pub fn prioritize_recommendations(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<Vec<PrioritizedRecommendation>> {
        let actor = actor.into();
        let mut recommendations = self.store.list_recommendations(&investigation_id)?;
        if recommendations.is_empty() {
            recommendations = self.generate_recommendation(investigation_id, actor)?;
        }
        let receipts = self.store.list_verifications(&investigation_id)?;
        let evaluations = self.store.list_evaluations(&investigation_id)?;
        let prior = self
            .recall_prior_outcomes(crate::runtime::search::OutcomeFilter {
                repository: None,
                similar_to: Some(investigation_id),
                disposition: None,
            })
            .unwrap_or_default();

        let mut ranked = Vec::new();
        for rec in &recommendations {
            let mut factors = Vec::new();
            let evidence_strength =
                (rec.evaluation_ids.len() + rec.verification_ids.len()) as f64 / 6.0;
            let evidence_strength = evidence_strength.clamp(0.1, 1.0);
            factors.push(RankingFactor {
                name: "evidence_strength".into(),
                weight: 0.25,
                contribution: 0.25 * evidence_strength,
                explanation: format!(
                    "{} evaluation + {} verification references",
                    rec.evaluation_ids.len(),
                    rec.verification_ids.len()
                ),
            });

            let verify_score = if receipts
                .iter()
                .any(|r| r.result == VerificationResult::Pass)
            {
                0.9
            } else if receipts
                .iter()
                .any(|r| r.result == VerificationResult::Fail)
            {
                0.2
            } else {
                0.4
            };
            factors.push(RankingFactor {
                name: "verification_status".into(),
                weight: 0.2,
                contribution: 0.2 * verify_score,
                explanation: format!("{} verification receipt(s) considered", receipts.len()),
            });

            let risk_urgency = evaluations
                .iter()
                .find(|e| e.assessment_type == AssessmentType::Risk)
                .map(|e| match e.severity {
                    Severity::Critical => 1.0,
                    Severity::High => 0.85,
                    Severity::Medium => 0.5,
                    Severity::Low => 0.3,
                    Severity::Info => 0.2,
                })
                .unwrap_or(0.4);
            factors.push(RankingFactor {
                name: "urgency".into(),
                weight: 0.2,
                contribution: 0.2 * risk_urgency,
                explanation: "Derived from Risk Evaluation severity.".into(),
            });

            factors.push(RankingFactor {
                name: "confidence".into(),
                weight: 0.15,
                contribution: 0.15 * rec.confidence.value(),
                explanation: format!(
                    "Recommendation confidence {:.0}%",
                    rec.confidence.value() * 100.0
                ),
            });

            let prior_success = if prior.iter().any(|p| {
                matches!(
                    p.outcome.disposition,
                    crate::domain::OutcomeDisposition::Successful
                )
            }) {
                0.7
            } else if prior.iter().any(|p| {
                matches!(
                    p.outcome.disposition,
                    crate::domain::OutcomeDisposition::Unsuccessful
                )
            }) {
                0.35
            } else {
                0.5
            };
            factors.push(RankingFactor {
                name: "prior_outcome_success".into(),
                weight: 0.1,
                contribution: 0.1 * prior_success,
                explanation: "Historical outcomes are labeled context only.".into(),
            });

            // Reversibility baseline: monitoring-style recs score higher.
            let reversible = if rec.summary.to_lowercase().contains("monitor") {
                0.9
            } else {
                0.55
            };
            factors.push(RankingFactor {
                name: "reversibility".into(),
                weight: 0.1,
                contribution: 0.1 * reversible,
                explanation: "Monitoring recommendations are treated as more reversible.".into(),
            });

            let score: f64 = factors.iter().map(|f| f.contribution).sum();
            let explanation = factors
                .iter()
                .map(|f| format!("{}={:.2} (w={:.2})", f.name, f.contribution, f.weight))
                .collect::<Vec<_>>()
                .join("; ");

            ranked.push(PrioritizedRecommendation {
                recommendation_id: rec.id,
                rank: 0,
                score,
                summary: rec.summary.clone(),
                factors,
                explanation: format!(
                    "Score {:.3} from inspectable factors: {explanation}. Proposal only; never auto-applied.",
                    score
                ),
            });
        }

        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.recommendation_id
                        .to_string()
                        .cmp(&b.recommendation_id.to_string())
                })
        });
        for (i, item) in ranked.iter_mut().enumerate() {
            item.rank = (i + 1) as u32;
        }
        Ok(ranked)
    }

    /// Generate a durable engineering report from Runtime data.
    pub fn generate_engineering_report(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<EngineeringReport> {
        let actor = actor.into();
        let inv = self.store.load_investigation(&investigation_id)?;
        let memory = self.store.list_memory(&investigation_id)?;
        let knowledge = self.store.list_knowledge(&investigation_id)?;
        let evaluations = self.store.list_evaluations(&investigation_id)?;
        let receipts = self.store.list_verifications(&investigation_id)?;
        let recommendations = self.store.list_recommendations(&investigation_id)?;
        let hypotheses = self.store.list_hypotheses(&investigation_id)?;
        let readiness = self.store.list_deployment_readiness(&investigation_id)?;
        let risks = self.store.list_risk_forecasts(&investigation_id)?;
        let learning = self.store.list_learning(&investigation_id)?;
        let context = self.store.list_recalled_context(&investigation_id)?;

        let mut sections = Vec::new();
        sections.push(ReportSection {
            title: "Investigation".into(),
            body: format!(
                "Title: {}\nStatus: {}\nDescription: {}",
                inv.title,
                inv.status.as_str(),
                inv.description.as_deref().unwrap_or("(none)")
            ),
            object_refs: Vec::new(),
        });
        sections.push(ReportSection {
            title: "Evidence".into(),
            body: format!(
                "{} Memory record(s). Latest: {}",
                memory.len(),
                memory
                    .last()
                    .map(|m| m.summary.as_str())
                    .unwrap_or("(none)")
            ),
            object_refs: memory.iter().map(|m| m.id).collect(),
        });
        sections.push(ReportSection {
            title: "Knowledge".into(),
            body: knowledge
                .iter()
                .map(|k| format!("- [{}] {}", k.kind.as_str(), k.summary))
                .collect::<Vec<_>>()
                .join("\n"),
            object_refs: knowledge.iter().map(|k| k.id).collect(),
        });
        sections.push(ReportSection {
            title: "Evaluations".into(),
            body: evaluations
                .iter()
                .map(|e| {
                    format!(
                        "- {} severity={} conf={:.0}%: {}",
                        e.assessment_type.as_str(),
                        e.severity.as_str(),
                        e.confidence.value() * 100.0,
                        e.summary
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            object_refs: evaluations.iter().map(|e| e.id).collect(),
        });
        sections.push(ReportSection {
            title: "Verifications".into(),
            body: receipts
                .iter()
                .map(|r| format!("- {} — {}", r.result.as_str(), r.subject))
                .collect::<Vec<_>>()
                .join("\n"),
            object_refs: receipts.iter().map(|r| r.id).collect(),
        });
        sections.push(ReportSection {
            title: "Hypotheses".into(),
            body: if hypotheses.is_empty() {
                "(none generated yet)".into()
            } else {
                hypotheses
                    .iter()
                    .map(|h| {
                        format!(
                            "- rank {} [{}/{:.0}%]: {}",
                            h.rank,
                            h.status.as_str(),
                            h.confidence.value() * 100.0,
                            h.statement
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            object_refs: hypotheses.iter().map(|h| h.id).collect(),
        });
        if let Some(r) = readiness.last() {
            sections.push(ReportSection {
                title: "Deployment Readiness".into(),
                body: format!(
                    "Status: {} (conf {:.0}%)\nBlockers: {}\nWarnings: {}\nRecommendation: {}",
                    r.status.as_str(),
                    r.confidence.value() * 100.0,
                    if r.blockers.is_empty() {
                        "(none)".into()
                    } else {
                        r.blockers.join("; ")
                    },
                    if r.warnings.is_empty() {
                        "(none)".into()
                    } else {
                        r.warnings.join("; ")
                    },
                    r.recommendation_summary
                ),
                object_refs: vec![r.id],
            });
        }
        if let Some(f) = risks.last() {
            sections.push(ReportSection {
                title: "Risk Forecast".into(),
                body: format!(
                    "{}\n{}",
                    f.summary,
                    f.items
                        .iter()
                        .map(|i| format!(
                            "- {} severity={}: {}",
                            i.category.as_str(),
                            i.severity.as_str(),
                            i.explanation
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
                object_refs: vec![f.id],
            });
        }
        sections.push(ReportSection {
            title: "Recommendations".into(),
            body: recommendations
                .iter()
                .map(|r| {
                    format!(
                        "- [{} conf={:.0}%] {} — {}",
                        r.status.as_str(),
                        r.confidence.value() * 100.0,
                        r.summary,
                        r.rationale
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            object_refs: recommendations.iter().map(|r| r.id).collect(),
        });
        sections.push(ReportSection {
            title: "Historical Context".into(),
            body: format!(
                "{} Recalled Context record(s); {} Learning Outcome(s). Historical context is labeled and never auto-applied.",
                context.len(),
                learning.len()
            ),
            object_refs: context.iter().map(|c| c.id).collect(),
        });

        let mut markdown = format!("# Engineering Report: {}\n\n", inv.title);
        for section in &sections {
            markdown.push_str(&format!("## {}\n\n{}\n\n", section.title, section.body));
        }
        markdown
            .push_str("---\nGenerated by Rivora Runtime. Recommendations are proposals only.\n");

        let report = EngineeringReport::new(
            investigation_id,
            format!("Engineering Report — {}", inv.title),
            sections,
            markdown,
            Provenance::now(actor, "runtime").with_capability("generate_engineering_report"),
        );
        self.store.append_engineering_report(&report)?;
        Ok(report)
    }

    /// Summarize Investigation state for assistance and workflows.
    pub fn summarize_investigation_state(
        &self,
        investigation_id: InvestigationId,
        actor: impl Into<String>,
    ) -> RivoraResult<InvestigationSummary> {
        let actor = actor.into();
        let inv = self.store.load_investigation(&investigation_id)?;
        let memory = self.store.list_memory(&investigation_id)?;
        let knowledge = self.store.list_knowledge(&investigation_id)?;
        let evaluations = self.store.list_evaluations(&investigation_id)?;
        let verifications = self.store.list_verifications(&investigation_id)?;
        let recommendations = self.store.list_recommendations(&investigation_id)?;
        let hypotheses = self.store.list_hypotheses(&investigation_id)?;
        let learning = self.store.list_learning(&investigation_id)?;

        let mut gaps = Vec::new();
        if memory.is_empty() {
            gaps.push("No Memory — collect Observations.".into());
        }
        if knowledge.is_empty() {
            gaps.push("No Knowledge — run derive_knowledge.".into());
        }
        if evaluations.is_empty() {
            gaps.push("No Evaluations — run evaluate.".into());
        }
        if verifications.is_empty() {
            gaps.push("No Verification Receipts — run verify.".into());
        }
        if recommendations.is_empty() {
            gaps.push("No Recommendations yet.".into());
        }
        if hypotheses
            .iter()
            .any(|h| h.verification_summary.contains("unverified"))
        {
            gaps.push("Unverified hypotheses remain.".into());
        }

        let summary = format!(
            "Investigation `{}` is {} with {} memory, {} knowledge, {} evaluations, {} verifications, {} recommendations, {} hypotheses. {}",
            inv.title,
            inv.status.as_str(),
            memory.len(),
            knowledge.len(),
            evaluations.len(),
            verifications.len(),
            recommendations.len(),
            hypotheses.len(),
            if gaps.is_empty() {
                "No major gaps detected.".to_string()
            } else {
                format!("Gaps: {}", gaps.join(" "))
            }
        );

        Ok(InvestigationSummary {
            investigation_id,
            title: inv.title,
            status: inv.status.as_str().into(),
            summary,
            counts: SummaryCounts {
                memory: memory.len(),
                knowledge: knowledge.len(),
                evaluations: evaluations.len(),
                verifications: verifications.len(),
                recommendations: recommendations.len(),
                hypotheses: hypotheses.len(),
                learning: learning.len(),
            },
            gaps,
            summarized_at: Utc::now(),
            provenance: Provenance::now(actor, "runtime")
                .with_capability("summarize_investigation_state"),
        })
    }
}
