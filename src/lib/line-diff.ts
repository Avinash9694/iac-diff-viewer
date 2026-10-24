export type LineKind = 'same' | 'add' | 'del';

export interface DiffLine {
  kind: LineKind;
  text: string;
}

/** Past this many cells the quadratic LCS is skipped for a plain replace. */
const CELL_BUDGET = 4_000_000;

/** Unified line diff of two texts via longest common subsequence. */
export function diffLines(before: string, after: string): DiffLine[] {
  const a = before === '' ? [] : before.split('\n');
  const b = after === '' ? [] : after.split('\n');
  if (a.length * b.length > CELL_BUDGET) {
    return [
      ...a.map((text) => ({ kind: 'del' as const, text })),
      ...b.map((text) => ({ kind: 'add' as const, text })),
    ];
  }
  const n = a.length;
  const m = b.length;
  const table: Uint32Array[] = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    const row = table[i] as Uint32Array;
    const next = table[i + 1] as Uint32Array;
    for (let j = m - 1; j >= 0; j--) {
      row[j] =
        a[i] === b[j]
          ? (next[j + 1] as number) + 1
          : Math.max(next[j] as number, row[j + 1] as number);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      out.push({ kind: 'same', text: a[i] as string });
      i++;
      j++;
    } else if (
      ((table[i + 1] as Uint32Array)[j] as number) >= ((table[i] as Uint32Array)[j + 1] as number)
    ) {
      out.push({ kind: 'del', text: a[i] as string });
      i++;
    } else {
      out.push({ kind: 'add', text: b[j] as string });
      j++;
    }
  }
  while (i < n) out.push({ kind: 'del', text: a[i++] as string });
  while (j < m) out.push({ kind: 'add', text: b[j++] as string });
  return out;
}
