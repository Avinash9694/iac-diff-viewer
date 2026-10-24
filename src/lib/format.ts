import type { JsonValue } from '../engine/types';

export function isScalar(value: JsonValue | undefined): boolean {
  return value === undefined || value === null || typeof value !== 'object';
}

/** Scalars print bare; objects and arrays as compact JSON. */
export function formatValue(value: JsonValue | undefined): string {
  if (value === undefined) return '—';
  if (value === null) return 'null';
  if (typeof value === 'string') return value === '' ? '""' : value;
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

export function formatPretty(value: JsonValue | undefined): string {
  if (value === undefined) return '';
  return JSON.stringify(value, null, 2);
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(kb < 10 ? 1 : 0)} KB`;
  return `${(kb / 1024).toFixed(2)} MB`;
}

export function pluralize(count: number, singular: string, plural = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : plural}`;
}
