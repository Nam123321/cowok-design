# cowok.ai AI UX System v1.0

## Purpose

AI in cowok.ai is an operational actor inside workflow, not a standalone chatbot.

The system must make clear:

- what the Agent is doing;
- why it is doing it;
- what context it is using;
- what needs Human judgment;
- what has actually changed;
- what remains only a proposal.

Canonical flow:

`Request → Interpret → Draft → Review → Execute → Report`

---

# 1. Agent states

## Idle
Available, no active execution.

## Interpreting
Resolving user intent, workflow context and constraints.

## Working
Executing internal work.

## Draft ready
Output exists but is not necessarily approved or executed.

## Review required
Human decision is needed.

## Executing
Approved action is in progress.

## Completed
Execution/output finished.

## Failed
The requested execution did not complete.

## Blocked
Missing permission, context, dependency or policy approval.

---

# 2. Human authority

cowok.ai should distinguish three things explicitly:

1. **Agent output**
2. **Human-approved decision**
3. **Executed change**

These states must never be visually conflated.

Example:

`Draft created` is not the same as `Approved`.

`Approved` is not the same as `Executed`.

---

# 3. Risk model

## R0 — Informational
No side effect.

Examples:
- summarize
- research
- classify
- draft

Default review:
none.

## R1 — Low-impact reversible
Internal, easy to reverse.

Examples:
- tag record
- update internal draft
- reorder task

Default review:
optional depending on policy.

## R2 — Operational change
Meaningful internal operational effect.

Examples:
- update CRM record
- modify workflow
- change scheduled operation

Default review:
Human approval required.

## R3 — External / financial / bulk
High-impact or outward-facing.

Examples:
- send communication
- payment
- financial action
- bulk update
- destructive action

Default review:
explicit Human confirmation + impact summary.

---

# 4. Review surface

A review screen answers:

1. What is proposed?
2. Why?
3. What will change?
4. What could happen?
5. What evidence supports it?
6. What can the Human do?

Actions:
- Approve
- Edit
- Reject
- Ask for more context

Agent Orange may identify Agent-specific action, but the review container remains neutral.

---

# 5. Provenance

Show operational provenance when it helps decision quality:

- source
- owner
- freshness
- tool/action used
- related project/workflow
- timestamp
- external effect/result

Do not expose private chain-of-thought.

Use concise operational explanation:

> Considered A and B. Source X was updated 3h ago. Proposal changes 6 partner actions. Two assumptions remain uncertain.

---

# 6. Supervisor environment

Supervisor is a control layer, not another chat persona.

Checks:
- risk
- permissions
- approval requirement
- source/data validity
- workflow stage
- execution result
- exception

Healthy execution:
Supervisor stays quiet.

Problem:
Surface only the decision or intervention required.

---

# 7. Automation behavior

Healthy automation should fade into background.

Surface prominently only:
- review required
- failure
- blocked dependency
- external/high-impact action
- unusual deviation

Successful recurring runs should not compete for attention.

---

# 8. Visual hierarchy

Use:

- neutral surface for Agent activity
- Agent accent for active/review-required semantics
- Human accent for Human ownership/action
- status tokens for success/error/etc.

Do not:
- glow Agent panels
- use orange for all AI content
- use gradients for workflow state
- make every Agent update visually loud

---

# 9. Long-session behavior

Warm Light:
normal semantic emphasis.

Focus Light:
normal semantic emphasis.

Night:
normal semantic emphasis.

Night + Low-chroma:
mute inactive Agent/Human/status accents while preserving:
- current execution
- review-required
- critical error
- active Human decision

---

# 10. UX language

Prefer:

`Campaign Agent prepared a proposal.`

`Review 6 proposed changes.`

`3 sources are outdated.`

Avoid:

`The AI is thinking...`

`AI magic completed!`

`AI has intelligently decided...`

The system should sound operational, calm and accountable.
