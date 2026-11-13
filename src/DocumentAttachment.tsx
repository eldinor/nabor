import { useEffect, useState } from 'react';
import { FileText, LoaderCircle, X } from 'lucide-react';
import { choose, request } from './api';
import ActionMenu from './ActionMenu';
import type { DocumentAttachment, DocumentText, Item } from './types';

export default function DocumentRow({ item, document: doc, locked, onChanged, onMove, onBusy, beforeAction }: {
  item: Item; document: DocumentAttachment; locked: boolean; onChanged: (item: Item) => Promise<void>; onMove: () => void; onBusy: (busy: boolean) => void; beforeAction: () => Promise<Item>;
}) {
  const [text, setText] = useState<DocumentText | null>(null);
  const [error, setError] = useState(''); const [working, setWorking] = useState(false);
  const [preview, setPreview] = useState(false); const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(doc.title); const [notes, setNotes] = useState(doc.notes); const [role, setRole] = useState(doc.role);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let active = true; setText(null); setError('');
    request<DocumentText>('documentText', { itemId: item.id, documentId: doc.id }).then(value => { if (active) setText(value); }).catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [item.id, doc.id, doc.path, doc.contentHash, tick]);
  const run = async (op: (latest: Item) => Promise<void>) => {
    setWorking(true); onBusy(true); setError('');
    try { await op(await beforeAction()); } catch (e) { setError(String(e)); } finally { setWorking(false); onBusy(false); }
  };
  const refresh = (locate = false) => run(async latest => {
    let path: string | null = null;
    if (locate) { const selected = await choose(false, 'documents'); path = Array.isArray(selected) ? selected[0] : selected; if (!path) return; }
    await onChanged(await request<Item>('refreshDocument', { itemId: item.id, documentId: doc.id, expectedHash: latest.sourceHash, path }));
    setTick(v => v + 1);
  });
  return <div className="attachment document-attachment"><div className="document-icon"><FileText size={23} /><span>{doc.format.toUpperCase()}</span></div><div className="attachment-body">
    <strong title={doc.filename}>{doc.title || doc.filename}</strong><span className="storage-label">{doc.storage === 'link' ? 'Linked file' : 'In collection'} · {doc.role === 'caption-source' ? 'Caption source' : 'Reference'}</span>
    {!text && !error && <span className="document-status"><LoaderCircle className="spin" size={11} />Extracting text…</span>}
    {text && <span className={`document-status ${text.status !== 'ready' ? 'document-warning' : ''}`}>{text.warning || `${text.text.length.toLocaleString()} characters extracted`}</span>}
    {doc.notes && <p className="website-notes" title={doc.notes}>{doc.notes}</p>}
    <div className="attachment-actions"><button className="attachment-open" aria-label="Open in default app" title="Open in default app" disabled={locked || working} onClick={() => run(async () => { await request('openDocument', { itemId: item.id, documentId: doc.id }); })}>Open</button><button className="attachment-open" disabled={working || !text?.text} onClick={() => setPreview(true)}>View text</button><ActionMenu label={`Actions for ${doc.title || doc.filename}`} disabled={locked || working} actions={[
      {label:'Edit details',onClick:()=>{run(async()=>{setTitle(doc.title);setNotes(doc.notes);setRole(doc.role);setEditing(true);});}},
      {label:'Re-extract text',onClick:()=>{refresh();}},
      {label:'Move to item',onClick:onMove},
      ...(doc.storage === 'link' ? [{label:'Locate file',onClick:()=>{refresh(true);}}] : []),
      {label:'Remove document',danger:true,onClick:()=>{run(async latest=>{await onChanged(await request<Item>('removeDocument',{itemId:item.id,documentId:doc.id,expectedHash:latest.sourceHash}));});}}
    ]}/></div>
    {error && <p className="document-warning" role="alert">{error}</p>}
    {preview && <div className="modal-backdrop"><section className="modal document-preview" role="dialog" aria-modal="true" aria-label="Extracted document text"><div className="modal-heading"><h2>{doc.title || doc.filename}</h2><button aria-label="Close document text" onClick={() => setPreview(false)}><X size={20} /></button></div><div className="document-preview-body" tabIndex={0} aria-label="Document text">{text?.warning && <p>{text.warning}</p>}<pre>{text?.text}</pre></div></section></div>}
    {editing && <div className="modal-backdrop"><section className="modal link-editor" role="dialog" aria-modal="true" aria-label="Document details"><div className="modal-heading"><h2>Document details</h2><button aria-label="Close document details" disabled={working} onClick={() => setEditing(false)}><X size={20} /></button></div><form onSubmit={e => { e.preventDefault(); run(async latest => { await onChanged(await request<Item>('updateDocument', { itemId: item.id, documentId: doc.id, expectedHash: latest.sourceHash, title, notes, role })); setEditing(false); }); }}><label className="field-label" htmlFor={`document-title-${doc.id}`}>Title</label><input id={`document-title-${doc.id}`} disabled={working} value={title} onChange={e => setTitle(e.target.value)} /><label className="field-label" htmlFor={`document-notes-${doc.id}`}>Notes</label><textarea id={`document-notes-${doc.id}`} disabled={working} rows={3} value={notes} onChange={e => setNotes(e.target.value)} /><label className="field-label" htmlFor={`document-role-${doc.id}`}>Role</label><select id={`document-role-${doc.id}`} value={role} disabled={working} onChange={e => setRole(e.target.value as DocumentAttachment['role'])}><option value="reference">Reference</option><option value="caption-source">Caption source</option></select><div className="agent-buttons"><button type="submit" className="button primary" disabled={working}>Save details</button></div></form>{error && <p role="alert">{error}</p>}</section></div>}
  </div></div>;
}
