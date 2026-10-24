import type { Op, ResourceDiff, Risk } from '../engine/types';
import { RISK_ORDER } from '../engine/types';

export interface FilterState {
  query: string;
  /** Empty means every op. */
  ops: Op[];
  /** Empty means every risk. */
  risks: Risk[];
  /** Empty means every resource type. */
  kind: string;
  showUnchanged: boolean;
}

export const DEFAULT_FILTERS: FilterState = {
  query: '',
  ops: [],
  risks: [],
  kind: '',
  showUnchanged: false,
};

export function isFilterActive(f: FilterState): boolean {
  return f.query !== '' || f.ops.length > 0 || f.risks.length > 0 || f.kind !== '';
}

export function toggleIn<T>(list: T[], value: T): T[] {
  return list.includes(value) ? list.filter((v) => v !== value) : [...list, value];
}

function matchesQuery(r: ResourceDiff, needle: string): boolean {
  if (needle === '') return true;
  const hay = [r.id, r.kind, r.renamed_from ?? '', ...r.reasons, ...r.changes.map((c) => c.path)]
    .join('\n')
    .toLowerCase();
  return hay.includes(needle);
}

export function applyFilters(resources: readonly ResourceDiff[], f: FilterState): ResourceDiff[] {
  const needle = f.query.trim().toLowerCase();
  return resources.filter((r) => {
    if (r.op === 'unchanged' && !f.showUnchanged && !f.ops.includes('unchanged')) return false;
    if (f.ops.length > 0 && !f.ops.includes(r.op)) return false;
    if (f.risks.length > 0 && r.op !== 'unchanged' && !f.risks.includes(r.risk)) return false;
    if (f.risks.length > 0 && r.op === 'unchanged') return false;
    if (f.kind !== '' && r.kind !== f.kind) return false;
    return matchesQuery(r, needle);
  });
}

export type GroupKey = Risk | 'unchanged';

export interface ResourceGroup {
  key: GroupKey;
  label: string;
  items: ResourceDiff[];
}

const GROUP_LABEL: Record<GroupKey, string> = {
  critical: 'Critical',
  high: 'High',
  medium: 'Medium',
  low: 'Low',
  unchanged: 'Unchanged',
};

/** Risk bands in descending order, then an unchanged band; empty bands are dropped. */
export function groupByRisk(resources: readonly ResourceDiff[]): ResourceGroup[] {
  const keys: GroupKey[] = [...RISK_ORDER, 'unchanged'];
  const groups = keys.map((key) => ({ key, label: GROUP_LABEL[key], items: [] as ResourceDiff[] }));
  for (const r of resources) {
    const key: GroupKey = r.op === 'unchanged' ? 'unchanged' : r.risk;
    groups.find((g) => g.key === key)?.items.push(r);
  }
  return groups.filter((g) => g.items.length > 0);
}

export function uniqueKinds(resources: readonly ResourceDiff[]): string[] {
  return [...new Set(resources.map((r) => r.kind))].sort((a, b) => a.localeCompare(b));
}
