import { useState } from 'react';
import { X } from 'lucide-react';
import { request } from './api';
import type { Item, WebsiteLink } from './types';

export default function LinkEditor({ item, link, onSaved, onClose, onBusy }: {
  item: Item; link?: WebsiteLink; onSaved: (item: Item) => Promise<void>; onClose: () => void; onBusy: (busy: boolean) => void;
}) {
  const [url, setUrl] = useState(link?.url ?? '');
  const [title, setTitle] = useState(link?.title ?? '');
  const [notes, setNotes] = useState(link?.notes ?? '');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  return <div className="modal-backdrop"><section className="modal link-editor" role="dialog" aria-modal="true" aria-label={link ? 'Edit link' : 'Add link'}>
    <div className="modal-heading"><h2>{link ? 'Edit link' : 'Add link'}</h2><button aria-label="Close link editor" disabled={busy} onClick={onClose}><X size={20} /></button></div>
    <form onSubmit={async e => {
      e.preventDefault(); setBusy(true); onBusy(true); setError('');
      try { const saved = await request<Item>('saveLink', { itemId: item.id, expectedHash: item.sourceHash, linkId: link?.id ?? null, url, title, notes }); await onSaved(saved); }
      catch (e) { setError(String(e)); }
      finally { setBusy(false); onBusy(false); }
    }}>
      <label className="field-label" htmlFor="website-url">URL</label><input id="website-url" type="url" required autoFocus placeholder="https://" value={url} disabled={busy} onChange={e => setUrl(e.target.value)} />
      <label className="field-label" htmlFor="website-title">Title (optional)</label><input id="website-title" value={title} disabled={busy} onChange={e => setTitle(e.target.value)} />
      <label className="field-label" htmlFor="website-notes">Notes (optional)</label><textarea id="website-notes" rows={3} value={notes} disabled={busy} onChange={e => setNotes(e.target.value)} />
      <button className="button primary" type="submit" disabled={busy || !url.trim()}>Save link</button>
    </form>
    {error && <p className="agent-error" role="alert">{error}</p>}
  </section></div>;
}
