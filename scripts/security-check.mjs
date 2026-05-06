#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import process from "node:process";

const repoRoot = "/Users/kregenrek/projects/taugentic";
const desktopRoot = `${repoRoot}/apps/desktop`;
const pnpmAuditLevel = process.env.TAUGENTIC_PNPM_AUDIT_LEVEL ?? "high";
const trivySeverity = process.env.TAUGENTIC_TRIVY_SEVERITY ?? "HIGH,CRITICAL";

const steps = [
  {
    name: "gitleaks",
    command: "gitleaks",
    args: ["git", "--no-banner", "--redact=100", "--config", ".gitleaks.toml", "."],
    cwd: repoRoot,
  },
  {
    name: "cargo-audit",
    command: "cargo",
    args: ["audit"],
    cwd: repoRoot,
  },
  {
    name: "pnpm-audit",
    command: "pnpm",
    args: ["audit", "--audit-level", pnpmAuditLevel],
    cwd: desktopRoot,
  },
  {
    name: "trivy-fs",
    command: "trivy",
    args: [
      "fs",
      "--exit-code",
      "1",
      "--scanners",
      "vuln,misconfig",
      "--severity",
      trivySeverity,
      "--skip-dirs",
      ".git",
      "--skip-dirs",
      "node_modules",
      "--skip-dirs",
      "target",
      ".",
    ],
    cwd: repoRoot,
  },
];

const failures = [];

for (const step of steps) {
  console.log(`\n== ${step.name} ==`);
  const result = spawnSync(step.command, step.args, {
    cwd: step.cwd,
    encoding: "utf8",
    env: process.env,
  });
  if (result.error) {
    console.error(`${step.name} failed to start: ${result.error.message}`);
    failures.push({
      name: step.name,
      reason: "failed to start",
      hint: startupHint(step.name),
    });
    continue;
  }
  if (result.stdout) {
    process.stdout.write(result.stdout);
  }
  if (result.stderr) {
    process.stderr.write(result.stderr);
  }
  const combinedOutput = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  if ((result.status ?? 1) !== 0) {
    failures.push({
      name: step.name,
      reason: "non-zero exit",
      hint: failureHint(step.name, combinedOutput),
    });
    continue;
  }
  if (step.name === "cargo-audit" && cargoAuditReportedAdvisory(combinedOutput)) {
    failures.push({
      name: step.name,
      reason: "reported advisory",
      hint: "resolve or explicitly policy-ignore the reported RustSec advisory before shipping",
    });
  }
}

if (failures.length === 0) {
  console.log("\nsecurity-check passed");
  process.exit(0);
}

console.error("\nsecurity-check failed:");
for (const failure of failures) {
  console.error(`- ${failure.name}: ${failure.reason}`);
  if (failure.hint) {
    console.error(`  hint: ${failure.hint}`);
  }
}
process.exit(1);

function startupHint(stepName) {
  if (stepName === "cargo-audit") {
    return "install or repair cargo-audit so RustSec advisories can be checked";
  }
  return null;
}

function failureHint(stepName, output) {
  if (stepName === "cargo-audit" && output.includes("unsupported CVSS version: 4.0")) {
    return "upgrade cargo-audit to a release that understands CVSS 4.0 advisories";
  }
  if (stepName === "pnpm-audit" && output.toLowerCase().includes("registry")) {
    return "retry with working registry/network access or use --ignore-registry-errors if that policy is intentional";
  }
  return null;
}

function cargoAuditReportedAdvisory(output) {
  return /^ID:\s*RUSTSEC-\d{4}-\d+/m.test(output);
}
