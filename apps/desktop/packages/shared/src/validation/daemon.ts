import type {
  AgentRuntimeSnapshot,
  AuthProfileLoginResult,
  AuthProfileLogoutResult,
  DaemonAgentRuntimeAuthLoginParams,
  DaemonAgentRuntimeAuthLogoutParams,
  DaemonAgentRuntimePatchProfileParams,
  DaemonAgentRuntimeSelectProfileParams,
  DaemonAgentRuntimeSetExtensionEnabledParams,
  DaemonControlStatusResult,
  DaemonDiagnostics,
  DaemonInitializeResult,
  DaemonSessionAttachResult,
  DaemonSessionOpenParams,
  DaemonSessionOpenResult,
  DaemonStatusResult,
  DaemonSubscribeResult,
} from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import {
  ajv,
  formatProtocolValidationErrors,
  parseProtocolBigInt,
  parseNullableBoundaryValue,
  parseSchema,
  ProtocolValidationError,
} from "./core.js";
import { parseDaemonEventCursor } from "./cursors.js";
import { parseSessionAuthority } from "./identity.js";
import { parseSessionSummary } from "./summaries.js";

const validateDaemonInitializeResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.DaemonInitializeResult);
const validateDaemonSessionOpenParams = ajv.compile(PROTOCOL_JSON_SCHEMAS.DaemonSessionOpenParams);
const validateDaemonSessionOpenResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.DaemonSessionOpenResult);
const validateDaemonSessionAttachResult = ajv.compile(
  PROTOCOL_JSON_SCHEMAS.DaemonSessionAttachResult,
);
const validateDaemonControlStatusResult = ajv.compile(
  PROTOCOL_JSON_SCHEMAS.DaemonControlStatusResult,
);
const validateDaemonDiagnostics = ajv.compile(PROTOCOL_JSON_SCHEMAS.DaemonDiagnostics);
const validateAgentRuntimeSnapshot = ajv.compile(PROTOCOL_JSON_SCHEMAS.AgentRuntimeSnapshot);
const validateDaemonAgentRuntimeSelectProfileParams = ajv.compile(
  PROTOCOL_JSON_SCHEMAS.DaemonAgentRuntimeSelectProfileParams,
);
const validateDaemonAgentRuntimePatchProfileParams = ajv.compile(
  PROTOCOL_JSON_SCHEMAS.DaemonAgentRuntimePatchProfileParams,
);
const validateDaemonAgentRuntimeAuthLoginParams = ajv.compile(
  PROTOCOL_JSON_SCHEMAS.DaemonAgentRuntimeAuthLoginParams,
);
const validateDaemonAgentRuntimeAuthLogoutParams = ajv.compile(
  PROTOCOL_JSON_SCHEMAS.DaemonAgentRuntimeAuthLogoutParams,
);
const validateDaemonAgentRuntimeSetExtensionEnabledParams = ajv.compile(
  PROTOCOL_JSON_SCHEMAS.DaemonAgentRuntimeSetExtensionEnabledParams,
);
const validateAuthProfileLoginResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.AuthProfileLoginResult);
const validateAuthProfileLogoutResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.AuthProfileLogoutResult);
const validateDaemonStatusResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.DaemonStatusResult);
const validateDaemonSubscribeResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.DaemonSubscribeResult);

type DaemonDiagnosticErrorWire = Omit<DaemonDiagnostics["recentErrors"][number], "occurredAtMs"> & {
  occurredAtMs: string;
};
type DaemonDiagnosticTokenUsageWire = Omit<
  DaemonDiagnostics["tokenUsage"],
  | "cachedTokens"
  | "completionTokens"
  | "modelContextWindow"
  | "promptTokens"
  | "reasoningTokens"
  | "totalTokens"
> & {
  cachedTokens?: string | null;
  completionTokens?: string | null;
  modelContextWindow?: string | null;
  promptTokens?: string | null;
  reasoningTokens?: string | null;
  totalTokens?: string | null;
};
type DaemonDiagnosticsWire = Omit<DaemonDiagnostics, "recentErrors" | "tokenUsage" | "uptimeMs"> & {
  recentErrors: DaemonDiagnosticErrorWire[];
  tokenUsage: DaemonDiagnosticTokenUsageWire;
  uptimeMs: string;
};

