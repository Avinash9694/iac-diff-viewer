import { forwardRef } from 'react';
import type { Summary } from '../engine/types';
import { OP_GLYPH, OP_LABEL, OP_ORDER, RISK_LABEL, RISK_ORDER } from '../engine/types';
import { DEFAULT_FILTERS, isFilterActive, toggleIn, type FilterState } from '../lib/filters';
import styles from './List.module.css';

interface FiltersProps {
  filters: FilterState;
  onChange: (next: FilterState) => void;
  kinds: string[];
  summary: Summary;
  shown: number;
}

export const Filters = forwardRef<HTMLInputElement, FiltersProps>(function Filters(
  { filters, onChange, kinds, summary, shown },
  searchRef,
) {
  const set = (patch: Partial<FilterState>) => onChange({ ...filters, ...patch });
  return (
    <div className={styles.filters}>
      <div className={styles.searchRow}>
        <label className={styles.search}>
          <span className="sr-only">Filter resources</span>
          <svg
            viewBox="0 0 20 20"
            width="16"
            height="16"
            aria-hidden="true"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
          >
            <circle cx="9" cy="9" r="5.5" />
            <path d="M13.5 13.5 17 17" />
          </svg>
          <input
            ref={searchRef}
            type="search"
            className={styles.searchInput}
            placeholder="Filter by id, type, path or reason  ( / )"
            value={filters.query}
            onChange={(e) => set({ query: e.target.value })}
          />
        </label>
        <label className={styles.kind}>
          <span className="sr-only">Resource type</span>
          <select
            className={`field ${styles.kindSelect}`}
            value={filters.kind}
            onChange={(e) => set({ kind: e.target.value })}
          >
            <option value="">All types</option>
            {kinds.map((k) => (
              <option key={k} value={k}>
                {k}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className={styles.toggleRow} role="group" aria-label="Filter by operation">
        {OP_ORDER.filter((op) => op !== 'unchanged').map((op) => (
          <button
            key={op}
            type="button"
            className="toggle"
            aria-pressed={filters.ops.includes(op)}
            onClick={() => set({ ops: toggleIn(filters.ops, op) })}
          >
            <span className={`chip chip--${op}`} aria-hidden="true">
              {OP_GLYPH[op]}
            </span>
            {OP_LABEL[op]}
            <span className={styles.toggleCount}>{summary.by_op[op]}</span>
          </button>
        ))}
        <button
          type="button"
          className="toggle"
          aria-pressed={filters.showUnchanged}
          onClick={() => set({ showUnchanged: !filters.showUnchanged })}
        >
          <span className="chip chip--unchanged" aria-hidden="true">
            =
          </span>
          Unchanged
          <span className={styles.toggleCount}>{summary.by_op.unchanged}</span>
        </button>
      </div>

      <div className={styles.toggleRow} role="group" aria-label="Filter by risk">
        {RISK_ORDER.map((risk) => (
          <button
            key={risk}
            type="button"
            className="toggle"
            aria-pressed={filters.risks.includes(risk)}
            onClick={() => set({ risks: toggleIn(filters.risks, risk) })}
          >
            <span className={`badge badge--${risk} badge--dot`} aria-hidden="true" />
            {RISK_LABEL[risk]}
            <span className={styles.toggleCount}>{summary.by_risk[risk]}</span>
          </button>
        ))}
        {isFilterActive(filters) && (
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() => onChange({ ...DEFAULT_FILTERS, showUnchanged: filters.showUnchanged })}
          >
            Reset
          </button>
        )}
      </div>

      <p className={styles.shown} aria-live="polite">
        Showing {shown} of {summary.total}
      </p>
    </div>
  );
});
