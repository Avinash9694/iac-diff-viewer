import initWasm, {
  initSync,
  diff as wasmDiff,
  detect_document as wasmDetect,
  version as wasmVersion,
} from '../wasm/iac_diff.js';
import wasmUrl from '../wasm/iac_diff_bg.wasm?url';
import type { Detected, DiffReport, EngineErrorShape, Side } from './types';

export interface EngineInfo {
  version: string;
  wasmBytes: number;
}

export class EngineError extends Error {
  readonly line: number | undefined;
  readonly column: number | undefined;
  readonly side: Side | undefined;

  constructor(shape: EngineErrorShape) {
    super(shape.error);
    this.name = 'EngineError';
    this.line = shape.line;
    this.column = shape.column;
    this.side = shape.side;
  }

  get location(): string | null {
    return this.line !== undefined && this.column !== undefined
      ? `line ${this.line}, column ${this.column}`
      : null;
  }
}

let ready: Promise<EngineInfo> | null = null;

/**
 * Fetch and instantiate the wasm module once. The bytes are fetched by hand
 * rather than streamed so the footer can report the exact engine size.
 */
export function loadEngine(): Promise<EngineInfo> {
  if (!ready) {
    ready = instantiate().catch((err: unknown) => {
      ready = null;
      throw err;
    });
  }
  return ready;
}

async function instantiate(): Promise<EngineInfo> {
  const response = await fetch(wasmUrl);
  if (!response.ok) {
    throw new Error(`Could not download the diff engine (HTTP ${response.status})`);
  }
  const bytes = await response.arrayBuffer();
  await initWasm({ module_or_path: bytes });
  return { version: wasmVersion(), wasmBytes: bytes.byteLength };
}

/** Synchronous init from bytes already in memory; used by the Node test suite. */
export function primeEngine(bytes: Uint8Array): EngineInfo {
  initSync({ module: bytes });
  const info = { version: wasmVersion(), wasmBytes: bytes.byteLength };
  ready = Promise.resolve(info);
  return info;
}

export function isEngineErrorShape(value: unknown): value is EngineErrorShape {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as { error?: unknown }).error === 'string'
  );
}

function rethrow(err: unknown): never {
  if (err instanceof EngineError) throw err;
  if (isEngineErrorShape(err)) throw new EngineError(err);
  if (err instanceof Error) throw new EngineError({ error: err.message });
  throw new EngineError({ error: String(err) });
}

export function diffDocuments(before: string, after: string): DiffReport {
  try {
    return wasmDiff(before, after) as DiffReport;
  } catch (err: unknown) {
    rethrow(err);
  }
}

export function detectDocument(doc: string): Detected {
  try {
    return wasmDetect(doc) as Detected;
  } catch (err: unknown) {
    rethrow(err);
  }
}
