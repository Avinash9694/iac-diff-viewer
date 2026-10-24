import type { ChangeOp, JsonValue, PropertyChange, Side } from '../engine/types';
import { formatValue } from '../lib/format';
import styles from './Detail.module.css';

/** Path → change op, keyed on the side's own paths (Rust emits `before_path` when they differ). */
function changeIndex(changes: readonly PropertyChange[], side: Side): Map<string, ChangeOp> {
  const map = new Map<string, ChangeOp>();
  for (const c of changes) {
    if (side === 'before' && c.op === 'added') continue;
    if (side === 'after' && c.op === 'removed') continue;
    map.set(side === 'before' ? (c.before_path ?? c.path) : c.path, c.op);
  }
  return map;
}

function join(path: string, key: string): string {
  return path === '' ? key : `${path}.${key}`;
}

interface NodeProps {
  name: string | null;
  value: JsonValue;
  path: string;
  index: Map<string, ChangeOp>;
  inherited: ChangeOp | null;
  depth: number;
}

function Node({ name, value, path, index, inherited, depth }: NodeProps) {
  const own = index.get(path) ?? null;
  const mark = own ?? inherited;
  const cls = mark ? ` ${styles[`tree_${mark}`] ?? ''}` : '';
  const label = name !== null ? <span className={styles.treeKey}>{name}</span> : null;

  if (value !== null && typeof value === 'object') {
    const entries: Array<[string, JsonValue, string]> = Array.isArray(value)
      ? value.map((v, i) => [`[${i}]`, v, `${path}[${i}]`])
      : Object.keys(value)
          .sort()
          .map((k) => [k, (value as Record<string, JsonValue>)[k] as JsonValue, join(path, k)]);
    const open = Array.isArray(value) ? '[' : '{';
    const close = Array.isArray(value) ? ']' : '}';
    if (entries.length === 0) {
      return (
        <li
          className={`${styles.treeRow}${cls}`}
          style={{ '--depth': depth } as React.CSSProperties}
        >
          {label}
          <span className={styles.treeValue}>
            {open}
            {close}
          </span>
        </li>
      );
    }
    return (
      <li className={cls} style={{ '--depth': depth } as React.CSSProperties}>
        <details open={depth < 3 || mark !== null}>
          <summary className={styles.treeRow}>
            {label}
            <span className={styles.treePunct}>
              {open}
              <span className={styles.treeCount}>{entries.length}</span>
              {close}
            </span>
          </summary>
          <ul className={styles.treeList}>
            {entries.map(([k, v, p]) => (
              <Node
                key={p}
                name={k}
                value={v}
                path={p}
                index={index}
                inherited={mark}
                depth={depth + 1}
              />
            ))}
          </ul>
        </details>
      </li>
    );
  }

  return (
    <li className={`${styles.treeRow}${cls}`} style={{ '--depth': depth } as React.CSSProperties}>
      {label}
      <span
        className={`${styles.treeValue} ${styles[`type_${value === null ? 'null' : typeof value}`] ?? ''}`}
      >
        {formatValue(value)}
      </span>
    </li>
  );
}

interface PropertyTreeProps {
  value: JsonValue | undefined;
  changes: readonly PropertyChange[];
  side: Side;
}

export function PropertyTree({ value, changes, side }: PropertyTreeProps) {
  if (value === undefined) {
    return <p className={styles.treeAbsent}>Resource does not exist on this side.</p>;
  }
  const index = changeIndex(changes, side);
  return (
    <ul className={`${styles.treeList} ${styles.treeRoot}`}>
      <Node name={null} value={value} path="" index={index} inherited={null} depth={0} />
    </ul>
  );
}
