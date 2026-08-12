import { copyFile, mkdir, readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import path from "node:path";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDirectory, "..");
const source = path.join(
  projectRoot,
  "node_modules",
  "@mediapipe",
  "tasks-vision",
  "wasm",
);
const destination = path.join(projectRoot, "public", "mediapipe", "wasm");

const assets = [
  "vision_wasm_internal.js",
  "vision_wasm_internal.wasm",
  "vision_wasm_nosimd_internal.js",
  "vision_wasm_nosimd_internal.wasm",
];

await mkdir(destination, { recursive: true });
for (const asset of assets) {
  await copyFile(path.join(source, asset), path.join(destination, asset));
}

const modelRoot = path.join(projectRoot, "public", "models", "vision");
const manifest = await readFile(path.join(modelRoot, "SHA256SUMS.txt"), "utf8");
const expected = new Map(
  manifest
    .split(/\r?\n/u)
    .map((line) => /^([0-9a-f]{64})\s+(.+)$/iu.exec(line))
    .filter(Boolean)
    .map((match) => [match[2], match[1].toLowerCase()]),
);

for (const model of ["face_landmarker.task", "gesture_recognizer.task"]) {
  const expectedHash = expected.get(model);
  if (!expectedHash) {
    throw new Error(`Hash modello visivo mancante nel manifest: ${model}`);
  }

  const contents = await readFile(path.join(modelRoot, model));
  const actualHash = createHash("sha256").update(contents).digest("hex");
  if (actualHash !== expectedHash) {
    throw new Error(`Hash modello visivo non valido: ${model}`);
  }
}

console.log("Runtime e modelli MediaPipe verificati.");
