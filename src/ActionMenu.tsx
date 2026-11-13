import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Check, MoreHorizontal } from 'lucide-react';

export interface MenuAction { label: string; onClick: () => void; disabled?: boolean; danger?: boolean; checked?: boolean }
export default function ActionMenu({ label, actions, disabled = false, text, icon }: { label: string; actions: MenuAction[]; disabled?: boolean; text?: string; icon?: ReactNode }) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState({ top: 0, left: 0 });
  const trigger = useRef<HTMLButtonElement>(null); const menu = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (!open) return;
    const positionMenu = () => { const rect = trigger.current!.getBoundingClientRect(); setPosition({ left: Math.max(8, Math.min(rect.right - 244, window.innerWidth - 252)), top: Math.max(8, Math.min(rect.bottom + 6, window.innerHeight - actions.length * 36 - 20)) }); };
    positionMenu();
    const frame = requestAnimationFrame(() => menu.current?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus({ preventScroll: true }));
    const outside = (event: PointerEvent | FocusEvent) => { if (!menu.current?.contains(event.target as Node) && !trigger.current?.contains(event.target as Node)) setOpen(false); };
    const close = () => setOpen(false);
    const escape = (e: KeyboardEvent) => { if (e.key === 'Escape') { e.preventDefault(); setOpen(false); trigger.current?.focus(); } };
    document.addEventListener('pointerdown', outside); document.addEventListener('focusin', outside);
    document.addEventListener('keydown', escape);
    window.addEventListener('resize', close); window.addEventListener('scroll', positionMenu, true);
    return () => { cancelAnimationFrame(frame); document.removeEventListener('pointerdown', outside); document.removeEventListener('focusin', outside); document.removeEventListener('keydown', escape); window.removeEventListener('resize', close); window.removeEventListener('scroll', positionMenu, true); };
  }, [open, actions.length]);
  return <><button className={text ? 'nav-item menu-trigger appearance-trigger' : 'icon-button menu-trigger'} ref={trigger} aria-label={label} title={label} aria-haspopup="menu" aria-expanded={open} disabled={disabled} onClick={() => setOpen(v => !v)} onKeyDown={e => { if (e.key === 'ArrowDown') { e.preventDefault(); setOpen(true); } }}>{icon ?? <MoreHorizontal size={19} />}{text && <span>{text}</span>}</button>
    {open && createPortal(<div className="action-menu" role="menu" aria-label={label} ref={menu} style={position} onKeyDown={e => {
      if (e.key === 'Escape') { e.preventDefault(); setOpen(false); trigger.current?.focus(); }
      if (e.key === 'Tab') { setOpen(false); trigger.current?.focus(); return; }
      const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? []);
      const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
      if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) { e.preventDefault(); const next = e.key === 'Home' ? 0 : e.key === 'End' ? buttons.length - 1 : (index + (e.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length; buttons[next]?.focus({ preventScroll: true }); }
    }}>{actions.map(action => <button key={action.label} role={action.checked === undefined ? 'menuitem' : 'menuitemradio'} aria-checked={action.checked} className={action.danger ? 'danger' : ''} disabled={action.disabled} onClick={() => { setOpen(false); trigger.current?.focus(); action.onClick(); }}><span>{action.label}</span>{action.checked !== undefined && <Check size={16} className="menu-check" aria-hidden="true" style={{ visibility: action.checked ? 'visible' : 'hidden' }} />}</button>)}</div>, document.body)}
  </>;
}
