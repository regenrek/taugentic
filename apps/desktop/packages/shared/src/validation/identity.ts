import type { ApprovalDecision, ApprovalId, SessionAuthority, SessionId } from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import { ajv, parseNonEmptyProtocolString, parseSchema, ProtocolValidationError } from "./core.js";

const validateSessionId = ajv.compile(PROTOCOL_JSON_SCHEMAS.SessionId);
const validateSessionAuthority = ajv.compile(PROTOCOL_JSON_SCHEMAS.SessionAuthority);
const validateApprovalId = ajv.compile(PROTOCOL_JSON_SCHEMAS.ApprovalId);
const validateApprovalDecision = ajv.compile(PROTOCOL_JSON_SCHEMAS.ApprovalDecision);

export function parseSessionId(value: unknown): SessionId {
  const sessionId = parseSchema<SessionId>("SessionId", validateSessionId, value);
  return parseNonEmptyProtocolString(sessionId, "SessionId") as SessionId;
}

export function parseApprovalId(value: unknown): ApprovalId {
  const approvalId = parseSchema<ApprovalId>("ApprovalId", validateApprovalId, value);
  return parseNonEmptyProtocolString(approvalId, "ApprovalId") as ApprovalId;
}

export function parseSessionAuthority(value: unknown): SessionAuthority {
  const sessionAuthority = parseSchema<SessionAuthority>(
    "SessionAuthority",
    validateSessionAuthority,
    value,
  );
  return parseNonEmptyProtocolString(sessionAuthority, "SessionAuthority") as SessionAuthority;
}

export function parseClientCredential(value: unknown): string {
  if (typeof value !== "string") {
    throw new ProtocolValidationError("clientCredential must be a string");
  }

  const clientCredential = value.trim();
  if (clientCredential.length < 32 || !isAsciiProtocolString(clientCredential)) {
    throw new ProtocolValidationError(
      "clientCredential must be at least 32 non-whitespace ASCII characters",
    );
  }
  for (const char of clientCredential) {
    if (/\s/u.test(char)) {
      throw new ProtocolValidationError("clientCredential must not contain whitespace");
    }
  }
  return clientCredential;
}

function isAsciiProtocolString(value: string): boolean {
  for (const char of value) {
    if ((char.codePointAt(0) ?? 0) > 0x7f) {
      return false;
    }
  }
  return true;
}

export function parseApprovalDecision(value: unknown): ApprovalDecision {
  return parseSchema<ApprovalDecision>("ApprovalDecision", validateApprovalDecision, value);
}
