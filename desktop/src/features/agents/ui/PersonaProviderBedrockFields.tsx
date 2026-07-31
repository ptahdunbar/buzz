import * as React from "react";
import { Eye, EyeOff } from "lucide-react";

import { cn } from "@/shared/lib/cn";
import { Input } from "@/shared/ui/input";
import { RequiredFieldLabel } from "./agentConfigControls";
import {
  PERSONA_FIELD_CONTROL_CLASS,
  PERSONA_FIELD_SHELL_CLASS,
} from "./agentConfigOptions";

const AWS_REGION_ENV_VAR = "AWS_REGION";
const AWS_PROFILE_ENV_VAR = "AWS_PROFILE";
const AWS_ACCESS_KEY_ID_ENV_VAR = "AWS_ACCESS_KEY_ID";
const AWS_SECRET_ACCESS_KEY_ENV_VAR = "AWS_SECRET_ACCESS_KEY";

/** Env var keys this component owns — callers pass these to `hiddenEnvKeys`
 * on the generic Advanced env-var editor so they aren't shown twice. */
export const BEDROCK_OWNED_ENV_KEYS: readonly string[] = [
  AWS_REGION_ENV_VAR,
  AWS_PROFILE_ENV_VAR,
  AWS_ACCESS_KEY_ID_ENV_VAR,
  AWS_SECRET_ACCESS_KEY_ENV_VAR,
];

export type BedrockCredentialSource = "environment" | "profile" | "keys";

/** Derives which credential source is already in use from the current env
 * vars — exported for unit testing; components should treat the selector
 * as local UI state seeded from this (see the component doc comment). */
export function credentialSourceFromEnvVars(
  envVars: Record<string, string>,
): BedrockCredentialSource {
  if ((envVars[AWS_ACCESS_KEY_ID_ENV_VAR] ?? "").length > 0) return "keys";
  if ((envVars[AWS_PROFILE_ENV_VAR] ?? "").length > 0) return "profile";
  return "environment";
}

/**
 * AWS Bedrock credential fields: region (required) plus a credential-source
 * selector for the mutually exclusive ways `aws-config`'s default provider
 * chain resolves credentials (see `sigv4::load_aws_credentials`) —
 * an AWS profile, static access keys, or the ambient environment
 * (IAM role via IMDS, an ECS container role, or EKS IRSA — none of which
 * need anything typed here).
 *
 * Unlike `PersonaProviderApiKeyField`, there is no single secret env var:
 * this is a pure view over four keys in the same `envVars` map the parent
 * dialog owns (`AWS_REGION`, `AWS_PROFILE`, `AWS_ACCESS_KEY_ID`,
 * `AWS_SECRET_ACCESS_KEY`) — writes go through `onEnvVarsChange`, so no
 * second copy of any value exists.
 *
 * Only the region is enforced as required here. A bare EC2 instance-profile
 * role has no env footprint at all, so credential presence can't be fully
 * verified client-side — the backend readiness gate documents the same
 * blind spot (readiness.rs). Switching credential source does not clear the
 * other sources' values, so flipping back never loses a typed profile name
 * or key pair — consistent with how Databricks's non-secret fields already
 * behave on provider switch.
 */
