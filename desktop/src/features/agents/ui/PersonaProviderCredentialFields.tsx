import { PersonaProviderApiKeyField } from "./PersonaProviderApiKeyField";
import {
  BEDROCK_OWNED_ENV_KEYS,
  PersonaProviderBedrockFields,
} from "./PersonaProviderBedrockFields";

/**
 * Env var keys to hide from the generic Advanced env-var editor because a
 * structured field above already owns them — the Bedrock credential fields
 * or the single-secret API key field. Shared by every provider-picker call
 * site so the hidden-key policy can't drift between them.
 */
export function providerCredentialHiddenEnvKeys(
  effectiveProvider: string,
  topLevelSecretEnvVar: string | null | undefined,
): readonly string[] {
  if (effectiveProvider === "bedrock") return BEDROCK_OWNED_ENV_KEYS;
  return topLevelSecretEnvVar ? [topLevelSecretEnvVar] : [];
}

/** True when `PersonaProviderCredentialFields` would render something for
 * this provider — lets callers skip an empty wrapper element entirely. */
export function hasProviderCredentialFields(
  effectiveProvider: string,
  topLevelSecretEnvVar: string | null | undefined,
): boolean {
  return topLevelSecretEnvVar != null || effectiveProvider === "bedrock";
}

type SecretFieldProps = {
  inheritedLabel: string;
  isInherited: boolean;
  isRequired: boolean;
  secretEnvVar: string;
  value: string;
};

/** Builds the `secretField` prop for `PersonaProviderCredentialFields` from
 * the raw pieces every call site already computes — `undefined` when the
 * effective provider has no single-secret credential. */
export function buildSecretFieldProps(
  topLevelSecretEnvVar: string | null | undefined,
  rest: Omit<SecretFieldProps, "secretEnvVar">,
): SecretFieldProps | undefined {
  return topLevelSecretEnvVar
    ? { ...rest, secretEnvVar: topLevelSecretEnvVar }
    : undefined;
}

/**
 * Renders whichever provider-specific credential field set applies —
 * `PersonaProviderApiKeyField` for a single-secret provider (anthropic,
 * openai), `PersonaProviderBedrockFields` for bedrock's multi-source
 * credential chain, or nothing for providers with no typed credential
 * (databricks' OAuth PKCE, custom providers, etc.).
 *
 * Centralizes the three-way branch that would otherwise be duplicated at
 * every provider-picker call site (AgentDefinitionDialog,
 * AgentInstanceEditDialog, AgentConfigFields all share this registry).
 */
export function PersonaProviderCredentialFields({
  className,
  disabled,
  effectiveProvider,
  envVars,
  isRegionRequired,
  onEnvVarsChange,
  resetKey,
  secretField,
}: {
  /** Wraps the rendered field in a `<div className={className}>` — lets
   *  call sites skip a conditional wrapper of their own. No-op when this
   *  component renders `null`. */
  className?: string;
  disabled: boolean;
  /** The provider whose credential fields should render, if any. */
  effectiveProvider: string;
  /** Current agent-local env vars — used by the Bedrock field set. */
  envVars: Record<string, string>;
  /** True when the Bedrock region field should show the required asterisk. */
  isRegionRequired: boolean;
  onEnvVarsChange: (next: Record<string, string>) => void;
  /** Remounts the Bedrock field set's local selector state when the
   *  underlying agent identity changes (see PersonaProviderBedrockFields'
   *  doc comment) — e.g. `agent.pubkey`, or the persona id in create/edit
   *  dialogs. */
  resetKey: string;
  /** Props for the single-secret-provider field, when `topLevelSecretEnvVar`
   *  is set for the effective provider — build with `buildSecretFieldProps`. */
  secretField?: SecretFieldProps;
}) {
  if (secretField) {
    return (
      <div className={className}>
        <PersonaProviderApiKeyField
          disabled={disabled}
          inheritedLabel={secretField.inheritedLabel}
          isInherited={secretField.isInherited}
          isRequired={secretField.isRequired}
          label={
            effectiveProvider === "anthropic"
              ? "Anthropic API Key"
              : "OpenAI API Key"
          }
          onValueChange={(next) =>
            onEnvVarsChange({ ...envVars, [secretField.secretEnvVar]: next })
          }
          value={secretField.value}
        />
      </div>
    );
  }

  if (effectiveProvider === "bedrock") {
    return (
      <div className={className}>
        <PersonaProviderBedrockFields
          disabled={disabled}
          envVars={envVars}
          isRegionRequired={isRegionRequired}
          key={resetKey}
          onEnvVarsChange={onEnvVarsChange}
        />
      </div>
    );
  }

  return null;
}
