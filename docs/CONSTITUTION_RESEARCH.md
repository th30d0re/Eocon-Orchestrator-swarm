# Research Brief: Constitutional Principles for an Agentic Software-Engineering Swarm

**Source notebook:** *Aligning Multi-Agent Systems: Governance, Security, and Curation*  
**Notebook ID:** `a9f74988-db21-4714-bd80-11e8dd3a94c0`  
**Query tool:** `~/.local/bin/nlm` (NotebookLM Tools CLI)  
**Research date:** 2026-07-06  
**Author:** Research agent (Kimi Code CLI)

---

## Notebook Overview

The notebook exists and contains **67 sources** spanning multi-agent systems, swarm intelligence, AI safety, alignment, decentralized coordination, and agent evaluation. The corpus is weighted toward recent arXiv preprints (many dated 2025–2026) and includes a small number of web articles, PDFs, GitHub docs, and one audio file.

The materials cluster around several recurring themes that are directly relevant to a constitutional framework for an agentic software-engineering swarm:

- Decentralized coordination (shared context, ledgers, stigmergy)
- Hierarchical policy control and runtime safety
- Reputation, consensus, and tribunal-like quality gates
- Information sharing boundaries and latent-channel privacy
- Verification, trace auditing, and anti-deception measures
- Human oversight and process-level supervision

---

## Key Sources

The following sources were repeatedly surfaced across queries and are the most relevant for constitutional drafting.

| Source ID | Title | Relevance |
|-----------|-------|-----------|
| `c178772a-ab73-4869-859d-4c5a8eee4813` | Beyond Static Alignment: Hierarchical Policy Control for LLM Safety via Risk-Aware Chain-of-Thought (arXiv) | Two-tier policy architecture (Global vs. User Policy) |
| `8b3878fa-324c-4a97-8164-df2b7d7a2b97` | Decentralized Multi-Agent Systems with Shared Context (arXiv) | Admission-time verification, gists, unfoldable memory |
| `6c4ef21d-ddd4-49fc-9510-a70e12cd9797` | LCGuard: Latent Communication Guard for Safe KV Sharing in Multi-Agent Systems (arXiv) | Latent-channel privacy, KV-cache sanitization |
| `b8384f98-3bb0-4408-96e7-c80078cde7f5` | Fortytwo: Swarm Inference with Peer-Ranked Consensus (arXiv) | Pairwise ranking, reputation, proof-of-capability |
| `bdadb8ce-a361-4846-922f-7ea942bb7495` | OpenCLAW-P2P v7.0: Resilient Multi-Layer Persistence, Live Reference Verification, and Production-Scale Evaluation of Decentralized AI Peer Review (arXiv) | Tribunal system, conflict-of-interest rules, reputation updates |
| `8366360f-9ff0-4480-be60-a7331dbea216` | Paradigms of Coordination Engineering: Evaluative Harnesses, Dynamic Safety, and Cryptographic Governance in Multi-Agent Systems and AI Swarms (generated text) | Governance primitives, cryptographic identity, Swarm Skills |
| `63f4b6f9-96b6-4d82-89a7-aa8fd355a362` | How to Steer Your Multi-Agent System: Human-LLM Collaborative Planning (arXiv) | DAG-based process supervision, human-in-the-loop co-planning |
| `df8d3808-6bc8-42bd-86f8-baf3ede81f96` | Daily Papers - Hugging Face (curated paper list) | AgentWall, TrustAgent, AgentForesight, SafePred, symbolic guardrails |
| `6edda78b-2192-420a-85f8-fdf95148981f` | Beyond Goodhart's Law: A Dynamic Benchmark for Evaluating Compliance in Multi-Agent Systems (arXiv PDF) | Procedural compliance, Machiavellian Gap, trace auditing |
| `fe8c10f7-da4a-428c-8980-774f81e65539` | Swarm Skills: A Portable, Self-Evolving Multi-Agent System Specification for Coordination Engineering (arXiv) | Self-evolving coordination, E/U/F scoring, rollback |
| `ff98b401-7a58-4d2e-af30-1fd4ac150f42` | AI Alignment Research - AE Studio | Self-Other Overlap, SAE feature steering |
| `276dc90a-dbff-4be3-bf63-179ad500d326` | Cutting-edge swarm intelligence research - Scouts by Yutori | Ledger-state stigmergy, anomaly monitoring |

---

## Governance Principles

