import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import styles from './App.module.css';
import { DetailPane } from './components/DetailPane';
import { DocumentInputs, type SideStatus } from './components/DocumentInputs';
import { Filters } from './components/Filters';
import { Footer } from './components/Footer';
import { ResourceList } from './components/ResourceList';
import { SummaryStrip } from './components/SummaryStrip';
import { TopBar } from './components/TopBar';
import { EngineError, detectDocument, diffDocuments } from './engine/engine';
import type { DiffReport } from './engine/types';
import { RISK_LABEL } from './engine/types';
import { useDebouncedValue } from './hooks/useDebouncedValue';
import { useEngine } from './hooks/useEngine';
import { useTheme } from './hooks/useTheme';
import {
  applyFilters,
  DEFAULT_FILTERS,
  groupByRisk,
  uniqueKinds,
  type FilterState,
} from './lib/filters';
import { buildMarkdownReport, reportFilename } from './lib/markdown-report';
import { resourceKey } from './lib/resource-key';
import { DEFAULT_SAMPLE_ID, findSample } from './samples';

type DiffState =
  | { status: 'idle' }
  | { status: 'ok'; report: DiffReport }
  | { status: 'error'; error: EngineError };

function sideStatus(text: string): SideStatus {
  if (text.trim() === '') return { kind: 'empty' };
  try {
    return { kind: 'ok', detected: detectDocument(text) };
  } catch (err: unknown) {
    const e = err instanceof EngineError ? err : new EngineError({ error: String(err) });
    return { kind: 'error', message: e.message, location: e.location };
  }
}

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
}

