import type { EngineState } from '../hooks/useEngine';
import { formatBytes } from '../lib/format';
import styles from './Footer.module.css';

export function Footer({ engine }: { engine: EngineState }) {
  const size =
    engine.status === 'ready'
      ? `${formatBytes(engine.info.wasmBytes)} · v${engine.info.version}`
      : engine.status === 'loading'
        ? 'loading…'
        : 'failed to load';
  return (
    <footer className={styles.footer}>
      <p className={styles.engine}>
        <span className={styles.engineDot} aria-hidden="true" data-status={engine.status} />
        Engine: Rust → WebAssembly, runs in your browser
        <span className={styles.size}>({size})</span>
      </p>
      <p className={styles.credit}>
        Built by{' '}
        <a href="https://avinash9694.github.io" target="_blank" rel="noreferrer">
          Avinash Sahu
        </a>
        <a
          className={styles.iconLink}
          href="https://github.com/Avinash9694"
          target="_blank"
          rel="noreferrer"
          aria-label="GitHub profile"
        >
          <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="currentColor">
            <path d="M12 .5a12 12 0 0 0-3.8 23.4c.6.1.8-.3.8-.6v-2.2c-3.3.7-4-1.4-4-1.4-.6-1.4-1.4-1.8-1.4-1.8-1.1-.7.1-.7.1-.7 1.2.1 1.8 1.2 1.8 1.2 1.1 1.8 2.8 1.3 3.5 1 .1-.8.4-1.3.8-1.6-2.7-.3-5.5-1.3-5.5-5.9 0-1.3.5-2.4 1.2-3.2-.1-.3-.5-1.5.1-3.2 0 0 1-.3 3.3 1.2a11.5 11.5 0 0 1 6 0c2.3-1.5 3.3-1.2 3.3-1.2.6 1.7.2 2.9.1 3.2.8.8 1.2 1.9 1.2 3.2 0 4.6-2.8 5.6-5.5 5.9.4.4.8 1.1.8 2.2v3.3c0 .3.2.7.8.6A12 12 0 0 0 12 .5z" />
          </svg>
        </a>
        <a
          className={styles.iconLink}
          href="https://www.linkedin.com/in/A-v-i-n-a-s-h/"
          target="_blank"
          rel="noreferrer"
          aria-label="LinkedIn profile"
        >
          <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="currentColor">
            <path d="M20.5 2h-17A1.5 1.5 0 0 0 2 3.5v17A1.5 1.5 0 0 0 3.5 22h17a1.5 1.5 0 0 0 1.5-1.5v-17A1.5 1.5 0 0 0 20.5 2zM8 19H5v-9h3zM6.5 8.3A1.8 1.8 0 1 1 8.3 6.5 1.8 1.8 0 0 1 6.5 8.3zM19 19h-3v-4.7c0-1.4-.6-2.1-1.6-2.1-1.2 0-1.9.8-1.9 2.1V19h-3v-9h2.9v1.3a3.4 3.4 0 0 1 3-1.6c2.1 0 3.6 1.3 3.6 4z" />
          </svg>
        </a>
      </p>
    </footer>
  );
}
