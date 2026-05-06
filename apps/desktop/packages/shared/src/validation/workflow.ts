import type {
  WorkflowLoadParams,
  WorkflowReloadOutcome,
  WorkflowStatusResult,
  WorkflowValidateParams,
  WorkflowValidationReport,
} from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import { ajv, parseProtocolBigInt, parseSchema } from "./core.js";

const validateWorkflowLoadParams = ajv.compile(PROTOCOL_JSON_SCHEMAS.WorkflowLoadParams);
const validateWorkflowValidateParams = ajv.compile(PROTOCOL_JSON_SCHEMAS.WorkflowValidateParams);
const validateWorkflowStatusResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.WorkflowStatusResult);
const validateWorkflowValidationReport = ajv.compile(
  PROTOCOL_JSON_SCHEMAS.WorkflowValidationReport,
);

type WorkflowReloadOutcomeWire =
  | (Omit<Extract<WorkflowReloadOutcome, { status: "reloaded" }>, "version"> & { version: string })
  | Extract<WorkflowReloadOutcome, { status: "failed" }>;

type WorkflowStatusResultWire = Omit<WorkflowStatusResult, "loaded" | "lastReload"> & {
  loaded?:
    | (Omit<NonNullable<WorkflowStatusResult["loaded"]>, "version"> & { version: string })
    | null;
  lastReload?: WorkflowReloadOutcomeWire | null;
};

export function parseWorkflowLoadParams(value: unknown): WorkflowLoadParams {
  return parseSchema<WorkflowLoadParams>("WorkflowLoadParams", validateWorkflowLoadParams, value);
}

export function parseWorkflowValidateParams(value: unknown): WorkflowValidateParams {
  return parseSchema<WorkflowValidateParams>(
    "WorkflowValidateParams",
    validateWorkflowValidateParams,
    value,
  );
}

export function parseWorkflowStatusResult(value: unknown): WorkflowStatusResult {
  const parsed = parseSchema<WorkflowStatusResultWire>(
    "WorkflowStatusResult",
    validateWorkflowStatusResult,
    value,
  );
  return {
    ...parsed,
    loaded: parsed.loaded
      ? {
          ...parsed.loaded,
          version: parseProtocolBigInt(parsed.loaded.version, "WorkflowLoadedStatus.version"),
        }
      : parsed.loaded,
    lastReload: parseWorkflowReloadOutcome(parsed.lastReload),
  };
}

export function parseWorkflowValidationReport(value: unknown): WorkflowValidationReport {
  return parseSchema<WorkflowValidationReport>(
    "WorkflowValidationReport",
    validateWorkflowValidationReport,
    value,
  );
}

function parseWorkflowReloadOutcome(
  value: WorkflowReloadOutcomeWire | null | undefined,
): WorkflowReloadOutcome | null | undefined {
  if (!value || value.status === "failed") {
    return value;
  }
  return {
    ...value,
    version: parseProtocolBigInt(value.version, "WorkflowReloadOutcome.version"),
  };
}
