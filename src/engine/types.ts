// Mirrors the serde output of crates/iac-diff/src/model.rs. Field names are
// snake_case because they cross the wasm boundary as-is.

export type DocKind = 'cloud-formation' | 'terraform-plan' | 'terraform-state';
export type Syntax = 'json' | 'yaml';
export type Op = 'added' | 'removed' | 'modified' | 'renamed' | 'unchanged';
export type ChangeOp = 'added' | 'removed' | 'modified';
export type Risk = 'low' | 'medium' | 'high' | 'critical';
export type Side = 'before' | 'after';

export type JsonValue =
  null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };

export interface Detected {
  kind: DocKind;
  syntax: Syntax;
  label: string;
  resource_count: number;
}

export interface PropertyChange {
  path: string;
  before_path?: string;
  op: ChangeOp;
  before?: JsonValue;
  after?: JsonValue;
}

export interface ResourceDiff {
  id: string;
  renamed_from?: string;
  kind: string;
  op: Op;
  risk: Risk;
  reasons: string[];
  changes: PropertyChange[];
  before?: JsonValue;
  after?: JsonValue;
  forces_replacement?: boolean;
}

export interface OpCounts {
  added: number;
  removed: number;
  modified: number;
  renamed: number;
  unchanged: number;
}

export interface RiskCounts {
  low: number;
  medium: number;
  high: number;
  critical: number;
}

export interface Summary {
  total: number;
  changed: number;
  by_op: OpCounts;
  by_risk: RiskCounts;
  overall_risk: Risk;
}

export interface DiffReport {
  before: Detected;
  after: Detected;
  summary: Summary;
  resources: ResourceDiff[];
}

export interface EngineErrorShape {
  error: string;
  line?: number;
  column?: number;
  side?: Side;
}

export const RISK_ORDER: readonly Risk[] = ['critical', 'high', 'medium', 'low'];
export const OP_ORDER: readonly Op[] = ['removed', 'modified', 'renamed', 'added', 'unchanged'];

export const RISK_LABEL: Record<Risk, string> = {
  critical: 'Critical',
  high: 'High',
  medium: 'Medium',
  low: 'Low',
};

export const OP_LABEL: Record<Op, string> = {
  added: 'Added',
  removed: 'Removed',
  modified: 'Modified',
  renamed: 'Renamed',
  unchanged: 'Unchanged',
};

export const OP_GLYPH: Record<Op, string> = {
  added: '+',
  removed: '−',
  modified: '~',
  renamed: '↻',
  unchanged: '=',
};
