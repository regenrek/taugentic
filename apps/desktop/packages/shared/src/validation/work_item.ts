import type {
  WorkItemDismissParams,
  WorkItemDismissResult,
  WorkItemListResult,
  WorkItemRefreshParams,
  WorkItemTriggerParams,
  WorkItemTriggerResult,
} from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import { ajv, parseSchema } from "./core.js";

const validateWorkItemListResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.WorkItemListResult);
const validateWorkItemRefreshParams = ajv.compile(PROTOCOL_JSON_SCHEMAS.WorkItemRefreshParams);
const validateWorkItemDismissParams = ajv.compile(PROTOCOL_JSON_SCHEMAS.WorkItemDismissParams);
const validateWorkItemDismissResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.WorkItemDismissResult);
const validateWorkItemTriggerParams = ajv.compile(PROTOCOL_JSON_SCHEMAS.WorkItemTriggerParams);
const validateWorkItemTriggerResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.WorkItemTriggerResult);

export function parseWorkItemListResult(value: unknown): WorkItemListResult {
  return parseSchema<WorkItemListResult>("WorkItemListResult", validateWorkItemListResult, value);
}

export function parseWorkItemRefreshParams(value: unknown): WorkItemRefreshParams {
  return parseSchema<WorkItemRefreshParams>(
    "WorkItemRefreshParams",
    validateWorkItemRefreshParams,
    value,
  );
}

export function parseWorkItemDismissParams(value: unknown): WorkItemDismissParams {
  return parseSchema<WorkItemDismissParams>(
    "WorkItemDismissParams",
    validateWorkItemDismissParams,
    value,
  );
}

export function parseWorkItemDismissResult(value: unknown): WorkItemDismissResult {
  return parseSchema<WorkItemDismissResult>(
    "WorkItemDismissResult",
    validateWorkItemDismissResult,
    value,
  );
}

export function parseWorkItemTriggerParams(value: unknown): WorkItemTriggerParams {
  return parseSchema<WorkItemTriggerParams>(
    "WorkItemTriggerParams",
    validateWorkItemTriggerParams,
    value,
  );
}

export function parseWorkItemTriggerResult(value: unknown): WorkItemTriggerResult {
  return parseSchema<WorkItemTriggerResult>(
    "WorkItemTriggerResult",
    validateWorkItemTriggerResult,
    value,
  );
}
