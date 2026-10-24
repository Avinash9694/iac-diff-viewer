import '@testing-library/jest-dom/vitest';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { primeEngine } from '../engine/engine';

// The suite exercises the real wasm build: `npm run build:wasm` must have run.
const wasmPath = resolve(process.cwd(), 'src/wasm/iac_diff_bg.wasm');
if (!existsSync(wasmPath)) {
  throw new Error(`Missing ${wasmPath}. Run \`npm run build:wasm\` before \`npm test\`.`);
}
primeEngine(new Uint8Array(readFileSync(wasmPath)));
