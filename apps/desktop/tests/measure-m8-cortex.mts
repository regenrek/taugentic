import { performance } from "node:perf_hooks"
import { mapCortexGraph } from "../src/features/conversation-branches/branch-graph.js"
import type { RunLineageGraphResult } from "@taugentic/desktop-protocol"

const graph: RunLineageGraphResult = {
  nodes: Array.from({ length: 128 }, (_, index) => ({ id: `run-${String(index).padStart(3, "0")}`, relationship: index ? { kind: "nativeSubagent", parentRunId: `run-${String(index - 1).padStart(3, "0")}` } : { kind: "root" }, harness: "native", status: index === 127 ? "running" : "completed" })),
  edges: Array.from({ length: 127 }, (_, index) => ({ parentRunId: `run-${String(index).padStart(3, "0")}`, childRunId: `run-${String(index + 1).padStart(3, "0")}` })),
  orphanRunIds: [], totalCount: 128, omittedCount: 0, truncated: false, cycleBroken: false,
}
const samples = Array.from({ length: 128 }, () => {
  const started = performance.now()
  const mapped = mapCortexGraph(graph)
  return { elapsed: performance.now() - started, mapped }
})
const sorted = samples.map((sample) => sample.elapsed).sort((left, right) => left - right)
const p95 = sorted[Math.ceil(sorted.length * 0.95) - 1]!
const mapped = samples[0]!.mapped
if (p95 > 8) throw new Error(`Cortex product mapper p95 exceeded 8 ms: ${p95.toFixed(2)} ms`)
if (mapped.commands.length > 256) throw new Error("Cortex command cap exceeded")
if (mapped.commands.some((command) => command.type === "particle")) throw new Error("Cortex mapper emitted particles")
console.log(`m8-cortex product mapper 128 invocations; p95 ${p95.toFixed(2)} ms; ${mapped.commands.length} commands; 0 particles`)
