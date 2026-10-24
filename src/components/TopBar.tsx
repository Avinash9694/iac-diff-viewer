import type { Theme } from '../hooks/useTheme';
import { SAMPLES } from '../samples';
import styles from './TopBar.module.css';

interface TopBarProps {
  sampleId: string;
  onSelectSample: (id: string) => void;
  onSwap: () => void;
  editorsOpen: boolean;
  onToggleEditors: () => void;
  canExport: boolean;
  onCopyMarkdown: () => void;
  onDownloadMarkdown: () => void;
  theme: Theme;
  onToggleTheme: () => void;
}

export function TopBar(props: TopBarProps) {
  const {
    sampleId,
    onSelectSample,
    onSwap,
    editorsOpen,
    onToggleEditors,
    canExport,
    onCopyMarkdown,
    onDownloadMarkdown,
    theme,
    onToggleTheme,
  } = props;

  return (
    <header className={styles.bar}>
      <div className={styles.brand}>
        <svg viewBox="0 0 64 64" width="28" height="28" aria-hidden="true" className={styles.logo}>
          <rect width="64" height="64" rx="14" fill="var(--surface-3)" />
          <path d="M14 20h16v4H14zM14 30h22v4H14zM14 40h12v4H14z" fill="var(--text-muted)" />
          <path d="M34 20h16v4H34z" fill="var(--op-removed)" />
          <path d="M40 30h10v4H40zM30 40h20v4H30z" fill="var(--op-added)" />
          <circle cx="50" cy="14" r="6" fill="var(--accent)" />
        </svg>
        <div>
          <h1 className={styles.title}>IaC Diff Viewer</h1>
          <p className={styles.subtitle}>Resource-level CloudFormation &amp; Terraform diff</p>
        </div>
      </div>

      <div className={styles.controls}>
        <label className={styles.sampleLabel}>
          <span className="sr-only">Load a sample</span>
          <select
            className={`field ${styles.sample}`}
            value={sampleId}
            onChange={(e) => onSelectSample(e.target.value)}
          >
            <option value="">Custom documents</option>
            {SAMPLES.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="btn"
          aria-pressed={editorsOpen}
          aria-controls="document-editors"
          onClick={onToggleEditors}
        >
          <svg
            viewBox="0 0 20 20"
            width="16"
            height="16"
            aria-hidden="true"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
          >
            <path d="M13.5 3.5l3 3L7 16H4v-3z" />
          </svg>
          {editorsOpen ? 'Hide documents' : 'Edit documents'}
        </button>
        <button type="button" className="btn" onClick={onSwap} title="Swap before and after">
          <svg
            viewBox="0 0 20 20"
            width="16"
            height="16"
            aria-hidden="true"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
          >
            <path d="M4 7h11m0 0-3-3m3 3-3 3M16 13H5m0 0 3 3m-3-3 3-3" />
          </svg>
          Swap
        </button>
        <div className={styles.group} role="group" aria-label="Export report">
          <button type="button" className="btn" disabled={!canExport} onClick={onCopyMarkdown}>
            Copy Markdown
          </button>
          <button
            type="button"
            className="btn btn--primary"
            disabled={!canExport}
            onClick={onDownloadMarkdown}
          >
            Download .md
          </button>
        </div>
      </div>

      <button
        type="button"
        className={`btn btn--icon btn--ghost ${styles.themeBtn}`}
        onClick={onToggleTheme}
        aria-label={theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
        title={theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
      >
        {theme === 'dark' ? (
          <svg
            viewBox="0 0 20 20"
            width="18"
            height="18"
            aria-hidden="true"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
          >
            <circle cx="10" cy="10" r="3.5" />
            <path d="M10 2v2M10 16v2M2 10h2M16 10h2M4.3 4.3l1.4 1.4M14.3 14.3l1.4 1.4M4.3 15.7l1.4-1.4M14.3 5.7l1.4-1.4" />
          </svg>
        ) : (
          <svg
            viewBox="0 0 20 20"
            width="18"
            height="18"
            aria-hidden="true"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
          >
            <path d="M16.5 12.5A7 7 0 0 1 7.5 3.5a7 7 0 1 0 9 9z" />
          </svg>
        )}
      </button>
    </header>
  );
}
