import { useMemo, useState } from 'react';
import type { ResourceDiff } from '../engine/types';
import { OP_LABEL } from '../engine/types';
import { formatPretty, formatValue, isScalar, pluralize } from '../lib/format';
import { diffLines } from '../lib/line-diff';
import { OpChip, RiskBadge } from './Badges';
import { PropertyTree } from './PropertyTree';
import styles from './Detail.module.css';

type Tab = 'changes' | 'properties' | 'raw';

const TABS: Array<{ id: Tab; label: string }> = [
  { id: 'changes', label: 'Changes' },
  { id: 'properties', label: 'Properties' },
  { id: 'raw', label: 'Raw JSON diff' },
];

function ValueCell({ value, op }: { value: ResourceDiff['before']; op: 'before' | 'after' }) {
  if (value === undefined) return <td className={`${styles.valueCell} ${styles.absent}`}>—</td>;
  const text = isScalar(value) ? formatValue(value) : formatPretty(value);
  return (
    <td className={`${styles.valueCell} ${styles[`cell_${op}`] ?? ''}`}>
      <code className={styles.valueCode}>{text}</code>
    </td>
  );
}

function ChangesTab({ resource }: { resource: ResourceDiff }) {
  if (resource.op === 'added' || resource.op === 'removed') {
    const side = resource.op === 'added' ? 'after' : 'before';
    return (
      <div className={styles.single}>
        <p className={styles.hint}>
          {resource.op === 'added' ? 'Every property is new.' : 'Every property is removed.'}
        </p>
        <PropertyTree value={resource[side]} changes={[]} side={side} />
      </div>
    );
  }
  if (resource.changes.length === 0) {
    return <p className={styles.hint}>No property differences.</p>;
  }
  return (
    <div className={styles.tableWrap}>
      <table className={styles.table}>
        <thead>
          <tr>
            <th scope="col">Property</th>
            <th scope="col">Before</th>
            <th scope="col">After</th>
          </tr>
        </thead>
        <tbody>
          {resource.changes.map((c, i) => (
            <tr key={`${c.path}-${i}`} className={styles[`row_${c.op}`] ?? ''}>
              <th scope="row" className={styles.pathCell}>
                <span className={`chip chip--${c.op}`} aria-label={c.op}>
                  {c.op === 'added' ? '+' : c.op === 'removed' ? '−' : '~'}
                </span>
                <code className={styles.path}>
                  {c.before_path && c.before_path !== c.path ? (
                    <>
                      <span className={styles.pathOld}>{c.before_path}</span> → {c.path}
                    </>
                  ) : (
                    c.path
                  )}
                </code>
              </th>
              <ValueCell value={c.before} op="before" />
              <ValueCell value={c.after} op="after" />
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function PropertiesTab({ resource }: { resource: ResourceDiff }) {
  return (
    <div className={styles.split}>
      <section className={styles.splitPane} aria-label="Before">
        <h3 className={styles.splitTitle}>
          Before{resource.renamed_from ? ` · ${resource.renamed_from}` : ''}
        </h3>
        <PropertyTree value={resource.before} changes={resource.changes} side="before" />
      </section>
      <section className={styles.splitPane} aria-label="After">
        <h3 className={styles.splitTitle}>
          After{resource.renamed_from ? ` · ${resource.id}` : ''}
        </h3>
        <PropertyTree value={resource.after} changes={resource.changes} side="after" />
      </section>
    </div>
  );
}

function RawTab({ resource }: { resource: ResourceDiff }) {
  const lines = useMemo(
    () => diffLines(formatPretty(resource.before), formatPretty(resource.after)),
    [resource],
  );
  const stats = lines.reduce(
    (acc, l) => {
      if (l.kind === 'add') acc.add++;
      if (l.kind === 'del') acc.del++;
      return acc;
    },
    { add: 0, del: 0 },
  );
  return (
    <div className={styles.rawWrap}>
      <p className={styles.hint}>
        Unified diff of the normalised property JSON ·{' '}
        <span className={styles.rawAdd}>+{stats.add}</span>{' '}
        <span className={styles.rawDel}>−{stats.del}</span>
      </p>
      <pre className={styles.raw}>
        {lines.map((l, i) => (
          <span key={i} className={styles[`line_${l.kind}`] ?? ''}>
            <span className={styles.lineSign} aria-hidden="true">
              {l.kind === 'add' ? '+' : l.kind === 'del' ? '-' : ' '}
            </span>
            {l.text}
            {'\n'}
          </span>
        ))}
      </pre>
    </div>
  );
}

export function DetailPane({ resource }: { resource: ResourceDiff | null }) {
  const [tab, setTab] = useState<Tab>('changes');

  if (!resource) {
    return (
      <section className={styles.detail} aria-label="Resource detail">
        <div className={styles.placeholder}>
          <p>Select a resource to see what changed.</p>
          <p className="muted">Use ↑ ↓ in the list, / to search.</p>
        </div>
      </section>
    );
  }

  return (
    <section className={styles.detail} aria-label={`Detail for ${resource.id}`}>
      <header className={styles.header}>
        <div className={styles.headline}>
          <OpChip op={resource.op} />
          <h2 className={styles.id}>
            {resource.renamed_from ? (
              <>
                <span className={styles.idOld}>{resource.renamed_from}</span>
                <span aria-hidden="true"> → </span>
                <span className="sr-only">renamed to</span>
                {resource.id}
              </>
            ) : (
              resource.id
            )}
          </h2>
          {resource.op !== 'unchanged' && <RiskBadge risk={resource.risk} />}
        </div>
        <p className={styles.kind}>
          <code>{resource.kind}</code>
          <span className={styles.sep}>·</span>
          {OP_LABEL[resource.op]}
          {resource.forces_replacement && (
            <span className={styles.replace}>forces replacement</span>
          )}
          {resource.changes.length > 0 && (
            <>
              <span className={styles.sep}>·</span>
              {pluralize(resource.changes.length, 'property change')}
            </>
          )}
        </p>
        {resource.reasons.length > 0 && (
          <ul className={styles.reasons}>
            {resource.reasons.map((reason) => (
              <li key={reason} className={styles[`reason_${resource.risk}`] ?? ''}>
                {reason}
              </li>
            ))}
          </ul>
        )}
      </header>

      <div className={styles.tabs} role="tablist" aria-label="Detail views">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            id={`tab-${t.id}`}
            aria-selected={tab === t.id}
            aria-controls={`panel-${t.id}`}
            tabIndex={tab === t.id ? 0 : -1}
            className={`${styles.tab}${tab === t.id ? ` ${styles.tabActive}` : ''}`}
            onClick={() => setTab(t.id)}
            onKeyDown={(e) => {
              const i = TABS.findIndex((x) => x.id === tab);
              if (e.key === 'ArrowRight') setTab((TABS[(i + 1) % TABS.length] as { id: Tab }).id);
              if (e.key === 'ArrowLeft')
                setTab((TABS[(i - 1 + TABS.length) % TABS.length] as { id: Tab }).id);
            }}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div
        id={`panel-${tab}`}
        role="tabpanel"
        aria-labelledby={`tab-${tab}`}
        className={styles.panel}
      >
        {tab === 'changes' && <ChangesTab resource={resource} />}
        {tab === 'properties' && <PropertiesTab resource={resource} />}
        {tab === 'raw' && <RawTab resource={resource} />}
      </div>
    </section>
  );
}
