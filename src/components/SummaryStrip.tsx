import type { DiffReport } from '../engine/types';
import { OP_ORDER, RISK_LABEL, RISK_ORDER } from '../engine/types';
import { pluralize } from '../lib/format';
import { OpChip, RiskBadge } from './Badges';
import styles from './Summary.module.css';

export function SummaryStrip({ report }: { report: DiffReport }) {
  const { summary } = report;
  return (
    <section className={styles.strip} aria-label="Diff summary">
      <div className={styles.overall}>
        <span className={styles.overallLabel}>Overall risk</span>
        <span className={`${styles.overallBadge} badge badge--${summary.overall_risk} badge--dot`}>
          {RISK_LABEL[summary.overall_risk]}
        </span>
      </div>

      <dl className={styles.docs}>
        <div>
          <dt>Before</dt>
          <dd>
            {report.before.label} · {pluralize(report.before.resource_count, 'resource')}
          </dd>
        </div>
        <div>
          <dt>After</dt>
          <dd>
            {report.after.label} · {pluralize(report.after.resource_count, 'resource')}
          </dd>
        </div>
      </dl>

      <ul className={styles.counts} aria-label="Changes by operation">
        {OP_ORDER.map((op) => (
          <li key={op} className={styles.count} data-zero={summary.by_op[op] === 0}>
            <OpChip op={op} />
            <span className={styles.countValue}>{summary.by_op[op]}</span>
            <span className={styles.countLabel}>{op}</span>
          </li>
        ))}
      </ul>

      <ul className={styles.counts} aria-label="Changed resources by risk">
        {RISK_ORDER.map((risk) => (
          <li key={risk} className={styles.count} data-zero={summary.by_risk[risk] === 0}>
            <RiskBadge risk={risk} />
            <span className={styles.countValue}>{summary.by_risk[risk]}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
