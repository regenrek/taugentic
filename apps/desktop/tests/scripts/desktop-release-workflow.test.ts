import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vite-plus/test";

const workflowPath = new URL("../../../../.github/workflows/desktop-release.yml", import.meta.url);

describe("desktop-release workflow", () => {
  it("keeps packaging publish-disabled inside the matrix job", async () => {
    const packageJob = await readWorkflowJob("package");

    expect(packageJob).toContain("run: pnpm --dir apps/desktop package -- --publish never");
    expect(packageJob).not.toContain("gh release create");
    expect(packageJob).not.toContain("gh release upload");
    expect(packageJob).not.toContain("gh release edit");
  });

  it("keeps GitHub release mutation and publish-mode validation in finalize-release only", async () => {
    const workflow = await readWorkflowText();
    const finalizeJob = getWorkflowJobSection(workflow, "finalize-release");
    const firstGhReleaseIndex = finalizeJob.indexOf("gh release ");

    expect(finalizeJob).toContain("if: ${{ startsWith(github.ref, 'refs/tags/v') }}");
    expect(finalizeJob).toContain(
      'run: test "$(pnpm --dir apps/desktop release:publish-mode -- --ref="${GITHUB_REF}")" = "always"',
    );
    expect(firstGhReleaseIndex).toBeGreaterThan(-1);
    expect(finalizeJob.indexOf("release:publish-mode")).toBeLessThan(firstGhReleaseIndex);

    const ghReleaseCommands = workflow.match(/gh release (create|upload|edit)/g) ?? [];
    expect(ghReleaseCommands).toEqual([
      "gh release create",
      "gh release upload",
      "gh release edit",
    ]);
  });

  it("keeps the matrix upload and finalizer download on the canonical desktop-release prefix", async () => {
    const workflow = await readWorkflowText();
    const packageJob = getWorkflowJobSection(workflow, "package");
    const finalizeJob = getWorkflowJobSection(workflow, "finalize-release");

    expect(packageJob).toContain(
      "name: desktop-release-${{ github.event_name == 'workflow_dispatch' && inputs.release_profile || 'stable' }}-${{ matrix.platform }}",
    );
    expect(finalizeJob).toContain("pattern: desktop-release-*");
  });
});

async function readWorkflowJob(jobName: string) {
  return getWorkflowJobSection(await readWorkflowText(), jobName);
}

async function readWorkflowText() {
  return await readFile(workflowPath, "utf8");
}

function getWorkflowJobSection(workflow: string, jobName: string) {
  const start = workflow.indexOf(`  ${jobName}:`);
  if (start === -1) {
    throw new Error(`missing workflow job: ${jobName}`);
  }
  const nextJob = workflow.slice(start + 1).search(/\n {2}[a-z0-9-]+:\n/g);
  const end = nextJob === -1 ? workflow.length : start + 1 + nextJob;
  return workflow.slice(start, end);
}
