import type { DiffReport, ResourceDiff } from '../engine/types';
import { OP_LABEL, RISK_LABEL } from '../engine/types';
import { formatValue } from './format';

export interface ReportOptions {
  includeUnchanged?: boolean;
  generatedAt?: Date;
}

function cell(text: string): string {
  return text.replace(/\|/g, '\\|').replace(/\n/g, ' ');
}

function code(text: string): string {
  return '`' + text.replace(/`/g, 'ˋ') + '`';
}

function resourceSection(r: ResourceDiff): string[] {
  const title = r.op === 'renamed' && r.renamed_from ? `${r.renamed_from} → ${r.id}` : r.id;
  const lines = [
    `### ${title}`,
    '',
    `- Type: ${code(r.kind)}`,
    `- Change: ${OP_LABEL[r.op]}${r.forces_replacement ? ' (forces replacement)' : ''}`,
    `- Risk: **${RISK_LABEL[r.risk]}**`,
  ];
  for (const reason of r.reasons) lines.push(`  - ${reason}`);
  if (r.changes.length > 0) {
    lines.push('', '| Property | Before | After |', '| --- | --- | --- |');
    for (const c of r.changes) {
      const path =
        c.before_path && c.before_path !== c.path ? `${c.before_path} → ${c.path}` : c.path;
      lines.push(
        `| ${code(cell(path))} | ${cell(formatValue(c.before))} | ${cell(formatValue(c.after))} |`,
      );
    }
  }
  lines.push('');
  return lines;
}

export function buildMarkdownReport(report: DiffReport, options: ReportOptions = {}): string {
  const { summary } = report;
  const when = (options.generatedAt ?? new Date()).toISOString().slice(0, 16).replace('T', ' ');
  const resources = report.resources.filter(
    (r) => options.includeUnchanged || r.op !== 'unchanged',
  );
  const lines: string[] = [
    '# Infrastructure diff report',
    '',
    `Generated ${when} UTC by IaC Diff Viewer.`,
    '',
    `- Before: ${report.before.label} — ${report.before.resource_count} resources`,
    `- After: ${report.after.label} — ${report.after.resource_count} resources`,
    `- Overall risk: **${RISK_LABEL[summary.overall_risk]}**`,
    '',
    '| Added | Removed | Modified | Renamed | Unchanged |',
    '| ---: | ---: | ---: | ---: | ---: |',
    `| ${summary.by_op.added} | ${summary.by_op.removed} | ${summary.by_op.modified} | ${summary.by_op.renamed} | ${summary.by_op.unchanged} |`,
    '',
    '| Critical | High | Medium | Low |',
    '| ---: | ---: | ---: | ---: |',
    `| ${summary.by_risk.critical} | ${summary.by_risk.high} | ${summary.by_risk.medium} | ${summary.by_risk.low} |`,
    '',
    '## Resources',
    '',
  ];
  if (resources.length === 0) {
    lines.push('No changes.', '');
  }
  for (const r of resources) lines.push(...resourceSection(r));
  return lines.join('\n');
}

export function reportFilename(report: DiffReport, now: Date = new Date()): string {
  const stamp = now.toISOString().slice(0, 10);
  return `iac-diff-${report.summary.overall_risk}-${stamp}.md`;
}
