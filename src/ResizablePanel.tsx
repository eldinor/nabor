import { useCallback, useEffect, useRef, useState } from 'react';

export function usePanelWidth(key: string, initial: number) {
  const [node, setNode] = useState<HTMLElement | null>(null);
  const ref = useCallback((value: HTMLElement | null) => setNode(value), []);
  const [preferred, setPreferred] = useState(() => { try { const n = Number(localStorage.getItem(key)); return n >= 360 && n <= 720 ? n : initial; } catch { return initial; } });
  const [available, setAvailable] = useState(720);
  useEffect(() => {
    const parent = node?.parentElement; if (!parent) return;
    const update = () => setAvailable(Math.max(360, Math.min(720, parent.clientWidth - 280 - 12)));
    update(); const observer = new ResizeObserver(update); observer.observe(parent); return () => observer.disconnect();
  }, [node]);
  const width = Math.min(preferred, available);
  const change = (value: number) => { const next = Math.round(Math.max(360, Math.min(value, available))); setPreferred(next); try { localStorage.setItem(key, String(next)); } catch { /* Layout preferences are optional. */ } };
  return { ref, width, max: available, change, reset: () => change(initial) };
}

export function ResizeHandle({ label, width, max, onChange, onReset }: { label: string; width: number; max: number; onChange: (value: number) => void; onReset: () => void }) {
  const start = useRef({ x: 0, width: 0 });
  const stop = () => document.documentElement.classList.remove('resizing-panels');
  useEffect(() => stop, []);
  return <div className="panel-resize-handle" role="separator" aria-label={label} aria-orientation="vertical" aria-valuemin={360} aria-valuemax={max} aria-valuenow={width} tabIndex={0} title="Drag to resize; arrow keys adjust width; double-click to reset" onDoubleClick={onReset} onPointerDown={e => { e.preventDefault(); start.current = { x: e.clientX, width }; e.currentTarget.setPointerCapture(e.pointerId); document.documentElement.classList.add('resizing-panels'); }} onPointerMove={e => { if (e.currentTarget.hasPointerCapture(e.pointerId)) onChange(start.current.width + start.current.x - e.clientX); }} onPointerUp={e => { if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId); stop(); }} onPointerCancel={stop} onLostPointerCapture={stop} onKeyDown={e => {
    if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) { e.preventDefault(); onChange(e.key === 'Home' ? 360 : e.key === 'End' ? max : width + (e.key === 'ArrowLeft' ? 1 : -1) * (e.shiftKey ? 40 : 12)); }
  }} />;
}
