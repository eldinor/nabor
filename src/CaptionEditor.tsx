import { useRef, useState } from 'react';
import { LoaderCircle, X } from 'lucide-react';
import { agentRequest, batchRequest } from './api';
import type { CaptionDraft, Item } from './types';

interface Version { id: string; caption: string; label: string }
export default function CaptionEditor({ initial, onSaved, onClose, onBusy }: { initial: CaptionDraft; onSaved: (item: Item) => Promise<void>; onClose: () => void; onBusy: (busy: boolean) => void }) {
  const [caption, setCaption] = useState(initial.caption);
  const [instructions, setInstructions] = useState('');
  const [busy, setBusy] = useState(false); const [error, setError] = useState('');
  const counter = useRef(0);
  const [versions, setVersions] = useState<Version[]>(() => [
    ...(initial.history ?? []).map((v, i) => ({ id: `saved-${i}`, caption: v.caption, label: `Saved ${new Date(v.savedAt * 1000).toLocaleString()}` })),
    { id: 'starting', caption: initial.caption, label: initial.replaceExisting ? 'Current saved caption' : 'First draft' },
  ]);
  const [selected, setSelected] = useState('');
  const draftRevision = useRef(initial.draftRevision);
  const checkpoint = (text: string, label: string) => {
    const id = `draft-${++counter.current}`;
    setVersions(old => old.at(-1)?.caption === text ? old : [...old, { id, caption: text, label }].slice(-40));
  };
  const run = async (operation: () => Promise<void>) => {
    setBusy(true); onBusy(true); setError('');
    try { await operation(); } catch (e) { setError(String(e)); }
    finally { setBusy(false); onBusy(false); }
  };
  const revise = () => run(async () => {
    checkpoint(caption, 'Before revision');
    const next = await agentRequest<CaptionDraft>('revise', { itemId: initial.itemId, expectedHash: initial.expectedHash, currentCaption: caption, instructions, draftRevision: draftRevision.current ?? null });
    draftRevision.current=next.draftRevision;
    setCaption(next.caption); checkpoint(next.caption, 'Agent revision'); setInstructions(''); setSelected('');
  });
  const restore = () => {
    const version = versions.find(v => v.id === selected); if (!version) return;
    checkpoint(caption, 'Before restore'); setCaption(version.caption); setSelected(''); setError('');
  };
  return <div className="modal-backdrop"><section className="modal caption-editor" role="dialog" aria-modal="true" aria-label="Review caption">
    <div className="modal-heading"><h2>Review caption</h2><button aria-label="Close caption editor" disabled={busy} onClick={() => draftRevision.current === undefined ? onClose() : run(async () => { await batchRequest('keepDraft', { itemId: initial.itemId, draftRevision: draftRevision.current, caption }); onClose(); })}><X size={20} /></button></div>
    {initial.stale && <p className="agent-error">This item changed after generation. Its draft is retained; saving will be blocked until the source conflict is resolved.</p>}
    <textarea aria-label="Caption draft" rows={8} value={caption} disabled={busy} onChange={e => setCaption(e.target.value)} />
    <label className="field-label" htmlFor="caption-revision-instructions">REVISION INSTRUCTIONS</label>
    <textarea id="caption-revision-instructions" rows={2} placeholder="e.g. Shorten it and include the dimensions" value={instructions} disabled={busy} onChange={e => setInstructions(e.target.value)} />
    <button className="button secondary" disabled={busy || !instructions.trim()} onClick={revise}>{busy ? <LoaderCircle size={15} className="spin" /> : null}Revise with agent</button>
    <div className="caption-history"><select aria-label="Caption history" disabled={busy} value={selected} onChange={e => setSelected(e.target.value)}><option value="">Earlier versions</option>{[...versions].reverse().map((v, i) => <option key={v.id} value={v.id}>{versions.length - i}. {v.label} — {v.caption.slice(0, 55) || '(empty)'}</option>)}</select><button className="button secondary" disabled={busy || !selected} onClick={restore}>Restore</button></div>
    <div className="agent-buttons"><button className="button primary" disabled={busy || !caption.trim() || initial.stale} onClick={() => run(async () => { const saved = await agentRequest<Item>('apply', { itemId: initial.itemId, expectedHash: initial.expectedHash, caption, replaceExisting: initial.replaceExisting ?? false, draftRevision: draftRevision.current ?? null }); await onSaved(saved); })}>Save caption</button>{draftRevision.current !== undefined && <button className="button secondary" disabled={busy || !caption.trim()} onClick={() => run(async () => { await batchRequest('keepDraft', { itemId: initial.itemId, draftRevision: draftRevision.current, caption }); onClose(); })}>Keep draft</button>}<button className="button secondary" disabled={busy} onClick={() => run(async () => { if (draftRevision.current !== undefined) await batchRequest('discardDraft', { itemId: initial.itemId, draftRevision: draftRevision.current }); onClose(); })}>Discard</button></div>
    {error && <p className="agent-error" role="alert">{error}</p>}
  </section></div>;
}
