# Quality Strategy, Benchmark & Safety

## Core hypothesis evaluation
H1 Flow-first UI reduces time to accepted *multi-state* user experience vs screen-first/prompt-to-code.  
H2 Visual clarification decreases rework despite question cost.  
H3 Deterministic code+browser harness improves acceptance/build correctness at sustainable cost on same model.

## Experimental protocol
- Recruit 10–20 users initially; split non-tech product owners and vibe coders.
- Scenario set: B2B order approval, inventory receiving, support ticket triage, SaaS onboarding.
- Keep task text, references, target stack, model version/temperature, budget and reviewer rubric fixed.
- Compare baseline direct model/prompt-to-code, candidate harness without clarification, full harness; randomize ordering.
- Blind reviewers score correct behavior, UI consistency, accessibility basics, code integration and task completion.
- Record user time, clarification burden, tokens, tool spend, attempts, accepted/rejected change, test failures.

## Metrics and formulas
- Time-to-accepted-flow: wall-clock task start to signed-off working scenario.
- First-pass acceptance = accepted without rework / total tasks.
- Flow-state coverage = validated expected states / specified expected states.
- Transition coverage = tested edges / specified edges.
- Export build success = clean exported projects that install+build / attempted exports.
- Change accuracy = edits satisfying target without unauthorized regressions / edits attempted.
- Cost per accepted design = AI+execution variable spend / accepted tasks.
- Clarification utility: prevented incorrect rework vs extra user interaction/latency; compare randomized cohorts.

## Acceptance rubric (0/1/2 each)
1. Flow completeness (including error and cancel), 2. state correctness, 3. visual consistency/tokens, 4. responsive behavior, 5. accessibility basics, 6. code maintainability, 7. minimal patch scope, 8. user requirement traceability. 16 possible, no claims without empirical trial.

## Test pyramid
- Unit: graph constraints, SDI parsing and risk classification, anchor resolver and path validation.
- Contract: API schema, MCP tool inputs, provider adapters and source map formats.
- Integration: preview session, fixture route, comment assets, proposal approval and export.
- E2E Playwright: order approval modal; cancel; submit; network failure/retry; permission-denied.
- Security: tenant isolation, untrusted repo malicious scripts, prompt-injection attachment, zip-slip, XSS, SSRF, rate limits, credentials redaction.
- Visual: per viewport screenshot deltas with tolerances and manual acceptance; anti-flaky font loading.

## Failure policy
Failed tests are presented transparently; do not self-certify. Limit automated patch retries; stop and request user review on repeated errors. Preserve failed logs and previous good commit. Model evaluator is a diagnostic signal, not a substitute for functional tests.

## Rollout
Internal dogfood → invited pilots → paid beta only after reliable export + tenant security. Benchmark against documented vendor versions, not marketing demonstrations.
