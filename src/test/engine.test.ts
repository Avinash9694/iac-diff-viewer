import { describe, expect, it } from 'vitest';
import { EngineError, detectDocument, diffDocuments } from '../engine/engine';
import type { ResourceDiff } from '../engine/types';
import { SAMPLES, findSample } from '../samples';

function sample(id: string) {
  const s = findSample(id);
  if (!s) throw new Error(`missing sample ${id}`);
  return s;
}

function byId(resources: ResourceDiff[], id: string): ResourceDiff {
  const r = resources.find((x) => x.id === id);
  if (!r) throw new Error(`missing resource ${id}`);
  return r;
}

describe('detectDocument', () => {
  it('identifies CloudFormation YAML with short-form intrinsics', () => {
    const d = detectDocument(sample('cfn-web-tier').before);
    expect(d).toMatchObject({ kind: 'cloud-formation', syntax: 'yaml', resource_count: 11 });
    expect(d.label).toBe('CloudFormation (YAML)');
  });

  it('identifies CloudFormation JSON', () => {
    const d = detectDocument('{"Resources":{"B":{"Type":"AWS::S3::Bucket"}}}');
    expect(d).toMatchObject({ kind: 'cloud-formation', syntax: 'json', resource_count: 1 });
  });

  it('identifies a Terraform plan and a Terraform state', () => {
    expect(detectDocument(sample('tf-plan').before).kind).toBe('terraform-plan');
    expect(detectDocument(sample('tf-rename').before)).toMatchObject({
      kind: 'terraform-state',
      resource_count: 6,
    });
  });

  it('throws a structured error with a position for malformed JSON', () => {
    let caught: unknown;
    try {
      detectDocument('{\n  "Resources": \n}');
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(EngineError);
    const e = caught as EngineError;
    expect(e.message).toMatch(/Invalid JSON/);
    expect(e.line).toBe(3);
    expect(e.location).toMatch(/line 3, column \d+/);
  });

  it('rejects an unrecognised document shape', () => {
    expect(() => detectDocument('{"hello": 1}')).toThrow(/Unrecognised document/);
  });
});

describe('diffDocuments', () => {
  it('names the failing side', () => {
    try {
      diffDocuments(sample('cfn-web-tier').before, 'Resources: [');
      throw new Error('expected a throw');
    } catch (err) {
      expect(err).toBeInstanceOf(EngineError);
      expect((err as EngineError).side).toBe('after');
    }
  });

  it('reports the CloudFormation sample as expected', () => {
    const { before, after } = sample('cfn-web-tier');
    const report = diffDocuments(before, after);
    expect(report.summary).toEqual({
      total: 12,
      changed: 9,
      by_op: { added: 1, removed: 2, modified: 6, renamed: 0, unchanged: 3 },
      by_risk: { low: 2, medium: 3, high: 2, critical: 2 },
      overall_risk: 'critical',
    });
    expect(byId(report.resources, 'Database')).toMatchObject({ op: 'removed', risk: 'critical' });
    const sg = byId(report.resources, 'WebSecurityGroup');
    expect(sg.risk).toBe('critical');
    expect(sg.reasons[0]).toMatch(/Ingress opened to the world/);
    expect(byId(report.resources, 'WebRole').reasons).toEqual(
      expect.arrayContaining([expect.stringMatching(/wildcard action: s3:\*/)]),
    );
    expect(byId(report.resources, 'OrdersTable').reasons).toEqual(
      expect.arrayContaining([expect.stringMatching(/DeletionPolicy weakened: Retain → Delete/)]),
    );
    expect(byId(report.resources, 'AssetsBucket')).toMatchObject({
      risk: 'low',
      reasons: ['Tag-only change'],
    });
    expect(byId(report.resources, 'ApiFunction')).toMatchObject({ op: 'added', risk: 'low' });
    // Ordered by risk, then op.
    expect(report.resources.map((r) => r.risk === 'critical')).toEqual([
      true,
      true,
      ...Array<boolean>(10).fill(false),
    ]);
  });

  it('diffs a single Terraform plan against itself (prior state vs planned values)', () => {
    const { before, after } = sample('tf-plan');
    expect(after).toBe('');
    const report = diffDocuments(before, after);
    expect(report.before.label).toBe('Terraform plan — prior state');
    expect(report.after.label).toBe('Terraform plan — planned values');
    expect(report.summary).toEqual({
      total: 8,
      changed: 6,
      by_op: { added: 1, removed: 0, modified: 5, renamed: 0, unchanged: 2 },
      by_risk: { low: 2, medium: 2, high: 2, critical: 0 },
      overall_risk: 'high',
    });
    expect(byId(report.resources, 'aws_s3_bucket_public_access_block.logs').risk).toBe('high');
    expect(byId(report.resources, 'aws_sqs_queue.events').reasons[0]).toMatch(/KMS key removed/);
    expect(byId(report.resources, 'module.database.aws_db_instance.main').reasons).toEqual(
      expect.arrayContaining([expect.stringMatching(/db\.t3\.small → db\.r6g\.large/)]),
    );
    expect(byId(report.resources, 'aws_instance.web').changes.map((c) => c.path)).toEqual([
      'instance_type',
      'tags.Environment',
      'tags_all.Environment',
    ]);
  });

  it('detects renamed Terraform addresses', () => {
    const { before, after } = sample('tf-rename');
    const report = diffDocuments(before, after);
    expect(report.summary.by_op).toEqual({
      added: 0,
      removed: 0,
      modified: 0,
      renamed: 3,
      unchanged: 3,
    });
    expect(report.summary.overall_risk).toBe('high');
    const bucket = byId(report.resources, 'aws_s3_bucket.static_assets');
    expect(bucket).toMatchObject({
      op: 'renamed',
      renamed_from: 'aws_s3_bucket.assets',
      risk: 'high',
    });
    expect(bucket.changes).toEqual([]);
    const role = byId(report.resources, 'aws_iam_role.app_role');
    expect(role).toMatchObject({ op: 'renamed', renamed_from: 'aws_iam_role.app', risk: 'low' });
    expect(role.changes).toEqual([
      {
        path: 'description',
        op: 'modified',
        before: 'Application role',
        after: 'Application role (web tier)',
      },
    ]);
  });

  it('every bundled sample diffs without error', () => {
    for (const s of SAMPLES) {
      expect(() => diffDocuments(s.before, s.after)).not.toThrow();
    }
  });
});
