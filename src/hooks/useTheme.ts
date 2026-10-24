import { useCallback, useEffect, useState } from 'react';

export type ThemeChoice = 'system' | 'dark' | 'light';
export type Theme = 'dark' | 'light';

const STORAGE_KEY = 'iac-diff-theme';

function readStored(): ThemeChoice {
  try {
    // `?theme=dark|light` wins for the session (shareable links, screenshots); storage otherwise.
    const fromUrl = new URLSearchParams(window.location.search).get('theme');
    const v = fromUrl ?? localStorage.getItem(STORAGE_KEY);
    return v === 'dark' || v === 'light' ? v : 'system';
  } catch {
    return 'system';
  }
}

function systemTheme(): Theme {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return 'dark';
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

export function useTheme(): { choice: ThemeChoice; theme: Theme; cycle: () => void } {
  const [choice, setChoice] = useState<ThemeChoice>(readStored);
  const [system, setSystem] = useState<Theme>(systemTheme);

  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    const mq = window.matchMedia('(prefers-color-scheme: light)');
    const onChange = () => setSystem(mq.matches ? 'light' : 'dark');
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, []);

  const theme: Theme = choice === 'system' ? system : choice;

  useEffect(() => {
    const root = document.documentElement;
    if (choice === 'system') delete root.dataset.theme;
    else root.dataset.theme = choice;
    try {
      if (choice === 'system') localStorage.removeItem(STORAGE_KEY);
      else localStorage.setItem(STORAGE_KEY, choice);
    } catch {
      /* storage unavailable */
    }
  }, [choice]);

  // Toggle between the two explicit themes; system preference is the untouched default.
  const cycle = useCallback(() => {
    setChoice(theme === 'dark' ? 'light' : 'dark');
  }, [theme]);

  return { choice, theme, cycle };
}
