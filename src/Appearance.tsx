import { useEffect, useState } from 'react';
import { Monitor, Moon, Sun } from 'lucide-react';
import ActionMenu from './ActionMenu';

type Appearance = 'system' | 'light' | 'dark';
const valid = (value: string | null | undefined): Appearance => value === 'light' || value === 'dark' ? value : 'system';
function readPreference() { try { return valid(localStorage.getItem('nabor.appearance')); } catch { return 'system' as const; } }
function apply(preference: Appearance) {
  const resolved = preference === 'system' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : preference;
  const root = document.documentElement;
  root.dataset.appearance = preference; root.dataset.theme = resolved; root.style.colorScheme = resolved;
  const background = getComputedStyle(root).getPropertyValue('--workspace').trim();
  root.style.backgroundColor = background;
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', background);
}
export default function AppearanceMenu({ onError }: { onError: (message: string) => void }) {
  const [preference, setPreference] = useState<Appearance>(readPreference);
  useEffect(() => {
    apply(preference);
    const system = matchMedia('(prefers-color-scheme: dark)');
    const change = () => { if (preference === 'system') apply('system'); };
    const storage = (e: StorageEvent) => { if (e.key === 'nabor.appearance' || e.key === null) setPreference(readPreference()); };
    system.addEventListener('change', change); window.addEventListener('storage', storage);
    return () => { system.removeEventListener('change', change); window.removeEventListener('storage', storage); };
  }, [preference]);
  const choose = (next: Appearance) => {
    apply(next); setPreference(next);
    try { localStorage.setItem('nabor.appearance', next); }
    catch { onError('Appearance changed for this session; the preference could not be saved.'); }
  };
  return <ActionMenu label="Appearance" text="Appearance" icon={preference === 'system' ? <Monitor size={18} /> : preference === 'dark' ? <Moon size={18} /> : <Sun size={18} />} actions={(['system', 'light', 'dark'] as const).map(value => ({ label: value[0].toUpperCase() + value.slice(1), checked: preference === value, onClick: () => choose(value) }))} />;
}