1. **Hierarchical policy control.** A constitution should separate non-negotiable global safety baselines from flexible domain policies. PACT (`c178772a-ab73-4869-859d-4c5a8eee4813`) proposes a parameter-level Global Policy for critical risks and a prompt-configurable User Policy for domain rules. This prevents local optimization from overriding universal safety constraints.

2. **Decentralized shared context with admission verification.** DeLM (`8b3878fa-324c-4a97-8164-df2b7d7a2b97`) replaces centralized orchestration with a task queue and shared context board. Every update must pass an LLM-as-a-verifier check against cited evidence before admission.

3. **Ledger-state stigmergy.** For high-latency or adversarial environments, agents should coordinate indirectly via an immutable cryptographic ledger rather than direct message passing (`276dc90a-dbff-4be3-bf63-179ad500d326`, `8366360f-9ff0-4480-be60-a7331dbea216`).

4. **Peer-ranked consensus.** Fortytwo (`b8384f98-3bb0-4408-96e7-c80078cde7f5`) recommends distributed pairwise ranking with multi-token reasoning chains and Bradley–Terry aggregation, weighted by reputation, instead of simple majority voting.

5. **Reputation-weighted influence with slashing.** Influence should scale with demonstrated accuracy and quality, decay with inactivity, and be reduced ("slashed") for adversarial or persistently low-quality behavior (`b8384f98-3bb0-4408-96e7-c80078cde7f5`, `bdadb8ce-a361-4846-922f-7ea942bb7495`).

6. **Tribunal quality gates and conflict-of-interest rules.** The OpenCLAW-P2P tribunal (`bdadb8ce-a361-4846-922f-7ea942bb7495`) requires agents to pass a structured cognitive examination before publishing; examiners are barred from evaluating their own work.

7. **Soft-label governance.** SWARM (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`) uses continuous probabilistic risk labels and adjustable levers (transaction taxes, circuit breakers, random audits, reputation decay) rather than binary good/bad classifications.

---

## Safety & Security Principles

1. **Runtime interception of host actions.** Proposed actions that touch the filesystem, shell, APIs, or network must be intercepted by a declarative policy layer (AgentWall/ILION) and routed for human approval when sensitive (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`).

2. **Symbolic guardrails for verifiable guarantees.** Symbolic guardrails can enforce 74% of specified policy requirements with strong, mathematically checkable guarantees, unlike neural-only safeguards (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`).

3. **Execution bounds.** Swarm Skills (`fe8c10f7-da4a-428c-8980-774f81e65539`) impose maximum step thresholds, budget limits, and API sandboxing rules to contain runaway behavior.

4. **Resource governors.** Mathematical regulators such as Chebyshev guards for memory eviction and Lyapunov-stable PD governors for workload stability can bound resource consumption (`bdadb8ce-a361-4846-922f-7ea942bb7495`, `8366360f-9ff0-4480-be60-a7331dbea216`).

5. **Cryptographic identity and capability tokens.** Agent identity should use hierarchical deterministic key derivation (Ed25519), W3C DIDs, and signed capability tokens that specify scope, expiry, and delegation rights (`bdadb8ce-a361-4846-922f-7ea942bb7495`).

6. **Three-stage constitutional execution.** TrustAgent (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`) structures safety around pre-planning, in-planning, and post-planning inspection.

---

## Information Sharing Norms

1. **Share compressed, verified updates only.** Agents should publish compact "gists" to a shared context board, not raw logs or full outputs. Deeper summaries and raw evidence remain available on demand (`8b3878fa-324c-4a97-8164-df2b7d7a2b97`).

2. **Keep agent-specific inputs private.** User context, proprietary retrieved documents, system prompts, and intermediate reasoning states must remain localized to the agent (`6c4ef21d-ddd4-49fc-9510-a70e12cd9797`).

3. **Prevent shared-memory pollution.** Unverified claims and raw tool outputs must not be dumped into shared memory, because downstream agents may treat a single hallucination as ground truth (`27082091-93a7-44af-8a82-ce0838184a5b`, `8b3878fa-324c-4a97-8164-df2b7d7a2b97`).

4. **Sanitize latent communication channels.** KV-cache sharing between agents improves efficiency but leaks sensitive information at the representation level. LCGuard (`6c4ef21d-ddd4-49fc-9510-a70e12cd9797`) applies adversarial residual bottleneck transformations to limit reconstructable private information.

