import { useEffect, useState } from 'react';
import { loadEngine, type EngineInfo } from '../engine/engine';

export type EngineState =
  | { status: 'loading' }
  | { status: 'ready'; info: EngineInfo }
  | { status: 'error'; message: string };

export function useEngine(): EngineState {
  const [state, setState] = useState<EngineState>({ status: 'loading' });

  useEffect(() => {
    let cancelled = false;
    loadEngine().then(
      (info) => {
        if (!cancelled) setState({ status: 'ready', info });
      },
      (err: unknown) => {
        if (!cancelled) {
          setState({
            status: 'error',
            message: err instanceof Error ? err.message : 'The diff engine failed to load',
          });
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  return state;
}
