import { constants } from "node:fs";
import { copyFile, mkdir, rename, rm } from "node:fs/promises";
import { execFile } from "node:child_process";
import { randomUUID } from "node:crypto";
import { promisify } from "node:util";
import { basename, dirname, resolve } from "node:path";
import { platform } from "node:process";

const root = resolve(import.meta.dirname, "../../../../../");
const profile = process.env.CARGO_PROFILE ?? "debug";
const extension = platform === "darwin" ? "dylib" : platform === "win32" ? "dll" : "so";
const { stdout } = await promisify(execFile)("cargo", ["metadata", "--format-version=1", "--no-deps"], { cwd: root });
const { target_directory } = JSON.parse(stdout);
const source = resolve(target_directory, profile, `libta_desktop_native.${extension}`);
const destination = resolve(import.meta.dirname, "../taugentic_desktop_native.node");
const destinationDirectory = dirname(destination);
const temporary = resolve(
  destinationDirectory,
  `.${basename(destination)}.${process.pid}.${randomUUID()}.tmp`,
);

await mkdir(destinationDirectory, { recursive: true });
try {
  await copyFile(source, temporary, constants.COPYFILE_EXCL);
  await rename(temporary, destination);
} finally {
  await rm(temporary, { force: true });
}
