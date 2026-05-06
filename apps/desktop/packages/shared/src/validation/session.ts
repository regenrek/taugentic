import type { SessionOverviewQuery, SessionOverviewResult } from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import {
  ajv,
  formatProtocolValidationErrors,
  parseNullableBoundaryValue,
  parseProtocolBigInt,
  parseSchema,
  ProtocolValidationError,
} from "./core.js";
import { parseDaemonEventEnvelope } from "./event.js";
import { parseRunSummary } from "./run.js";
import { parseSessionSummary, parseSessionSummaryList } from "./summaries.js";

const validateSessionOverviewQuery = ajv.compile(PROTOCOL_JSON_SCHEMAS.SessionOverviewQuery);
const validateSessionOverviewResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.SessionOverviewResult);

export { parseSessionSummary, parseSessionSummaryList };

export function parseSessionOverviewQuery(value: unknown): SessionOverviewQuery {
  return parseSchema<SessionOverviewQuery>(
    "SessionOverviewQuery",
    validateSessionOverviewQuery,
    value,
  );
}

export function parseSessionOverviewResult(value: unknown): SessionOverviewResult {
  if (validateSessionOverviewResult(value)) {
    type SessionOverviewItem = NonNullable<SessionOverviewResult["sessions"]>[number];
    const record = value as {
      sessions?: Array<{
        session: unknown;
        latestRun?: unknown;
        laneStatus: SessionOverviewItem["laneStatus"];
        isActive: boolean;
        approvalAttention: SessionOverviewItem["approvalAttention"];
        pendingApprovalCount: number;
        lastActivityAtMs?: string | null;
        lastEventPreview?: string | null;
        recentActivity?: unknown[];
      }>;
    };
    return {
      sessions: (record.sessions ?? []).map((session) => ({
        session: parseSessionSummary(session.session),
        latestRun: parseNullableBoundaryValue(session.latestRun, (latestRun) =>
          parseRunSummary(latestRun),
        ),
        laneStatus: session.laneStatus,
        isActive: session.isActive,
        approvalAttention: session.approvalAttention,
        pendingApprovalCount: session.pendingApprovalCount,
        lastActivityAtMs:
          session.lastActivityAtMs == null
            ? null
            : parseProtocolBigInt(session.lastActivityAtMs, "SessionOverview.lastActivityAtMs"),
        lastEventPreview: session.lastEventPreview ?? null,
        recentActivity: (session.recentActivity ?? []).map((event) =>
          parseDaemonEventEnvelope(event),
        ),
      })),
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("SessionOverviewResult", validateSessionOverviewResult.errors),
  );
}
