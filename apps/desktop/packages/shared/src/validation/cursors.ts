import type { ActivityCursor, DaemonEventCursor } from "../contracts.js";
import {
  parseNullableBoundaryValue,
  parseProtocolBigInt,
  ProtocolValidationError,
} from "./core.js";

export function parseNullableActivityCursor(
  value: unknown,
  fieldName: string,
): ActivityCursor | null {
  return parseNullableBoundaryValue(value, (cursor) => parseActivityCursor(cursor, fieldName));
}

export function parseNullableDaemonEventCursor(
  value: unknown,
  fieldName: string,
): DaemonEventCursor | null {
  return parseNullableBoundaryValue(value, (cursor) => parseDaemonEventCursor(cursor, fieldName));
}

export function parseNullableProtocolBigInt(value: unknown, fieldName: string): bigint | null {
  return parseNullableBoundaryValue(value, (item) => {
    if (typeof item !== "string") {
      throw new ProtocolValidationError(`${fieldName} must be a uint64 decimal string or null`);
    }
    return parseProtocolBigInt(item, fieldName);
  });
}

export function parseDaemonEventCursor(value: unknown, fieldName: string): DaemonEventCursor {
  if (
    typeof value !== "object" ||
    value === null ||
    !Object.hasOwn(value, "daemonInstanceId") ||
    !Object.hasOwn(value, "sessionId") ||
    !Object.hasOwn(value, "sequence") ||
    Object.keys(value).length !== 3
  ) {
    throw new ProtocolValidationError(
      `${fieldName} must contain only daemonInstanceId, sessionId, and sequence`,
    );
  }

  const record = value as {
    daemonInstanceId: string;
    sessionId: string;
    sequence: string;
  };
  if (typeof record.daemonInstanceId !== "string" || record.daemonInstanceId.length === 0) {
    throw new ProtocolValidationError(`${fieldName}.daemonInstanceId must be a non-empty string`);
  }
  if (typeof record.sessionId !== "string" || record.sessionId.length === 0) {
    throw new ProtocolValidationError(`${fieldName}.sessionId must be a non-empty string`);
  }
  return {
    daemonInstanceId: record.daemonInstanceId,
    sessionId: record.sessionId,
    sequence: parseProtocolBigInt(record.sequence, `${fieldName}.sequence`),
  };
}

export function parseActivityCursor(value: unknown, fieldName: string): ActivityCursor {
  if (
    typeof value !== "object" ||
    value === null ||
    !Object.hasOwn(value, "sequence") ||
    Object.keys(value).length !== 1
  ) {
    throw new ProtocolValidationError(`${fieldName} must contain only sequence`);
  }

  const record = value as { sequence: string };
  return {
    sequence: parseProtocolBigInt(record.sequence, `${fieldName}.sequence`),
  };
}
