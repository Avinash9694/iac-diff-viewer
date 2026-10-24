import type { Op, Risk } from '../engine/types';
import { OP_GLYPH, OP_LABEL, RISK_LABEL } from '../engine/types';

export function RiskBadge({ risk, dot = true }: { risk: Risk; dot?: boolean }) {
  return (
    <span className={`badge badge--${risk}${dot ? ' badge--dot' : ''}`}>{RISK_LABEL[risk]}</span>
  );
}

export function OpChip({ op, title }: { op: Op; title?: string }) {
  return (
    <span
      className={`chip chip--${op}`}
      aria-label={title ?? OP_LABEL[op]}
      title={title ?? OP_LABEL[op]}
    >
      <span aria-hidden="true">{OP_GLYPH[op]}</span>
    </span>
  );
}
