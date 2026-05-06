import type {
  ListNativeRunsRequest,
  ListNativeRunsResult,
  ForkRunRequest,
  ForkRunResult,
  RunDetail,
  RunEventStreamItem,
  RunId,
  RunTimeline,
  StartRunCommand,
  SubscribeRunEventsResult,
} from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import { ajv, parseNonEmptyProtocolString, parseSchema } from "./core.js";
export { parseRunSummary, parseRunSummaryList } from "./summaries.js";

const validateRunId = ajv.compile(PROTOCOL_JSON_SCHEMAS.RunId);
const validateStartRunCommand = ajv.compile(PROTOCOL_JSON_SCHEMAS.StartRunCommand);
const validateListNativeRunsRequest = ajv.compile(PROTOCOL_JSON_SCHEMAS.ListNativeRunsRequest);
const validateListNativeRunsResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.ListNativeRunsResult);
const validateForkRunRequest = ajv.compile(PROTOCOL_JSON_SCHEMAS.ForkRunRequest);
const validateForkRunResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.ForkRunResult);
const validateRunDetail = ajv.compile(PROTOCOL_JSON_SCHEMAS.RunDetail);
const validateRunTimeline = ajv.compile(PROTOCOL_JSON_SCHEMAS.RunTimeline);
const validateSubscribeRunEventsResult = ajv.compile(
  PROTOCOL_JSON_SCHEMAS.SubscribeRunEventsResult,
);
const validateRunEventStreamItem = ajv.compile(PROTOCOL_JSON_SCHEMAS.RunEventStreamItem);

export function parseRunId(value: unknown): RunId {
  const runId = parseSchema<RunId>("RunId", validateRunId, value);
  return parseNonEmptyProtocolString(runId, "RunId") as RunId;
}

export function parseStartRunCommand(value: unknown): StartRunCommand {
  return parseSchema<StartRunCommand>("StartRunCommand", validateStartRunCommand, value);
}

export function parseListNativeRunsRequest(value: unknown): ListNativeRunsRequest {
  return parseSchema<ListNativeRunsRequest>(
    "ListNativeRunsRequest",
    validateListNativeRunsRequest,
    value,
  );
}

export function parseListNativeRunsResult(value: unknown): ListNativeRunsResult {
  return parseSchema<ListNativeRunsResult>(
    "ListNativeRunsResult",
    validateListNativeRunsResult,
    value,
  );
}

export function parseForkRunRequest(value: unknown): ForkRunRequest {
  return parseSchema<ForkRunRequest>("ForkRunRequest", validateForkRunRequest, value);
}

export function parseForkRunResult(value: unknown): ForkRunResult {
  return parseSchema<ForkRunResult>("ForkRunResult", validateForkRunResult, value);
}

export function parseRunDetail(value: unknown): RunDetail {
  return parseSchema<RunDetail>("RunDetail", validateRunDetail, value);
}

export function parseRunTimeline(value: unknown): RunTimeline {
  return parseSchema<RunTimeline>("RunTimeline", validateRunTimeline, value);
}

export function parseSubscribeRunEventsResult(value: unknown): SubscribeRunEventsResult {
  return parseSchema<SubscribeRunEventsResult>(
    "SubscribeRunEventsResult",
    validateSubscribeRunEventsResult,
    value,
  );
}

export function parseRunEventStreamItem(value: unknown): RunEventStreamItem {
  return parseSchema<RunEventStreamItem>("RunEventStreamItem", validateRunEventStreamItem, value);
}