5. **Label data flows by confidentiality and trust level.** Capability-enhanced MCP-style labels on capabilities, confidentiality, and trust level should constrain inter-agent data flows (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`).

---

## Verification & Curation

1. **Admission-time verification.** Before any agent output enters shared memory, it must be checked against cited evidence via an LLM-as-a-verifier protocol (`8b3878fa-324c-4a97-8164-df2b7d7a2b97`).

2. **Trajectory-level evaluation.** Compliance is a property of process, not only final output. MAC-Bench (`6edda78b-2192-420a-85f8-fdf95148981f`, `bf244c16-fe7b-4e60-bb13-43e935b2d9b3`) audits complete execution traces and measures the Machiavellian Gap between success and compliance.

3. **Multi-judge ensemble calibration.** Automated evaluators should be calibrated against human preference data, with a production target of ≥0.80 Spearman correlation with expert judgment, using randomized presentation and minority-veto mechanisms (`2dba4977-3374-4f5b-973b-4521a24d0677`).

4. **Formal verification for claims.** OpenCLAW-P2P (`bdadb8ce-a361-4846-922f-7ea942bb7495`) uses Lean4 theorem proving and a commit-reveal protocol, issuing a Certificate of Authenticity and Bounds (CAB) on success.

5. **Online auditing for early failure detection.** AgentForesight (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`) reframes failure detection as an online auditing problem: an auditor observes only the trajectory prefix and triggers an alarm at the earliest decisive error.

6. **Self-evolving Swarm Skills with rollback.** Successful trajectories should be distilled into reusable Swarm Skills and scored by Effectiveness, Utilization, and Freshness; automated SIMPLIFY, REBUILD, and ROLLBACK routines prevent low-quality patch accumulation (`fe8c10f7-da4a-428c-8980-774f81e65539`, `8366360f-9ff0-4480-be60-a7331dbea216`).

---

## Risk & Autonomy Framework

1. **Two-tier risk classification.** Global Policy covers immutable critical risks (e.g., child safety, violent extremism, malicious code execution, data exfiltration). User Policy covers configurable, domain-specific risks (`c178772a-ab73-4869-859d-4c5a8eee4813`).

2. **Autonomy proportional to risk.** Risks to people and systems increase with the autonomy ceded to agents. Higher-risk actions require stronger oversight (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`).

3. **Human-in-the-loop co-planning.** Plans should be represented as editable Directed Acyclic Graphs (DAGs) where humans can manipulate nodes, reassign agents, and alter dependencies before and during execution (`63f4b6f9-96b6-4d82-89a7-aa8fd355a362`).

4. **Runtime intervention points.** Sensitive host-level operations require explicit human approval; automated monitors can also trigger alarms during execution (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`, `fe8c10f7-da4a-428c-8980-774f81e65539`).

5. **Cold-start safety mitigation.** Agents are most vulnerable at the start of a session; warming them with a few routine tasks before safety-critical work improves safety while preserving capability (`346a1b1e-b820-42c8-ae18-bdb756e95532`, `df8d3808-6bc8-42bd-86f8-baf3ede81f96`).

---

## Recommended Constitutional Clauses (numbered list)

The following clauses synthesize the principles above into draftable constitutional language. They are research recommendations, not a final constitution.

1. **Proof-of-Capability and Compute Staking.** No agent may join the active swarm or participate in consensus without passing a dynamically generated cognitive examination in relevant technical domains. The computational cost of this examination serves as a non-forgeable stake and a Sybil-defense mechanism (`b8384f98-3bb0-4408-96e7-c80078cde7f5`).

2. **Two-Tiered Hierarchical Policy Control.** Every agent shall operate under an immutable Global Policy enforced at the parameter level for critical risks, plus a configurable User Policy for domain-specific software-engineering rules (`c178772a-ab73-4869-859d-4c5a8eee4813`).

3. **Hierarchical, Unfoldable Shared Context.** Agents may publish only compact, verified "gists" to shared memory. Deeper summaries and raw evidence must remain hidden until explicitly unfolded by a downstream agent or human (`8b3878fa-324c-4a97-8164-df2b7d7a2b97`).

4. **Decentralized Admission-Time Verification.** No update may enter the shared context until an LLM-as-a-verifier protocol checks it against cited evidence; unsupported claims are dropped or regenerated (`8b3878fa-324c-4a97-8164-df2b7d7a2b97`).

5. **Latent Communication Guarding.** Direct sharing of KV caches or other latent representations is prohibited unless sanitized by representation-level transformations that limit reconstructable sensitive information (`6c4ef21d-ddd4-49fc-9510-a70e12cd9797`).

6. **Explicit Process-Level Planning via DAGs.** All multi-agent workflows must be represented as Directed Acyclic Graphs that human overseers can inspect and manipulate structurally before and during execution (`63f4b6f9-96b6-4d82-89a7-aa8fd355a362`).

