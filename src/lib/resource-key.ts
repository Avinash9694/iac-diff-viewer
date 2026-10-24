import type { ResourceDiff } from '../engine/types';

/** Stable list key: a removed and an added row may share an id after a type change. */
export function resourceKey(r: ResourceDiff): string {
  return `${r.op}:${r.id}`;
}
