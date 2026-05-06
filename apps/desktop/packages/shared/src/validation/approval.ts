import type {
  ApprovalSnapshotResult,
  DaemonApprovalDecideResult,
  ListApprovalsQuery,
} from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import {
  ajv,
  formatProtocolValidationErrors,
  parseSchema,
  ProtocolValidationError,
} from "./core.js";
import { parseNullableDaemonEventCursor } from "./cursors.js";
import { parseRunSummary } from "./run.js";
import { parseApprovalRequest, parseApprovalRequestList } from "./summaries.js";

const validateListApprovalsQuery = ajv.compile(PROTOCOL_JSON_SCHEMAS.ListApprovalsQuery);
const validateApprovalSnapshotResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.ApprovalSnapshotResult);
const validateDaemonApprovalDecideResult = ajv.compile(
  PROTOCOL_JSON_SCHEMAS.DaemonApprovalDecideResult,
);

export { parseApprovalRequest, parseApprovalRequestList };

export function parseListApprovalsQuery(value: unknown): ListApprovalsQuery {
  return parseSchema<ListApprovalsQuery>("ListApprovalsQuery", validateListApprovalsQuery, value);
}

export function parseDaemonApprovalDecideResult(value: unknown): DaemonApprovalDecideResult {
  if (validateDaemonApprovalDecideResult(value)) {
    const record = value as { run: unknown };
    return {
      run: parseRunSummary(record.run),
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors(
      "DaemonApprovalDecideResult",
      validateDaemonApprovalDecideResult.errors,
    ),
  );
}

export function parseApprovalSnapshotResult(value: unknown): ApprovalSnapshotResult {
  if (validateApprovalSnapshotResult(value)) {
    const record = value as {
      items?: unknown[];
      latestCursor?: unknown;
    };
    return {
      items: (record.items ?? []).map((item) => parseApprovalRequest(item)),
      latestCursor: parseNullableDaemonEventCursor(
        record.latestCursor,
        "ApprovalSnapshotResult.latestCursor",
      ),
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("ApprovalSnapshotResult", validateApprovalSnapshotResult.errors),
  );
}