7. **Ledger-State Stigmergy.** For critical state transitions, the swarm shall coordinate indirectly via a cryptographically verified, immutable shared ledger rather than fragile direct message passing (`276dc90a-dbff-4be3-bf63-179ad500d326`, `8366360f-9ff0-4480-be60-a7331dbea216`).

8. **Distributed Pairwise Ranking Consensus.** For code evaluation and synthesis, consensus shall use distributed pairwise peer ranking with multi-token reasoning chains and Bradley–Terry aggregation, weighted by reputation (`b8384f98-3bb0-4408-96e7-c80078cde7f5`).

9. **Strict Prohibition of Self-Evaluation.** Consensus mechanisms and tribunals must cryptographically reject self-comparisons, and examiners are permanently excluded from evaluating their own submissions (`b8384f98-3bb0-4408-96e7-c80078cde7f5`, `bdadb8ce-a361-4846-922f-7ea942bb7495`).

10. **Reputation-Weighted Influence and Slashing.** Voting weight in consensus decisions shall scale with historical accuracy and normalized progress; agents that submit adversarial or persistently low-quality work shall be slashed (`b8384f98-3bb0-4408-96e7-c80078cde7f5`, `bdadb8ce-a361-4846-922f-7ea942bb7495`).

11. **Continuous Deception and Anomaly Detection.** All published artifacts, execution logs, and network interactions must be continuously audited for graph-based anomalies and Machiavellian rule-bending (`276dc90a-dbff-4be3-bf63-179ad500d326`, `6edda78b-2192-420a-85f8-fdf95148981f`, `bf244c16-fe7b-4e60-bb13-43e935b2d9b3`).

12. **Three-Stage Constitutional Execution.** Agent planning and execution must follow pre-planning safety injection, in-planning constraint enforcement, and post-planning trajectory inspection (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`).

13. **Runtime Interception of Host Actions.** Any proposed host-level operation shall be intercepted by an independent, declarative runtime safety proxy and routed for human approval when sensitive (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`).

14. **Autonomous Evolution via Experience Distillation.** The swarm shall distill successful trajectories into portable, reusable Swarm Skills, scored by Effectiveness, Utilization, and Freshness, with automated SIMPLIFY, REBUILD, and ROLLBACK routines (`fe8c10f7-da4a-428c-8980-774f81e65539`, `8366360f-9ff0-4480-be60-a7331dbea216`).

15. **Online Auditing for Early Failure Prediction.** An online auditor shall evaluate agent trajectories step-by-step and trigger an alarm or intervention at the earliest decisive error before downstream agents compound it (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`).

---

## Open Questions

1. **Enforcement mechanism.** Which clauses can be enforced purely through prompts and policy configuration, and which require dedicated runtime infrastructure (e.g., AgentWall-style proxies, ledger nodes, KV sanitizers)?

2. **Human oversight granularity.** How should the constitution define "sensitive" host actions requiring human approval? Should the threshold be task-type, file-path, API-scope, or risk-score based?

3. **Reputation and slashing economics.** Can a software-engineering swarm replicate proof-of-capability and reputation slashing without an on-chain token layer, or should it use a lighter-weight in-memory ledger?

4. **Verification latency.** Admission-time verification, pairwise consensus, and Lean4 formal proof checking add latency. What is the acceptable overhead for a software-engineering swarm, and which clauses should be optional versus mandatory?

5. **Latent-channel leakage.** If the swarm does not share KV caches, is LCGuard still relevant, or should the constitution focus on text-channel privacy controls such as those benchmarked by AgentLeak (`df8d3808-6bc8-42bd-86f8-baf3ede81f96`)?

6. **Constitutional versioning.** How should the constitution itself be amended? Should amendments follow the same consensus/tribunal procedures recommended for code artifacts?

7. **Tooling integration.** Several sources reference tools (AgentWall, ILION, Lean4, MCP) that are not yet part of the Eocon-Orchestrator-swarm stack. A follow-up step should assess which are available or need to be built.

---

## nlm CLI Issues Observed

- `nlm source list` requires the notebook ID as a positional argument; the `--notebook` flag is not accepted.
- `nlm query` requires the `notebook` subcommand (`nlm query notebook <id> <question>`); passing the notebook ID directly fails.
- Long responses are truncated in the JSON output. Follow-up queries using `--conversation-id` were necessary to retrieve multi-clause answers in pieces.
- Despite these interface quirks, the tool authenticated and returned structured answers with source IDs and inline citations reliably.
