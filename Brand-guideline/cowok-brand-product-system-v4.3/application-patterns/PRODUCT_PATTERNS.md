# cowok.ai Product / Application Patterns v1.0

## Purpose

This layer defines how foundations and components become complete product experiences.

Dependencies:
- Visual Foundations v1.1
- Iconography v2.0
- Core Components v1.0

---

## Global product principles

### 1. Workflow first
The screen should answer:
- What is happening?
- What needs attention?
- Who owns the next step?
- What is Human vs Agent?
- What changed?

### 2. Human authority
High-impact actions expose review, impact and provenance.

### 3. Agent visibility without AI theater
Agents are shown as operational actors:
- role
- current work
- status
- output
- tools/context where useful

Do not make every screen look like a chatbot.

### 4. Quiet by default
Most surfaces remain neutral.

Color is reserved for:
- active Human ownership
- active Agent execution
- review-required
- semantic status
- selected state

### 5. Long-session hierarchy
Information hierarchy should come from:
1. layout
2. typography
3. spacing
4. surface hierarchy
5. border/elevation
6. color last

---

## Pattern 01 — Dashboard

Purpose:
Operational overview.

Recommended modules:
- needs-attention first
- active work
- Human/Agent activity
- KPI summary
- recent work

Avoid:
- colorful KPI tiles everywhere
- AI statistics as vanity metrics
- dashboard-as-control-room visual noise

---

## Pattern 02 — Project

Purpose:
One shared outcome for Human + Agent participants.

Recommended hierarchy:
1. Outcome / status
2. current milestone
3. next action
4. Human members / Agents
5. workstream
6. history

Agent should look like a participant, not a floating AI assistant.

---

## Pattern 03 — Workflow / Kanban

Purpose:
Make work state and handoff explicit.

Canonical flow:
`Request → Interpret → Draft → Review → Execute → Report`

Use Kanban only when state movement is meaningful.

Do not color whole cards by owner.
Use:
- small Human/Agent label
- edge accent
- semantic state label

---

## Pattern 04 — Agent Workspace

Purpose:
Give transparency into Agent execution.

Show:
- role
- task
- working context
- output
- relevant tool/action history
- confidence/uncertainty where meaningful
- review requirement

Do not expose raw chain-of-thought.
Show operational explanation/provenance instead.

---

## Pattern 05 — Knowledge / Wiki

Purpose:
Trusted enterprise context.

Every knowledge result should make source quality understandable:
- source
- owner
- updated date
- linked workflow/project
- trust/verification state

---

## Pattern 06 — Review & Approval

Review screen answers:
1. What is proposed?
2. Why?
3. What changes?
4. What could happen?
5. What evidence supports it?
6. What can the Human do?

Agent orange is an accent only.
The full panel remains neutral.

---

## Pattern 07 — Automation

Automation should be quiet when healthy.

Emphasize:
- exceptions
- failed run
- review required
- external action
- financial/high-impact change

Do not make every successful run noisy.

---

## Responsive behavior

### Desktop
- persistent navigation
- multi-column workspaces
- contextual side panels
- tables where density matters

### Tablet
- collapsible navigation
- 1–2 column layout
- details moved into drawer

### Mobile
Primary goals:
- review
- approve
- check status
- comment
- capture quick input

Do not force desktop administration patterns onto mobile.

---

## Theme behavior

Warm Light:
primary daytime theme.

Focus Light:
softer light environment for long reading/work.

Night:
Deep Warm Navy / Charcoal Navy.

Low-chroma Focus:
available only in Night.