export function App() {
  const engine = useEngine();
  const { theme, cycle: toggleTheme } = useTheme();

  const initial = findSample(DEFAULT_SAMPLE_ID);
  const [sampleId, setSampleId] = useState(initial ? initial.id : '');
  const [before, setBefore] = useState(initial?.before ?? '');
  const [after, setAfter] = useState(initial?.after ?? '');
  const [editorsOpen, setEditorsOpen] = useState(!initial);
  const [filters, setFilters] = useState<FilterState>(DEFAULT_FILTERS);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const debouncedBefore = useDebouncedValue(before, 250);
  const debouncedAfter = useDebouncedValue(after, 250);
  const ready = engine.status === 'ready';

  const diff: DiffState = useMemo(() => {
    if (!ready) return { status: 'idle' };
    if (debouncedBefore.trim() === '' && debouncedAfter.trim() === '') return { status: 'idle' };
    try {
      return { status: 'ok', report: diffDocuments(debouncedBefore, debouncedAfter) };
    } catch (err: unknown) {
      return {
        status: 'error',
        error: err instanceof EngineError ? err : new EngineError({ error: String(err) }),
      };
    }
  }, [ready, debouncedBefore, debouncedAfter]);

  const beforeStatus = useMemo<SideStatus>(
    () => (ready ? sideStatus(debouncedBefore) : { kind: 'empty' }),
    [ready, debouncedBefore],
  );
  const afterStatus = useMemo<SideStatus>(
    () => (ready ? sideStatus(debouncedAfter) : { kind: 'empty' }),
    [ready, debouncedAfter],
  );
  const planOnly =
    beforeStatus.kind === 'ok' &&
    beforeStatus.detected.kind === 'terraform-plan' &&
    afterStatus.kind === 'empty';

  const report = diff.status === 'ok' ? diff.report : null;
  const filtered = useMemo(
    () => (report ? applyFilters(report.resources, filters) : []),
    [report, filters],
  );
  const groups = useMemo(() => groupByRisk(filtered), [filtered]);
  const kinds = useMemo(() => (report ? uniqueKinds(report.resources) : []), [report]);

  const selected = useMemo(() => {
    if (filtered.length === 0) return null;
    return filtered.find((r) => resourceKey(r) === selectedKey) ?? filtered[0] ?? null;
  }, [filtered, selectedKey]);

  const loadSample = useCallback((id: string) => {
    setSampleId(id);
    const sample = findSample(id);
    if (sample) {
      setBefore(sample.before);
      setAfter(sample.after);
      setEditorsOpen(false);
    } else {
      setBefore('');
      setAfter('');
      setEditorsOpen(true);
    }
    setSelectedKey(null);
    setFilters(DEFAULT_FILTERS);
  }, []);

  const onEdit = (side: 'before' | 'after') => (text: string) => {
    setSampleId('');
    if (side === 'before') setBefore(text);
    else setAfter(text);
  };

  const swap = () => {
    setBefore(after);
    setAfter(before);
    setSelectedKey(null);
  };

  const announce = (message: string) => {
    setToast(message);
    window.setTimeout(() => setToast((t) => (t === message ? null : t)), 2500);
  };

  const copyMarkdown = async () => {
    if (!report) return;
    try {
      await navigator.clipboard.writeText(buildMarkdownReport(report));
      announce('Markdown report copied to the clipboard');
    } catch {
      announce('Clipboard is not available in this browser');
    }
  };

  const downloadMarkdown = () => {
    if (!report) return;
    const blob = new Blob([buildMarkdownReport(report)], { type: 'text/markdown;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = reportFilename(report);
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    announce(`Downloaded ${a.download}`);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === '/' && !isTypingTarget(e.target) && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        searchRef.current?.focus();
      } else if (e.key === 'Escape' && editorsOpen && !isTypingTarget(e.target)) {
        setEditorsOpen(false);
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [editorsOpen]);

  const liveMessage =
    engine.status === 'loading'
      ? 'Loading the diff engine…'
      : engine.status === 'error'
        ? `Engine failed: ${engine.message}`
        : diff.status === 'ok'
          ? `Diff ready: ${diff.report.summary.changed} of ${diff.report.summary.total} resources changed, overall risk ${RISK_LABEL[diff.report.summary.overall_risk]}`
          : diff.status === 'error'
            ? `Cannot diff: ${diff.error.message}`
            : 'Paste two documents to compare';

  return (
    <div className={styles.app}>
      <TopBar
        sampleId={sampleId}
        onSelectSample={loadSample}
        onSwap={swap}
        editorsOpen={editorsOpen}
        onToggleEditors={() => setEditorsOpen((o) => !o)}
        canExport={report !== null}
        onCopyMarkdown={() => void copyMarkdown()}
        onDownloadMarkdown={downloadMarkdown}
        theme={theme}
        onToggleTheme={toggleTheme}
      />

      <p className="sr-only" role="status" aria-live="polite">
        {liveMessage}
      </p>
      {toast && (
        <div className={styles.toast} role="status">
          {toast}
        </div>
      )}

      {editorsOpen && (
        <DocumentInputs
          before={before}
          after={after}
          beforeStatus={beforeStatus}
          afterStatus={afterStatus}
          onBeforeChange={onEdit('before')}
          onAfterChange={onEdit('after')}
          {...(planOnly
            ? {
                afterNote:
                  'Terraform plan detected: its prior state is diffed against its planned values.',
              }
            : {})}
        />
      )}

      {engine.status === 'loading' && (
        <div className={styles.notice} aria-hidden="true">
          <span className={styles.spinner} /> Loading the Rust diff engine…
        </div>
      )}
      {engine.status === 'error' && (
        <div className={`${styles.notice} ${styles.noticeError}`} role="alert">
          The WebAssembly engine could not be loaded: {engine.message}. Reload the page to try
          again.
        </div>
      )}
      {diff.status === 'error' && (
        <div className={`${styles.notice} ${styles.noticeError}`} role="alert">
          {diff.error.side ? `${diff.error.side === 'before' ? 'Before' : 'After'} document: ` : ''}
          {diff.error.message}
          {diff.error.location ? ` (${diff.error.location})` : ''}
          {!editorsOpen && (
            <button type="button" className="btn btn--sm" onClick={() => setEditorsOpen(true)}>
              Open editors
            </button>
          )}
        </div>
      )}
      {ready && diff.status === 'idle' && (
        <div className={styles.emptyState}>
          <h2>Nothing to compare yet</h2>
          <p className="muted">
            Paste a CloudFormation template, a `terraform show -json` plan or two state files, or
            pick a sample from the top bar.
          </p>
        </div>
      )}

      {report && (
        <>
          <SummaryStrip report={report} />
          <main className={styles.workspace}>
            <aside className={`${styles.listPane} card`} aria-label="Resources">
              <Filters
                ref={searchRef}
                filters={filters}
                onChange={setFilters}
                kinds={kinds}
                summary={report.summary}
                shown={filtered.length}
              />
              <ResourceList
                groups={groups}
                selectedKey={selected ? resourceKey(selected) : null}
                onSelect={setSelectedKey}
              />
            </aside>
            <div className={`${styles.detailPane} card`}>
              <DetailPane resource={selected} />
            </div>
          </main>
        </>
      )}

      <Footer engine={engine} />
    </div>
  );
}
