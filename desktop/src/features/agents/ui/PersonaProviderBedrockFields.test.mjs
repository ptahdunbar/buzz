import assert from "node:assert/strict";
import test from "node:test";

import {
  BEDROCK_OWNED_ENV_KEYS,
  credentialSourceFromEnvVars,
} from "./PersonaProviderBedrockFields.tsx";

// ── BEDROCK_OWNED_ENV_KEYS ─────────────────────────────────────────────────

test("BEDROCK_OWNED_ENV_KEYS lists exactly the four AWS credential keys", () => {
  assert.deepEqual(
    [...BEDROCK_OWNED_ENV_KEYS].sort(),
    [
      "AWS_ACCESS_KEY_ID",
      "AWS_PROFILE",
      "AWS_REGION",
      "AWS_SECRET_ACCESS_KEY",
    ].sort(),
  );
});

// ── credentialSourceFromEnvVars ────────────────────────────────────────────

test("credentialSourceFromEnvVars defaults to environment when nothing is set", () => {
  assert.equal(credentialSourceFromEnvVars({}), "environment");
});

test("credentialSourceFromEnvVars defaults to environment when region is set but no credential is", () => {
  assert.equal(
    credentialSourceFromEnvVars({ AWS_REGION: "us-east-1" }),
    "environment",
  );
});

test("credentialSourceFromEnvVars returns profile when AWS_PROFILE is set", () => {
  assert.equal(
    credentialSourceFromEnvVars({ AWS_PROFILE: "buzz-bedrock" }),
    "profile",
  );
});

test("credentialSourceFromEnvVars returns keys when AWS_ACCESS_KEY_ID is set", () => {
  assert.equal(
    credentialSourceFromEnvVars({ AWS_ACCESS_KEY_ID: "AKIDEXAMPLE" }),
    "keys",
  );
});

test("credentialSourceFromEnvVars prefers keys over profile when both are set", () => {
  // Mirrors aws-config's own precedence: static env-var credentials are
  // tried before the profile provider in the default chain.
  assert.equal(
    credentialSourceFromEnvVars({
      AWS_ACCESS_KEY_ID: "AKIDEXAMPLE",
      AWS_PROFILE: "buzz-bedrock",
    }),
    "keys",
  );
});

test("credentialSourceFromEnvVars treats an empty-string value as not set", () => {
  assert.equal(
    credentialSourceFromEnvVars({ AWS_PROFILE: "", AWS_ACCESS_KEY_ID: "" }),
    "environment",
  );
});
