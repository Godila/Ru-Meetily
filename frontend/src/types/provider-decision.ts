/**
 * Discriminated union for the LLM-provider decision the user makes during
 * onboarding. Mirrors the Rust `OnboardingProviderChoice` enum
 * (`onboarding.rs`) — both serialise as `{kind: "local"|"cloud"|"skip", ...}`.
 *
 * The `kind` tag forces every consumer to handle all three branches explicitly,
 * which the compiler enforces in both TypeScript (narrowing) and Rust (match).
 *
 * NOTE: Ollama is deliberately excluded from CLOUD_PROVIDERS — it runs a local
 * server, so in onboarding terms it behaves like a local choice. Users who want
 * Ollama should pick Skip and configure it in Settings afterwards.
 */

/** Cloud providers offered in onboarding, in display order.
 *  Caila is first because Convoic targets the Russian market. */
export const CLOUD_PROVIDERS = [
  'caila',
  'openai',
  'claude',
  'openrouter',
  'custom-openai',
] as const;
export type CloudProviderId = (typeof CLOUD_PROVIDERS)[number];

export type SummaryProviderDecision =
  | { readonly kind: 'local'; readonly model: string }
  | {
      readonly kind: 'cloud';
      readonly provider: CloudProviderId;
      readonly apiKey: string;
      readonly model: string;
    }
  | { readonly kind: 'deferred'; readonly reason: 'user_skipped' };

/** Wire format sent to the Rust `complete_onboarding` command. */
export interface LocalChoicePayload {
  kind: 'local';
  model: string;
}
export interface CloudChoicePayload {
  kind: 'cloud';
  provider: string;
  api_key: string | null;
  model: string;
}
export interface SkipChoicePayload {
  kind: 'skip';
}
export type OnboardingProviderChoicePayload =
  | LocalChoicePayload
  | CloudChoicePayload
  | SkipChoicePayload;

/** Returns true when the decision can drive a summary generation right away.
 *  `null` (no decision yet) and `deferred` both return false. */
export function isActionable(
  decision: SummaryProviderDecision | null
): decision is Extract<SummaryProviderDecision, { kind: 'local' | 'cloud' }> {
  if (decision === null) return false;
  return decision.kind === 'local' || decision.kind === 'cloud';
}

/** Converts the frontend decision into the exact wire shape the Rust command
 *  expects (snake_case fields for api_key, tag value "skip" not "deferred"). */
export function toBackendPayload(
  decision: SummaryProviderDecision
): OnboardingProviderChoicePayload {
  switch (decision.kind) {
    case 'local':
      return { kind: 'local', model: decision.model };
    case 'cloud':
      return {
        kind: 'cloud',
        provider: decision.provider,
        api_key: decision.apiKey,
        model: decision.model,
      };
    case 'deferred':
      return { kind: 'skip' };
  }
}

/** Parses the onboarding-status marker string back into a decision, or null.
 *  Marker values: "local" | "cloud:<provider>" | "deferred" | undefined. */
export function decisionFromStatusMarker(
  marker: string | null | undefined,
  fallbackModel: string
): SummaryProviderDecision | null {
  if (!marker) return null;
  if (marker === 'local') return { kind: 'local', model: fallbackModel };
  if (marker === 'deferred')
    return { kind: 'deferred', reason: 'user_skipped' };
  if (marker.startsWith('cloud:')) {
    const provider = marker.slice('cloud:'.length);
    return {
      kind: 'cloud',
      provider: provider as CloudProviderId,
      apiKey: '',
      model: '',
    };
  }
  return null;
}
