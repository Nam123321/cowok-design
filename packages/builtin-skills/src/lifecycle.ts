// @story 08.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
/** Strangler-Fig facade preserving backwards-compatible lifecycle exports while isolating cohesion cleanly. */
export { PluginStateMachine } from "./plugin-state-machine";
export { HumanSessionAuthority, type MintTokenInput } from "./learn-authority";
export {
  LearnPipeline,
  MAX_LEARN_PROMPT_TOKENS,
  type GateReportInput,
  type PromptInjectionResult,
  type ProposeCardInput,
} from "./learn-pipeline";
