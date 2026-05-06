import { describe, expect, it } from "vite-plus/test";

import type { ArtifactSummary } from "../../packages/shared/generated/ArtifactSummary.js";
import { reconcileCurrentArtifactId } from "../../packages/renderer/src/features/artifacts/selection.js";

function makeArtifact(id: string, runId = "run-1"): ArtifactSummary {
  return {
    id,
    kind: "Patch",
    runId,
    storagePath: `artifacts/${runId}/${id}.diff`,
  };
}

describe("artifact selection", () => {
  it("keeps the selected artifact only while it still exists in the hydrated list", () => {
    expect(
      reconcileCurrentArtifactId("artifact-2", [
        makeArtifact("artifact-1"),
        makeArtifact("artifact-2"),
      ]),
    ).toBe("artifact-2");

    expect(reconcileCurrentArtifactId("artifact-2", [makeArtifact("artifact-1")])).toBeNull();
  });

  it("drops a stale artifact id after hydration replaces the list", () => {
    const initialHydratedArtifacts = [makeArtifact("artifact-1"), makeArtifact("artifact-2")];
    const nextHydratedArtifacts = [makeArtifact("artifact-3")];

    const selectedArtifactId = reconcileCurrentArtifactId("artifact-2", initialHydratedArtifacts);

    expect(selectedArtifactId).toBe("artifact-2");
    expect(reconcileCurrentArtifactId(selectedArtifactId, nextHydratedArtifacts)).toBeNull();
  });

  it("keeps a null artifact selection null after daemon hydration", () => {
    expect(reconcileCurrentArtifactId(null, [makeArtifact("artifact-1")])).toBeNull();
  });
});
