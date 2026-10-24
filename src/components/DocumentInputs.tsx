import { useId, useRef, useState, type ChangeEvent, type DragEvent } from 'react';
import type { Detected, Side } from '../engine/types';
import { pluralize } from '../lib/format';
import styles from './Inputs.module.css';

export type SideStatus =
  | { kind: 'empty' }
  | { kind: 'ok'; detected: Detected }
  | { kind: 'error'; message: string; location: string | null };

interface PaneProps {
  side: Side;
  value: string;
  status: SideStatus;
  onChange: (text: string) => void;
  note?: string;
}

const MAX_FILE_BYTES = 8 * 1024 * 1024;

function DocumentPane({ side, value, status, onChange, note }: PaneProps) {
  const id = useId();
  const fileRef = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const [fileError, setFileError] = useState<string | null>(null);
  const label = side === 'before' ? 'Before' : 'After';

  const readFile = (file: File | undefined) => {
    if (!file) return;
    if (file.size > MAX_FILE_BYTES) {
      setFileError(`${file.name} is larger than 8 MB`);
      return;
    }
    setFileError(null);
    file
      .text()
      .then((text) => onChange(text))
      .catch(() => setFileError(`Could not read ${file.name}`));
  };

  const onDrop = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setDragging(false);
    readFile(e.dataTransfer.files[0]);
  };

  const onFileInput = (e: ChangeEvent<HTMLInputElement>) => {
    readFile(e.target.files?.[0]);
    e.target.value = '';
  };

  return (
    <div
      className={`${styles.pane}${dragging ? ` ${styles.dragging}` : ''}`}
      onDragOver={(e) => {
        e.preventDefault();
        setDragging(true);
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={onDrop}
    >
      <div className={styles.paneHeader}>
        <label htmlFor={id} className={styles.paneLabel}>
          {label}
        </label>
        <div className={styles.paneMeta}>
          {status.kind === 'ok' && (
            <span className={styles.detected}>
              {status.detected.label} · {pluralize(status.detected.resource_count, 'resource')}
            </span>
          )}
          {status.kind === 'empty' && <span className="muted">Paste or drop a document</span>}
          <button type="button" className="btn btn--sm" onClick={() => fileRef.current?.click()}>
            Open file…
          </button>
          <input
            ref={fileRef}
            type="file"
            accept=".json,.yaml,.yml,.tfstate,.template,application/json,text/yaml"
            className="sr-only"
            tabIndex={-1}
            aria-hidden="true"
            onChange={onFileInput}
          />
        </div>
      </div>
      <textarea
        id={id}
        className={styles.editor}
        value={value}
        spellCheck={false}
        placeholder={
          side === 'before'
            ? 'CloudFormation template (JSON/YAML), Terraform plan or state JSON'
            : 'The newer version of the same document (leave empty when the before side is a Terraform plan)'
        }
        onChange={(e) => onChange(e.target.value)}
        aria-invalid={status.kind === 'error'}
        aria-describedby={`${id}-status`}
      />
      <p id={`${id}-status`} className={styles.paneStatus} role="status">
        {fileError ? (
          <span className={styles.error}>{fileError}</span>
        ) : status.kind === 'error' ? (
          <span className={styles.error}>
            {status.message}
            {status.location ? ` — ${status.location}` : ''}
          </span>
        ) : (
          (note ?? '')
        )}
      </p>
    </div>
  );
}

interface DocumentInputsProps {
  before: string;
  after: string;
  beforeStatus: SideStatus;
  afterStatus: SideStatus;
  onBeforeChange: (text: string) => void;
  onAfterChange: (text: string) => void;
  afterNote?: string;
}

export function DocumentInputs(props: DocumentInputsProps) {
  const { before, after, beforeStatus, afterStatus, onBeforeChange, onAfterChange, afterNote } =
    props;
  return (
    <section id="document-editors" className={styles.inputs} aria-label="Documents to compare">
      <DocumentPane side="before" value={before} status={beforeStatus} onChange={onBeforeChange} />
      <DocumentPane
        side="after"
        value={after}
        status={afterStatus}
        onChange={onAfterChange}
        {...(afterNote !== undefined ? { note: afterNote } : {})}
      />
    </section>
  );
}
