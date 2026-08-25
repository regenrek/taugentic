import { mkdir, rename, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const sourceUrl = "https://models.dev/api.json";
const scriptDir = dirname(fileURLToPath(import.meta.url));
const outputPath = resolve(scriptDir, "../crates/ta-model-catalog/generated/catalog.json");
const stagedPath = `${outputPath}.new`;
const providerIds = [
  "anthropic",
  "deepseek",
  "google",
  "groq",
  "openai",
  "openrouter",
  "xai",
];

const response = await fetch(sourceUrl, { signal: AbortSignal.timeout(15_000) });
if (!response.ok) {
  throw new Error(`models.dev returned HTTP ${response.status}`);
}

const upstream = await response.json();
const providers = {};
for (const providerId of providerIds) {
  const provider = upstream[providerId];
  const models = {};
  for (const modelId of Object.keys(provider.models ?? {}).sort()) {
    const model = provider.models[modelId];
    if (model.tool_call !== true || model.status === "deprecated") continue;
    models[modelId] = {
      id: model.id,
      name: model.name,
      releaseDate: model.release_date ?? null,
      contextLimit: model.limit?.context ?? null,
      inputCostPerMillionMicros: dollarsToMicros(model.cost?.input),
      outputCostPerMillionMicros: dollarsToMicros(model.cost?.output),
      reasoning: model.reasoning === true,
      toolCall: true,
      structuredOutput: model.structured_output === true,
      inputModalities: [...(model.modalities?.input ?? [])],
    };
  }
  providers[providerId] = {
    id: provider.id,
    name: provider.name,
    models,
  };
}

for (const providerId of providerIds) {
  if (!providers[providerId] || Object.keys(providers[providerId].models).length === 0) {
    throw new Error(`models.dev catalog is missing required provider ${providerId}`);
  }
}

const catalog = {
  generatedAt: new Date().toISOString(),
  source: sourceUrl,
  providers,
};

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(stagedPath, `${JSON.stringify(catalog, null, 2)}\n`, "utf8");
await rm(outputPath, { force: true });
await rename(stagedPath, outputPath);
console.log(`wrote ${outputPath}`);

function dollarsToMicros(value) {
  return typeof value === "number" ? Math.round(value * 1_000_000) : null;
}
