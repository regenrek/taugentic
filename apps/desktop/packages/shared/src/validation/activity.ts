import type {
  ActivityPageItem,
  ActivityPageQuery,
  ActivityPageResult,
  DaemonEvent,
} from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import {
  ajv,
  formatProtocolValidationErrors,
  parseNullableBoundaryValue,
  parseProtocolBigInt,
  ProtocolValidationError,
} from "./core.js";
import { parseActivityCursor } from "./cursors.js";

const validateActivityPageQuery = ajv.compile(PROTOCOL_JSON_SCHEMAS.ActivityPageQuery);
const validateActivityPageResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.PublicActivityPageResult);

export function parseActivityPageQuery(value: unknown): ActivityPageQuery {
  const record = parseActivityPageQuerySchema(value);
  return {
    limit: record.limit,
    before: parseNullableBoundaryValue(record.before, (before) =>
      parseActivityCursor(before, "ActivityPageQuery.before"),
    ),
    kinds: record.kinds ?? [],
  };
}

function parseActivityPageQuerySchema(value: unknown): {
  limit: number;
  before?: unknown;
  kinds?: ActivityPageQuery["kinds"];
} {
  if (validateActivityPageQuery(value)) {
    return value as {
      limit: number;
      before?: unknown;
      kinds?: ActivityPageQuery["kinds"];
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("ActivityPageQuery", validateActivityPageQuery.errors),
  );
}

export function parseActivityPageResult(value: unknown): ActivityPageResult {
  if (validateActivityPageResult(value)) {
    const record = value as {
      items?: unknown[];
      nextBefore?: unknown;
      latestActivityCursor?: unknown;
    };
    return {
      items: (record.items ?? []).map((item) => parseActivityPageItem(item)),
      nextBefore: parseNullableBoundaryValue(record.nextBefore, (nextBefore) =>
        parseActivityCursor(nextBefore, "ActivityPageResult.nextBefore"),
      ),
      latestActivityCursor: parseNullableBoundaryValue(
        record.latestActivityCursor,
        (latestActivityCursor) =>
          parseActivityCursor(latestActivityCursor, "ActivityPageResult.latestActivityCursor"),
      ),
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("ActivityPageResult", validateActivityPageResult.errors),
  );
}

function parseActivityPageItem(value: unknown): ActivityPageItem {
  if (
    typeof value !== "object" ||
    value === null ||
    !("cursor" in value) ||
    !("occurredAtMs" in value) ||
    !("event" in value)
  ) {
    throw new ProtocolValidationError(
      "ActivityPageItem failed protocol validation: missing cursor, occurredAtMs, or event",
    );
  }

  const record = value as {
    cursor: unknown;
    occurredAtMs: string;
    event: DaemonEvent;
  };
  return {
    cursor: parseActivityCursor(record.cursor, "ActivityPageItem.cursor"),
    occurredAtMs: parseProtocolBigInt(record.occurredAtMs, "ActivityPageItem.occurredAtMs"),
    event: record.event,
  };
}