export function PersonaProviderBedrockFields({
  disabled,
  envVars,
  isRegionRequired,
  onEnvVarsChange,
}: {
  disabled: boolean;
  /** Current agent-local env vars (region/profile/keys live here). */
  envVars: Record<string, string>;
  /** True when the region field should show the required asterisk. */
  isRegionRequired: boolean;
  onEnvVarsChange: (next: Record<string, string>) => void;
}) {
  const [showSecretKey, setShowSecretKey] = React.useState(false);
  const region = envVars[AWS_REGION_ENV_VAR] ?? "";
  const profile = envVars[AWS_PROFILE_ENV_VAR] ?? "";
  const accessKeyId = envVars[AWS_ACCESS_KEY_ID_ENV_VAR] ?? "";
  const secretAccessKey = envVars[AWS_SECRET_ACCESS_KEY_ENV_VAR] ?? "";

  // The selector is local UI state, not purely derived from envVars: a value
  // typed into a field should switch the selector, but selecting a source
  // with nothing typed yet must still visibly switch — content alone can't
  // distinguish "picked profile, haven't typed a name" from "environment".
  // Seeded once from whichever source already has a value. Callers that
  // reuse this component across different agents (persistent-mount dialogs
  // like AgentInstanceEditDialog) MUST render it with `key={agent.pubkey}`
  // (or similar) so this seed — and showSecretKey above — reset per agent,
  // matching the dialog's own `[open, agent.pubkey]` reset lifecycle.
  const [credentialSource, setCredentialSource] =
    React.useState<BedrockCredentialSource>(() =>
      credentialSourceFromEnvVars(envVars),
    );

  const setEnvVar = (key: string, value: string) => {
    onEnvVarsChange({ ...envVars, [key]: value });
  };

  return (
    <div className="space-y-4">
      <div className="space-y-1.5">
        <RequiredFieldLabel
          htmlFor="persona-provider-bedrock-region"
          isRequired={isRegionRequired}
        >
          AWS Region
        </RequiredFieldLabel>
        <div
          className={cn(
            "flex min-h-11 items-center px-3",
            PERSONA_FIELD_SHELL_CLASS,
          )}
        >
          <Input
            autoComplete="off"
            className={cn(
              "h-8 flex-1 px-0 py-0 leading-6",
              PERSONA_FIELD_CONTROL_CLASS,
            )}
            data-testid="persona-provider-bedrock-region"
            disabled={disabled}
            id="persona-provider-bedrock-region"
            onChange={(event) =>
              setEnvVar(AWS_REGION_ENV_VAR, event.target.value)
            }
            placeholder="us-east-1"
            value={region}
          />
        </div>
      </div>

      <div className="space-y-1.5">
        <span className="text-sm font-medium">AWS Credentials</span>
        <div
          className="flex flex-wrap gap-2"
          data-testid="persona-provider-bedrock-credential-source"
        >
          {(
            [
              { label: "Use environment default", value: "environment" },
              { label: "Use AWS profile", value: "profile" },
              { label: "Use access keys", value: "keys" },
            ] as const
          ).map((option) => (
            <button
              aria-pressed={credentialSource === option.value}
              className={cn(
                "rounded-full border px-3 py-1.5 text-xs font-medium transition-colors",
                credentialSource === option.value
                  ? "border-foreground/30 bg-foreground/10 text-foreground"
                  : "border-input text-muted-foreground hover:text-foreground",
              )}
              disabled={disabled}
              key={option.value}
              onClick={() => setCredentialSource(option.value)}
              type="button"
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>

      {credentialSource === "profile" ? (
        <div className="space-y-1.5">
          <RequiredFieldLabel
            htmlFor="persona-provider-bedrock-profile"
            isRequired={false}
          >
            AWS Profile
          </RequiredFieldLabel>
          <div
            className={cn(
              "flex min-h-11 items-center px-3",
              PERSONA_FIELD_SHELL_CLASS,
            )}
          >
            <Input
              autoComplete="off"
              className={cn(
                "h-8 flex-1 px-0 py-0 leading-6",
                PERSONA_FIELD_CONTROL_CLASS,
              )}
              data-testid="persona-provider-bedrock-profile"
              disabled={disabled}
              id="persona-provider-bedrock-profile"
              onChange={(event) =>
                setEnvVar(AWS_PROFILE_ENV_VAR, event.target.value)
              }
              placeholder="Profile name from ~/.aws/credentials"
              value={profile}
            />
          </div>
        </div>
      ) : null}

      {credentialSource === "keys" ? (
        <>
          <div className="space-y-1.5">
            <RequiredFieldLabel
              htmlFor="persona-provider-bedrock-access-key-id"
              isRequired={false}
            >
              AWS Access Key ID
            </RequiredFieldLabel>
            <div
              className={cn(
                "flex min-h-11 items-center px-3",
                PERSONA_FIELD_SHELL_CLASS,
              )}
            >
              <Input
                autoComplete="off"
                className={cn(
                  "h-8 flex-1 px-0 py-0 leading-6",
                  PERSONA_FIELD_CONTROL_CLASS,
                )}
                data-testid="persona-provider-bedrock-access-key-id"
                disabled={disabled}
                id="persona-provider-bedrock-access-key-id"
                onChange={(event) =>
                  setEnvVar(AWS_ACCESS_KEY_ID_ENV_VAR, event.target.value)
                }
                placeholder="AKIA..."
                value={accessKeyId}
              />
            </div>
          </div>

          <div className="space-y-1.5">
            <RequiredFieldLabel
              htmlFor="persona-provider-bedrock-secret-access-key"
              isRequired={false}
            >
              AWS Secret Access Key
            </RequiredFieldLabel>
            <div
              className={cn(
                "flex min-h-11 items-center gap-2 px-3",
                PERSONA_FIELD_SHELL_CLASS,
              )}
            >
              <Input
                autoComplete="off"
                className={cn(
                  "h-8 flex-1 px-0 py-0 leading-6",
                  PERSONA_FIELD_CONTROL_CLASS,
                )}
                data-testid="persona-provider-bedrock-secret-access-key"
                disabled={disabled}
                id="persona-provider-bedrock-secret-access-key"
                onChange={(event) =>
                  setEnvVar(AWS_SECRET_ACCESS_KEY_ENV_VAR, event.target.value)
                }
                placeholder="Paste secret access key…"
                type={showSecretKey ? "text" : "password"}
                value={secretAccessKey}
              />
              <button
                aria-label={
                  showSecretKey
                    ? "Hide secret access key"
                    : "Show secret access key"
                }
                className="shrink-0 text-muted-foreground hover:text-foreground"
                onClick={() => setShowSecretKey((v) => !v)}
                type="button"
              >
                {showSecretKey ? (
                  <EyeOff className="h-4 w-4" />
                ) : (
                  <Eye className="h-4 w-4" />
                )}
              </button>
            </div>
          </div>
        </>
      ) : null}

      {credentialSource === "environment" ? (
        <p className="text-xs text-muted-foreground">
          Uses an IAM role already available in this environment (EC2 instance
          profile, ECS task role, or EKS IRSA) — nothing to enter.
        </p>
      ) : null}
    </div>
  );
}
