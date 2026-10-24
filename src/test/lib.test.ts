import { describe, expect, it } from 'vitest';
import { diffDocuments } from '../engine/engine';
import type { ResourceDiff } from '../engine/types';
import {
  DEFAULT_FILTERS,
  applyFilters,
  groupByRisk,
  isFilterActive,
  toggleIn,
  uniqueKinds,
} from '../lib/filters';
import { formatBytes, formatValue, pluralize } from '../lib/format';
import { diffLines } from '../lib/line-diff';
import { buildMarkdownReport, reportFilename } from '../lib/markdown-report';
import { findSample } from '../samples';

function res(
  partial: Partial<ResourceDiff> & Pick<ResourceDiff, 'id' | 'op' | 'risk'>,
): ResourceDiff {
  return { kind: 'aws_thing', reasons: [], changes: [], ...partial };
}

const fixture: ResourceDiff[] = [
  res({
    id: 'db',
    op: 'removed',
    risk: 'critical',
    kind: 'aws_db_instance',
    reasons: ['Stateful resource removed'],
  }),
  res({
    id: 'sg',
    op: 'modified',
    risk: 'critical',
    kind: 'aws_security_group',
    changes: [{ path: 'ingress[0].cidr_blocks[1]', op: 'added', after: '0.0.0.0/0' }],
  }),
  res({ id: 'role', op: 'modified', risk: 'high', kind: 'aws_iam_role' }),
  res({ id: 'web', op: 'modified', risk: 'medium', kind: 'aws_instance' }),
  res({ id: 'logs', op: 'added', risk: 'low', kind: 'aws_cloudwatch_log_group' }),
  res({ id: 'vpc', op: 'unchanged', risk: 'low', kind: 'aws_vpc' }),
];

describe('filters', () => {
  it('hides unchanged resources by default and shows them on request', () => {
    expect(applyFilters(fixture, DEFAULT_FILTERS).map((r) => r.id)).toEqual([
      'db',
      'sg',
      'role',
      'web',
      'logs',
    ]);
    expect(applyFilters(fixture, { ...DEFAULT_FILTERS, showUnchanged: true })).toHaveLength(6);
  });

  it('filters by op, risk and type', () => {
    expect(
      applyFilters(fixture, { ...DEFAULT_FILTERS, ops: ['removed', 'added'] }).map((r) => r.id),
    ).toEqual(['db', 'logs']);
    expect(
      applyFilters(fixture, { ...DEFAULT_FILTERS, risks: ['critical'] }).map((r) => r.id),
    ).toEqual(['db', 'sg']);
    expect(
      applyFilters(fixture, { ...DEFAULT_FILTERS, kind: 'aws_instance' }).map((r) => r.id),
    ).toEqual(['web']);
  });

  it('matches free text against id, type, reasons and change paths', () => {
    expect(
      applyFilters(fixture, { ...DEFAULT_FILTERS, query: 'STATEFUL' }).map((r) => r.id),
    ).toEqual(['db']);
    expect(
      applyFilters(fixture, { ...DEFAULT_FILTERS, query: 'cidr_blocks' }).map((r) => r.id),
    ).toEqual(['sg']);
    expect(applyFilters(fixture, { ...DEFAULT_FILTERS, query: 'iam' }).map((r) => r.id)).toEqual([
      'role',
    ]);
  });

  it('groups by risk band in descending order with unchanged last', () => {
    const groups = groupByRisk(fixture);
    expect(groups.map((g) => [g.key, g.items.length])).toEqual([
      ['critical', 2],
      ['high', 1],
      ['medium', 1],
      ['low', 1],
      ['unchanged', 1],
    ]);
  });

  it('helpers', () => {
    expect(isFilterActive(DEFAULT_FILTERS)).toBe(false);
    expect(isFilterActive({ ...DEFAULT_FILTERS, query: 'x' })).toBe(true);
    expect(toggleIn(['a'], 'b')).toEqual(['a', 'b']);
    expect(toggleIn(['a', 'b'], 'a')).toEqual(['b']);
    expect(uniqueKinds(fixture)).toEqual([
      'aws_cloudwatch_log_group',
      'aws_db_instance',
      'aws_iam_role',
      'aws_instance',
      'aws_security_group',
      'aws_vpc',
    ]);
  });
});

describe('format', () => {
  it('formats values and sizes', () => {
    expect(formatValue(undefined)).toBe('—');
    expect(formatValue(null)).toBe('null');
    expect(formatValue('')).toBe('""');
    expect(formatValue('t3.micro')).toBe('t3.micro');
    expect(formatValue({ a: [1, true] })).toBe('{"a":[1,true]}');
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(315279)).toBe('308 KB');
    expect(formatBytes(2 * 1024 * 1024)).toBe('2.00 MB');
    expect(pluralize(1, 'resource')).toBe('1 resource');
    expect(pluralize(3, 'resource')).toBe('3 resources');
  });
});

describe('diffLines', () => {
  it('produces a unified diff that keeps common lines', () => {
    const lines = diffLines('a\nb\nc', 'a\nx\nb\nc');
    expect(lines).toEqual([
      { kind: 'same', text: 'a' },
      { kind: 'add', text: 'x' },
      { kind: 'same', text: 'b' },
      { kind: 'same', text: 'c' },
    ]);
  });

  it('handles empty sides', () => {
    expect(diffLines('', 'a')).toEqual([{ kind: 'add', text: 'a' }]);
    expect(diffLines('a', '')).toEqual([{ kind: 'del', text: 'a' }]);
    expect(diffLines('', '')).toEqual([]);
  });
});

describe('markdown report', () => {
  it('renders summary tables and one section per changed resource', () => {
    const s = findSample('cfn-web-tier');
    if (!s) throw new Error('sample missing');
    const report = diffDocuments(s.before, s.after);
    const md = buildMarkdownReport(report, { generatedAt: new Date('2026-10-24T10:00:00Z') });
    expect(md).toContain('# Infrastructure diff report');
    expect(md).toContain('Generated 2026-10-24 10:00 UTC');
    expect(md).toContain('- Overall risk: **Critical**');
    expect(md).toContain('| 1 | 2 | 6 | 0 | 3 |');
    expect(md).toContain('### Database');
    expect(md).toContain('- Risk: **Critical**');
    expect(md).toContain('| `SecurityGroupIngress[1].CidrIp` | 10.0.0.0/8 | 0.0.0.0/0 |');
    expect(md).not.toContain('### Vpc');
    expect(buildMarkdownReport(report, { includeUnchanged: true })).toContain('### Vpc');
    expect(reportFilename(report, new Date('2026-10-24T10:00:00Z'))).toBe(
      'iac-diff-critical-2026-10-24.md',
    );
  });

  it('escapes pipes inside table cells', () => {
    const report = diffDocuments(
      '{"Resources":{"A":{"Type":"AWS::SNS::Topic","Properties":{"Name":"x|y"}}}}',
      '{"Resources":{"A":{"Type":"AWS::SNS::Topic","Properties":{"Name":"x|z"}}}}',
    );
    expect(buildMarkdownReport(report)).toContain('| `Name` | x\\|y | x\\|z |');
  });
});
