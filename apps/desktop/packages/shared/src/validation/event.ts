import type { DaemonEvent, DaemonEventEnvelope } from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import {
  ajv,
  formatProtocolValidationErrors,
  parseProtocolBigInt,
  ProtocolValidationError,
} from "./core.js";

const validateDaemonEventEnvelope = ajv.compile(PROTOCOL_JSON_SCHEMAS.PublicDaemonEventEnvelope);

export function parseDaemonEventEnvelope(value: unknown): DaemonEventEnvelope {
  if (validateDaemonEventEnvelope(value)) {
    const record = value as {
      daemonInstanceId: string;
      sessionId: string;
      sequence: string;
      occurredAtMs: string;
      event: DaemonEvent;
    };
    return {
      daemonInstanceId: record.daemonInstanceId,
      sessionId: record.sessionId,
      sequence: parseProtocolBigInt(record.sequence, "DaemonEventEnvelope.sequence"),
      occurredAtMs: parseProtocolBigInt(record.occurredAtMs, "DaemonEventEnvelope.occurredAtMs"),
      event: record.event,
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("DaemonEventEnvelope", validateDaemonEventEnvelope.errors),
  );
}
