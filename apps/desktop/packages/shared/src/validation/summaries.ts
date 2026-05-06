import type { ApprovalRequest, ArtifactSummary, RunSummary, SessionSummary } from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import { ajv, formatProtocolValidationErrors, ProtocolValidationError } from "./core.js";

const validateApprovalRequest = ajv.compile(PROTOCOL_JSON_SCHEMAS.ApprovalRequest);
const validateArtifactSummary = ajv.compile(PROTOCOL_JSON_SCHEMAS.ArtifactSummary);
const validateRunSummary = ajv.compile(PROTOCOL_JSON_SCHEMAS.RunSummary);
const validateSessionSummary = ajv.compile(PROTOCOL_JSON_SCHEMAS.SessionSummary);

export function parseRunSummary(value: unknown): RunSummary {
  if (validateRunSummary(value)) {
    return value as RunSummary;
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("RunSummary", validateRunSummary.errors),
  );
}

export function parseRunSummaryList(value: unknown): RunSummary[] {
  if (!Array.isArray(value)) {
    throw new ProtocolValidationError(
      "RunSummary[] failed protocol validation: value is not an array",
    );
  }

  return value.map((item) => parseRunSummary(item));
}

export function parseApprovalRequest(value: unknown): ApprovalRequest {
  if (validateApprovalRequest(value)) {
    return value as ApprovalRequest;
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("ApprovalRequest", validateApprovalRequest.errors),
  );
}

export function parseApprovalRequestList(value: unknown): ApprovalRequest[] {
  if (!Array.isArray(value)) {
    throw new ProtocolValidationError(
      "ApprovalRequest[] failed protocol validation: value is not an array",
    );
  }

  return value.map((item) => parseApprovalRequest(item));
}

export function parseArtifactSummary(value: unknown): ArtifactSummary {
  if (validateArtifactSummary(value)) {
    return value as ArtifactSummary;
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("ArtifactSummary", validateArtifactSummary.errors),
  );
}

export function parseArtifactSummaryList(value: unknown): ArtifactSummary[] {
  if (!Array.isArray(value)) {
    throw new ProtocolValidationError(
      "ArtifactSummary[] failed protocol validation: value is not an array",
    );
  }

  return value.map((item) => parseArtifactSummary(item));
}

export function parseSessionSummary(value: unknown): SessionSummary {
  if (validateSessionSummary(value)) {
    return value as SessionSummary;
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("SessionSummary", validateSessionSummary.errors),
  );
}

export function parseSessionSummaryList(value: unknown): SessionSummary[] {
  if (!Array.isArray(value)) {
    throw new ProtocolValidationError(
      "SessionSummary[] failed protocol validation: value is not an array",
    );
  }

  return value.map((item) => parseSessionSummary(item));
}
