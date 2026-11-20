# Edge Case Guardian & Adversarial Review Report — Story 08.5

- **Story:** `Story 08.5` — Split DE+DR Plugin Runtime, 7 Built-in DE Tools, Controlled Ephemeral Tool Jail & System-1 `LEARN` Loop
- **Epic:** `Epic 08` (`agentskills.io` Repackager, Split DE+DR Runtime, 7 Built-in DE Tools, Controlled Tool Jail & `LEARN`)
- **Reviewer:** Review Agent (`573b2485-b673-4d45-a4ca-014bf9e375c3`, Edge Case Guardian Skill)
- **Risk Tier:** `3` (`CS = 6 < 7`)
- **Initial Review Findings & Remediation Applied (`2026-11-20`):**
  1. Added missing `"02.4"` dependency (`CachePreservingSkillRouter`, `V16c user_skill_registry` slot) and switched `consumes_apis` to project-scoped `POST /api/v1/projects/:id/learn/cards` (`08.4` parity).
  2. Hardened Ephemeral Tool Jail with `setpgid(0,0)` + `kill(-pgid, SIGKILL)`, `RLIMIT_NPROC`/`RLIMIT_AS`, network isolation, `env_clear()`, `O_NOFOLLOW`, and unconditional `ActorKind::HumanWorkbenchSession` (`HUMAN_ONLY`).
  3. Added statistical hysteresis (`promote >= 0.95`, `auto-retire < 0.85`), `1,000`-card cap, BFS `visited` + `MAX_DEPTH` + `O_NOFOLLOW` in `get_artifact`, `V18a` ledger slot, and Formal-Verify Candidate.
- **FMEA Binding:** `EC-E08-11` (`RPN 250`), `EC-E08-12` (`RPN 225`) -> `MITIGATED_IN_SPEC`.

## Final Verdict
**VERDICT: PASS / APPROVED**
