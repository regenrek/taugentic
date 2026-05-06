import type { ArtifactSnapshotResult, ListArtifactsQuery } from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import {
  ajv,
  formatProtocolValidationErrors,
  parseSchema,
  ProtocolValidationError,
} from "./core.js";
import { parseNullableDaemonEventCursor } from "./cursors.js";
import { parseArtifactSummary, parseArtifactSummaryList } from "./summaries.js";

const validateListArtifactsQuery = ajv.compile(PROTOCOL_JSON_SCHEMAS.ListArtifactsQuery);
const validateArtifactSnapshotResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.ArtifactSnapshotResult);

export { parseArtifactSummary, parseArtifactSummaryList };

export function parseListArtifactsQuery(value: unknown): ListArtifactsQuery {
  return parseSchema<ListArtifactsQuery>("ListArtifactsQuery", validateListArtifactsQuery, value);
}

export function parseArtifactSnapshotResult(value: unknown): ArtifactSnapshotResult {
  if (validateArtifactSnapshotResult(value)) {
    const record = value as {
      items?: unknown[];
      latestCursor?: unknown;
    };
    return {
      items: (record.items ?? []).map((item) => parseArtifactSummary(item)),
      latestCursor: parseNullableDaemonEventCursor(
        record.latestCursor,
        "ArtifactSnapshotResult.latestCursor",
      ),
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("ArtifactSnapshotResult", validateArtifactSnapshotResult.errors),
  );
}
