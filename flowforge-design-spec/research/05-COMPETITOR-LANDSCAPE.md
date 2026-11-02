# Competitive Landscape — Research snapshot 2026-10-26

**Method:** official vendor sites/docs and publicly visible source repositories. Features change quickly; compare exact plans/releases before launch. "Not evidenced" means not substantiated by checked sources, NOT proven absent.

## Direct competitors
| Player | Verified strengths | Strategic implication | Sources |
|---|---|---|---|
| Onlook | Visual editor over real code, iframe/runtime, code-element instrumentation, comments, AI, repo import; public open-source architecture | Highest engineering overlap; investigate reuse/license and differentiate via flow intelligence | [O1], [O2] |
| pen.dev (formerly Pencil) | Design document + MCP integrations including Codex CLI and Antigravity; public tools and AI clients | MCP-to-agent and design workflows cannot be standalone moat | [P1], [P2] |
| Figma Make | Limited beta linking real local/GitHub codebase with annotations, visual edit, commits/PR | Incumbent expanding to code-native; avoid “Figma is static only” narrative | [F1], [F2] |
| Vercel v0 | Live preview design-mode element selection, property edits, instructions applied back to code | Prompt+visual edit+code already established | [V1] |

## Adjacent substitutes requiring dedicated follow-up validation
- Lovable, Bolt.new: prompt-to-app / code-building workflows, not necessarily flow graph as primary UX.
- Framer, Webflow, Readdy: web visual creation, delivery and hosting; compare export constraints.
- OpenDesign: design references/critique and MCP-based workflows; clarify product/version before detailed score.

## Feature-level opportunity hypotheses (not absence claims)
1. **State-aware executable flow**: flow node resolves real runtime state, not merely click prototype.
2. **Visual clarification for lay requirements**: controlled, testable spec generation rather than simply a better prompt.
3. **Requirement-to-code traceability**: accepted decisions linked to tests, source changes and comments.
4. **Flow coverage evaluation**: quantifying omissions and broken states.

## Competitive risk
Large incumbents and open-source tools can copy isolated functions; demand proof of workflow performance. Onlook's open-source code shows a meaningful code-native editor already exists, so deep clone would be poor use of scarce R&D.

## Evaluation matrix to run (not prefilled with invented scores)
Test each product on identical scenario: 9-state B2B order-approval app; generate; edit approval modal based on targeted comment; test failure path; export code; import repo. Score setup time, flow completion, first-pass acceptance, code integration, change precision, token/tool cost and user effort. Document version/date/tier/model per run.

## Source links
[O1] https://github.com/onlook-dev/onlook  
[O2] https://www.onlook.com/faq  
[P1] https://docs.pen.dev/getting-started/installation  
[P2] https://docs.pen.dev/getting-started/ai-integration  
[F1] https://www.figma.com/blog/figma-make-now-on-your-local-code/  
[F2] https://help.figma.com/hc/en-us/articles/40775535020695-Make-in-your-local-codebase  
[V1] https://v0.app/docs/design-mode