export function parseDaemonInitializeResult(value: unknown): DaemonInitializeResult {
  if (validateDaemonInitializeResult(value)) {
    return value as DaemonInitializeResult;
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("DaemonInitializeResult", validateDaemonInitializeResult.errors),
  );
}

export function parseDaemonSessionOpenParams(value: unknown): DaemonSessionOpenParams {
  return parseSchema<DaemonSessionOpenParams>(
    "DaemonSessionOpenParams",
    validateDaemonSessionOpenParams,
    value,
  );
}

export function parseDaemonSessionAttachResult(value: unknown): DaemonSessionAttachResult {
  if (validateDaemonSessionAttachResult(value)) {
    const record = value as {
      session: unknown;
      latestCursor?: unknown;
      sessionAuthority: unknown;
    };
    return {
      session: parseSessionSummary(record.session),
      latestCursor: parseNullableBoundaryValue(record.latestCursor, (latestCursor) =>
        parseDaemonEventCursor(latestCursor, "DaemonSessionAttachResult.latestCursor"),
      ),
      sessionAuthority: parseSessionAuthority(record.sessionAuthority),
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors(
      "DaemonSessionAttachResult",
      validateDaemonSessionAttachResult.errors,
    ),
  );
}

export function parseDaemonSessionOpenResult(value: unknown): DaemonSessionOpenResult {
  if (validateDaemonSessionOpenResult(value)) {
    const record = value as {
      session: unknown;
      latestCursor?: unknown;
      sessionAuthority: unknown;
    };
    return {
      session: parseSessionSummary(record.session),
      latestCursor: parseNullableBoundaryValue(record.latestCursor, (latestCursor) =>
        parseDaemonEventCursor(latestCursor, "DaemonSessionOpenResult.latestCursor"),
      ),
      sessionAuthority: parseSessionAuthority(record.sessionAuthority),
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors(
      "DaemonSessionOpenResult",
      validateDaemonSessionOpenResult.errors,
    ),
  );
}

export function parseDaemonControlStatusResult(value: unknown): DaemonControlStatusResult {
  if (validateDaemonControlStatusResult(value)) {
    return value as DaemonControlStatusResult;
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors(
      "DaemonControlStatusResult",
      validateDaemonControlStatusResult.errors,
    ),
  );
}

export function parseDaemonDiagnostics(value: unknown): DaemonDiagnostics {
  if (validateDaemonDiagnostics(value)) {
    const record = value as DaemonDiagnosticsWire;
    return {
      ...record,
      recentErrors: record.recentErrors.map((error) => ({
        ...error,
        occurredAtMs: parseProtocolBigInt(
          error.occurredAtMs,
          "DaemonDiagnostics.recentErrors.occurredAtMs",
        ),
      })),
      tokenUsage: parseDaemonDiagnosticTokenUsage(record.tokenUsage),
      uptimeMs: parseProtocolBigInt(record.uptimeMs, "DaemonDiagnostics.uptimeMs"),
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("DaemonDiagnostics", validateDaemonDiagnostics.errors),
  );
}

export function parseAgentRuntimeSnapshot(value: unknown): AgentRuntimeSnapshot {
  return parseSchema<AgentRuntimeSnapshot>(
    "AgentRuntimeSnapshot",
    validateAgentRuntimeSnapshot,
    value,
  );
}

export function parseDaemonAgentRuntimeSelectProfileParams(
  value: unknown,
): DaemonAgentRuntimeSelectProfileParams {
  return parseSchema<DaemonAgentRuntimeSelectProfileParams>(
    "DaemonAgentRuntimeSelectProfileParams",
    validateDaemonAgentRuntimeSelectProfileParams,
    value,
  );
}

export function parseDaemonAgentRuntimePatchProfileParams(
  value: unknown,
): DaemonAgentRuntimePatchProfileParams {
  return parseSchema<DaemonAgentRuntimePatchProfileParams>(
    "DaemonAgentRuntimePatchProfileParams",
    validateDaemonAgentRuntimePatchProfileParams,
    value,
  );
}

export function parseDaemonAgentRuntimeAuthLoginParams(
  value: unknown,
): DaemonAgentRuntimeAuthLoginParams {
  return parseSchema<DaemonAgentRuntimeAuthLoginParams>(
    "DaemonAgentRuntimeAuthLoginParams",
    validateDaemonAgentRuntimeAuthLoginParams,
    value,
  );
}

export function parseDaemonAgentRuntimeAuthLogoutParams(
  value: unknown,
): DaemonAgentRuntimeAuthLogoutParams {
  return parseSchema<DaemonAgentRuntimeAuthLogoutParams>(
    "DaemonAgentRuntimeAuthLogoutParams",
    validateDaemonAgentRuntimeAuthLogoutParams,
    value,
  );
}

export function parseDaemonAgentRuntimeSetExtensionEnabledParams(
  value: unknown,
): DaemonAgentRuntimeSetExtensionEnabledParams {
  return parseSchema<DaemonAgentRuntimeSetExtensionEnabledParams>(
    "DaemonAgentRuntimeSetExtensionEnabledParams",
    validateDaemonAgentRuntimeSetExtensionEnabledParams,
    value,
  );
}

export function parseAuthProfileLoginResult(value: unknown): AuthProfileLoginResult {
  return parseSchema<AuthProfileLoginResult>(
    "AuthProfileLoginResult",
    validateAuthProfileLoginResult,
    value,
  );
}

export function parseAuthProfileLogoutResult(value: unknown): AuthProfileLogoutResult {
  return parseSchema<AuthProfileLogoutResult>(
    "AuthProfileLogoutResult",
    validateAuthProfileLogoutResult,
    value,
  );
}

export function parseDaemonStatusResult(value: unknown): DaemonStatusResult {
  if (validateDaemonStatusResult(value)) {
    return value as DaemonStatusResult;
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("DaemonStatusResult", validateDaemonStatusResult.errors),
  );
}

export function parseDaemonSubscribeResult(value: unknown): DaemonSubscribeResult {
  if (validateDaemonSubscribeResult(value)) {
    const record = value as {
      status: "ready" | "historyGap";
      latestCursor?: unknown;
    };
    return {
      status: record.status,
      latestCursor: parseNullableBoundaryValue(record.latestCursor, (latestCursor) =>
        parseDaemonEventCursor(latestCursor, "DaemonSubscribeResult.latestCursor"),
      ),
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("DaemonSubscribeResult", validateDaemonSubscribeResult.errors),
  );
}

function parseOptionalProtocolBigInt(
  value: string | null | undefined,
  fieldName: string,
): bigint | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return parseProtocolBigInt(value, fieldName);
}

function parseDaemonDiagnosticTokenUsage(
  value: DaemonDiagnosticTokenUsageWire,
): DaemonDiagnostics["tokenUsage"] {
  const tokenUsage: DaemonDiagnostics["tokenUsage"] = {};
  const modelContextWindow = parseOptionalProtocolBigInt(
    value.modelContextWindow,
    "DaemonDiagnostics.tokenUsage.modelContextWindow",
  );
  if (modelContextWindow !== undefined) {
    tokenUsage.modelContextWindow = modelContextWindow;
  }
  const totalTokens = parseOptionalProtocolBigInt(
    value.totalTokens,
    "DaemonDiagnostics.tokenUsage.totalTokens",
  );
  if (totalTokens !== undefined) {
    tokenUsage.totalTokens = totalTokens;
  }
  const promptTokens = parseOptionalProtocolBigInt(
    value.promptTokens,
    "DaemonDiagnostics.tokenUsage.promptTokens",
  );
  if (promptTokens !== undefined) {
    tokenUsage.promptTokens = promptTokens;
  }
  const completionTokens = parseOptionalProtocolBigInt(
    value.completionTokens,
    "DaemonDiagnostics.tokenUsage.completionTokens",
  );
  if (completionTokens !== undefined) {
    tokenUsage.completionTokens = completionTokens;
  }
  const cachedTokens = parseOptionalProtocolBigInt(
    value.cachedTokens,
    "DaemonDiagnostics.tokenUsage.cachedTokens",
  );
  if (cachedTokens !== undefined) {
    tokenUsage.cachedTokens = cachedTokens;
  }
  const reasoningTokens = parseOptionalProtocolBigInt(
    value.reasoningTokens,
    "DaemonDiagnostics.tokenUsage.reasoningTokens",
  );
  if (reasoningTokens !== undefined) {
    tokenUsage.reasoningTokens = reasoningTokens;
  }
  return tokenUsage;
}
