import { useEffect, useRef, type KeyboardEvent } from 'react';
import type { ResourceDiff } from '../engine/types';
import { OP_LABEL } from '../engine/types';
import type { ResourceGroup } from '../lib/filters';
import { pluralize } from '../lib/format';
import { resourceKey } from '../lib/resource-key';
import { OpChip, RiskBadge } from './Badges';
import styles from './List.module.css';

interface ResourceListProps {
  groups: ResourceGroup[];
  selectedKey: string | null;
  onSelect: (key: string, source: 'pointer' | 'keyboard') => void;
}

export function ResourceList({ groups, selectedKey, onSelect }: ResourceListProps) {
  const flat = groups.flatMap((g) => g.items);
  const itemRefs = useRef(new Map<string, HTMLButtonElement>());
  const pendingFocus = useRef<string | null>(null);

  useEffect(() => {
    if (pendingFocus.current && pendingFocus.current === selectedKey) {
      itemRefs.current.get(selectedKey)?.focus();
      pendingFocus.current = null;
    }
  }, [selectedKey]);

  const moveTo = (index: number) => {
    const target = flat[Math.max(0, Math.min(flat.length - 1, index))];
    if (!target) return;
    const key = resourceKey(target);
    pendingFocus.current = key;
    onSelect(key, 'keyboard');
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (flat.length === 0) return;
    const current = flat.findIndex((r) => resourceKey(r) === selectedKey);
    switch (e.key) {
      case 'ArrowDown':
      case 'j':
        e.preventDefault();
        moveTo(current + 1);
        break;
      case 'ArrowUp':
      case 'k':
        e.preventDefault();
        moveTo(current - 1);
        break;
      case 'Home':
        e.preventDefault();
        moveTo(0);
        break;
      case 'End':
        e.preventDefault();
        moveTo(flat.length - 1);
        break;
      default:
        break;
    }
  };

  if (flat.length === 0) {
    return (
      <div className={styles.empty}>
        <p>No resources match the current filters.</p>
      </div>
    );
  }

  const activeKey =
    selectedKey && flat.some((r) => resourceKey(r) === selectedKey)
      ? selectedKey
      : resourceKey(flat[0] as ResourceDiff);

  return (
    <div
      className={styles.list}
      role="listbox"
      aria-label="Changed resources grouped by risk"
      aria-activedescendant={`res-${activeKey}`}
      tabIndex={-1}
      onKeyDown={onKeyDown}
    >
      {groups.map((group) => (
        <section
          key={group.key}
          className={styles.group}
          aria-label={`${group.label} (${pluralize(group.items.length, 'resource')})`}
        >
          <h2 className={`${styles.groupTitle} ${styles[`group_${group.key}`] ?? ''}`}>
            <span>{group.label}</span>
            <span className={styles.groupCount}>{group.items.length}</span>
          </h2>
          <ul className={styles.items}>
            {group.items.map((r) => {
              const key = resourceKey(r);
              const selected = key === activeKey;
              return (
                <li key={key}>
                  <button
                    id={`res-${key}`}
                    ref={(el) => {
                      if (el) itemRefs.current.set(key, el);
                      else itemRefs.current.delete(key);
                    }}
                    type="button"
                    role="option"
                    aria-selected={selected}
                    tabIndex={selected ? 0 : -1}
                    className={`${styles.item}${selected ? ` ${styles.itemSelected}` : ''}`}
                    onClick={() => onSelect(key, 'pointer')}
                  >
                    <OpChip
                      op={r.op}
                      title={r.forces_replacement ? `${OP_LABEL[r.op]} (replaces)` : OP_LABEL[r.op]}
                    />
                    <span className={styles.itemBody}>
                      <span className={styles.itemId}>
                        {r.op === 'renamed' && r.renamed_from ? (
                          <>
                            <span className={styles.renamedFrom}>{r.renamed_from}</span>
                            <span aria-hidden="true"> → </span>
                            <span className="sr-only"> renamed to </span>
                            {r.id}
                          </>
                        ) : (
                          r.id
                        )}
                      </span>
                      <span className={styles.itemKind}>{r.kind}</span>
                    </span>
                    {r.op !== 'unchanged' && (
                      <span className={styles.itemMeta}>
                        <RiskBadge risk={r.risk} dot={false} />
                        {r.changes.length > 0 && (
                          <span className={styles.changeCount}>{r.changes.length}</span>
                        )}
                      </span>
                    )}
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </div>
  );
}
