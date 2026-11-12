import { useCallback, useEffect, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ArrowDownToLine, ArrowLeft, ArrowRight, Check, CheckCheck, ChevronDown, CircleHelp, Clipboard, Copy, FileImage, FolderOpen, ImagePlus, Layers3, Link2, LoaderCircle, Plus, RefreshCw, Search, Settings, Trash2, X } from 'lucide-react';
import { agentRequest, batchRequest, blobPixels, choose, clipboard, confirmation, confirmDiscardCaption, imageUrl, request } from './api';
import type { BatchStatus, CaptionDraft, Filter, Info, Item, Missing, Page, Report, Role, Storage, Summary } from './types';
import AgentSettings from './AgentSettings';
import CaptionEditor from './CaptionEditor';
import AskAgent from './AskAgent';
import { fileBase64, readImageDrop } from './drop';
import { CaptionDrafts, CollectionSettings, GenerateBatch, QueueProgress } from './CaptionBatch';

const FILTERS: [Filter, string][] = [['all', 'All items'], ['empty-caption', 'Without captions'], ['missing-original', 'Without originals'], ['missing-files', 'Missing files']];
const PAGE_SIZE = 100;

export default function App() {
  const [info, setInfo] = useState<Info | null>(null);
  const [filter, setFilter] = useState<Filter>('all');
  const [query, setQuery] = useState(''); const [search, setSearch] = useState('');
  const [page, setPage] = useState<Page>({ items: [], total: 0 }); const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [item, setItem] = useState<Item | null>(null); const draft = useRef<Item | null>(null);
  const [dirty, setDirty] = useState(false); const dirtyRef = useRef(false);
  const [saving, setSaving] = useState(false); const savePromise = useRef<Promise<void> | null>(null);
  const [busy, setBusy] = useState(false); const busyRef = useRef(false);
  const [error, setError] = useState(''); const [notice, setNotice] = useState('');
  const [warnings, setWarnings] = useState<string[]>([]); const [missing, setMissing] = useState<Missing[] | null>(null);
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const [storage, setStorage] = useState<Storage>('copy'); const [dragging, setDragging] = useState(false);
  const [moveAsset, setMoveAsset] = useState<string | null>(null); const [destQuery, setDestQuery] = useState(''); const [destinations, setDestinations] = useState<Summary[]>([]);
  const [fullImage, setFullImage] = useState<string | null>(null); const [help, setHelp] = useState(false);
  const [agentSettings, setAgentSettings] = useState(false); const [captionDraft, setCaptionDraft] = useState<CaptionDraft | null>(null);
  const [conversationItem, setConversationItem] = useState<string | null>(null);
  const captionDraftOpen = useRef(false); captionDraftOpen.current = captionDraft !== null;
  const [collectionSettings,setCollectionSettings]=useState(false);const [generationPreview,setGenerationPreview]=useState<{ids:string[]|null}|null>(null);
  const [draftsView,setDraftsView]=useState(false);const [draftsRevision,setDraftsRevision]=useState(0);
  const [batchStatus,setBatchStatus]=useState<BatchStatus>({queue:null,draftCount:0,active:false});const [batchTick,setBatchTick]=useState(0);
  const grid = useRef<HTMLDivElement>(null); const [columns, setColumns] = useState(3);
  const loadEpoch = useRef(0);

  useEffect(() => {
    if (!isTauri()) return;
    let active=true;
    agentRequest<{ openSettings: boolean }>('launchIntent').then(intent => { if (active && intent.openSettings) setAgentSettings(true); }).catch(() => {});
    request<Info>('info').then(current => { if (active) { setInfo(current); setStorage(current.settings.defaultStorage); } }).catch(() => { /* No collection is open on a fresh launch. */ });
    return () => { active=false; };
  }, []);
  useEffect(()=>{
    if(!info)return;let active=true;let pending=false;
    const update=async()=>{if(pending)return;pending=true;try{const next=await batchRequest<BatchStatus>('status');if(active)setBatchStatus(next);}catch(e){if(active)setError(String(e))}finally{pending=false}};
    update();const timer=setInterval(update,2000);return()=>{active=false;clearInterval(timer)};
  },[info?.root,batchTick]);
  useEffect(()=>{if(!isTauri())return;const stop=listen('caption-queue-updated',()=>{setBatchTick(v=>v+1);setDraftsRevision(v=>v+1)});return()=>{stop.then(fn=>fn()).catch(()=>{})}},[]);

  const adopt = (value: Item | null) => { draft.current = value; setItem(value); dirtyRef.current = false; setDirty(false); };
  const edit = (patch: Partial<Item>) => { if (!draft.current) return; const next = { ...draft.current, ...patch }; draft.current = next; setItem(next); dirtyRef.current = true; setDirty(true); setError(''); };
  const flush = useCallback(async () => {
    if (savePromise.current) await savePromise.current;
    if (!dirtyRef.current || !draft.current) return;
    const snapshot = draft.current; setSaving(true);
    const promise = (async () => {
      const saved = await request<Item>('save', { item: snapshot });
      if (draft.current?.id === snapshot.id) { draft.current = saved; setItem(saved); dirtyRef.current = false; setDirty(false); }
    })(); savePromise.current = promise;
    try { await promise; } finally { savePromise.current = null; setSaving(false); }
  }, []);

  const loadPage = useCallback(async () => {
    const epoch = ++loadEpoch.current;
    const result = await request<Page>('list', { query: search, filter, offset, limit: PAGE_SIZE });
    if (epoch !== loadEpoch.current) return;
    if (result.total && offset >= result.total) { setOffset(Math.floor((result.total - 1) / PAGE_SIZE) * PAGE_SIZE); return; }
    setPage(result);
    const cached = await request<Record<string, string>>('previews', { ids: result.items.map(i => i.id) });
    if (epoch === loadEpoch.current) setPreviews(p => ({ ...p, ...cached }));
  }, [search, filter, offset]);

  const run = useCallback(async (operation: () => Promise<void>, save = true) => {
    if (busyRef.current) return;
    busyRef.current = true; setBusy(true); setError(''); setNotice('');
    try { if (save) await flush(); await operation(); } catch (e) { setError(String(e)); }
    finally { busyRef.current = false; setBusy(false); }
  }, [flush]);
  useEffect(() => { const timer = setTimeout(() => { setSearch(query); setOffset(0); }, 200); return () => clearTimeout(timer); }, [query]);
  useEffect(() => { if (info) loadPage().catch(e => setError(String(e))); }, [info, loadPage]);
  useEffect(() => { if (!dirty) return; const timer = setTimeout(() => { flush().then(loadPage).catch(e => setError(String(e))); }, 700); return () => clearTimeout(timer); }, [dirty, item, flush, loadPage]);
  useEffect(() => {
    const observer = new ResizeObserver(entries => setColumns(Math.max(1, Math.floor(entries[0].contentRect.width / 240))));
    if (grid.current) observer.observe(grid.current); return () => observer.disconnect();
  }, [info, item]);
  useEffect(() => {
    if (!isTauri()) return;
    const stop = listen<{ id: string; path: string }>('thumbnail-ready', event => setPreviews(p => ({ ...p, [event.payload.id]: event.payload.path })));
    return () => { stop.then(fn => fn()).catch(() => {}); };
  }, []);
  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent) => { if (dirtyRef.current || captionDraftOpen.current) { event.preventDefault(); event.returnValue = ''; } };
    window.addEventListener('beforeunload', beforeUnload); return () => window.removeEventListener('beforeunload', beforeUnload);
  }, []);
  useEffect(() => {
    if (!isTauri()) return;
    let cleanup: (() => void) | undefined; let cancelled = false;
    getCurrentWindow().onCloseRequested(async event => {
      if (dirtyRef.current || busyRef.current || savePromise.current || captionDraftOpen.current) {
        event.preventDefault();
        if (busyRef.current) { setError('Wait for the current operation to finish before closing.'); return; }
        if (captionDraftOpen.current && !await confirmDiscardCaption()) return;
        try { await flush(); await getCurrentWindow().destroy(); } catch (e) { setError(String(e)); }
      }
    }).then(fn => { if (cancelled) fn(); else cleanup = fn; }).catch(e => setError(String(e)));
    return () => { cancelled = true; cleanup?.(); };
  }, [flush]);

  const openCollection = (create: boolean) => run(async () => {
    const path = await choose(true); if (typeof path !== 'string') return;
    const next = await request<Info>('open', { path, create }); setInfo(next); setStorage(next.settings.defaultStorage);
    setWarnings(next.report?.warnings ?? []); setSelected(new Set()); setOffset(0); setQuery(''); setFilter('all'); setPreviews({}); adopt(null);
    setBatchStatus({queue:null,draftCount:0,active:false});setBatchTick(v=>v+1);
  });
  const openItem = (id: string) => run(async () => {
    const value = await request<Item>('get', { id }); adopt(value);
    const cached = await request<Record<string, string>>('previews', { ids: [id] }); setPreviews(p => ({ ...p, ...cached }));
    const absent = await request<Missing[]>('check', { ids: [id] }); setMissing(absent.length ? absent : null);
  });
  const importPaths = (paths: string[], target: string | null) => run(async () => {
    const result = await request<Item[]>('import', { paths, itemId: target, storage });
    if (target && result[0]) adopt(result[0]);
    await loadPage(); if (target) setPreviews(p => ({ ...p })); setNotice(`${paths.length} image${paths.length === 1 ? '' : 's'} ${target ? 'attached to this item' : 'added to your inbox'}.`);
    if (target) { const cached = await request<Record<string, string>>('previews', { ids: [target] }); setPreviews(p => ({ ...p, ...cached })); }
  });
  const addFiles = async (target: string | null) => {
    try { const paths = await choose(); if (paths) await importPaths(Array.isArray(paths) ? paths : [paths], target); } catch (e) { setError(String(e)); }
  };
  const infoRef = useRef(info); infoRef.current = info;
  const dropImages = (event: React.DragEvent<HTMLDivElement>) => {
    const dropTarget=event.target as HTMLElement;
    if (dropTarget.closest('input,textarea,[contenteditable]') && !event.dataTransfer.files.length && !event.dataTransfer.getData('text/uri-list') && !/<img\b/i.test(event.dataTransfer.getData('text/html'))) return;
    event.preventDefault(); setDragging(false);
    if (!infoRef.current) { setError('Open or create a collection before adding files.'); return; }
    if (busyRef.current) { setError('Wait for the current operation before dropping images.'); return; }
    try {
      const data = readImageDrop(event.dataTransfer);
      const target = (event.target as HTMLElement).closest<HTMLElement>('[data-drop-item]')?.dataset.dropItem ?? null;
      if (data.localPaths.length) { importPaths(data.localPaths, target); return; }
      run(async () => {
        try {
          const imported: Item[] = [];
          if (data.files.length) {
            for (const [index,file] of data.files.entries()) imported.push(await request<Item>('importImage', { itemId: target, filename: file.name, bytesBase64: await fileBase64(file), sourceUrl: data.urls.length === data.files.length ? data.urls[index] : data.files.length === 1 ? data.sourceUrl : null }));
          } else {
            for (const url of data.urls) imported.push(await request<Item>('importWebImage', { itemId: target, url }));
          }
          
          setNotice(data.files.length && storage === 'link' ? 'Dropped image bytes were copied. Use the file picker to link local files.' : `${imported.length} image${imported.length === 1 ? '' : 's'} imported.`);
          const ids = imported.map(i => i.id); if (ids.length) { const cached = await request<Record<string, string>>('previews', { ids }); setPreviews(p => ({ ...p, ...cached })); }
        } finally {
          if (target && target === draft.current?.id) {
            adopt(await request<Item>('get', { id: target }));
            const cached=await request<Record<string,string>>('previews',{ids:[target]});setPreviews(p=>({...p,...cached}));
          }
          await loadPage();
        }
      });
    } catch (e) { setError(String(e)); }
  };

  const pasteData = (data: Awaited<ReturnType<typeof clipboard>>) => run(async () => {
    if (typeof data === 'string') {
      let current = draft.current;
      if (!current) current = await request<Item>('create');
      current = { ...current, caption: current.caption ? `${current.caption}\n${data}` : data };
      const saved = await request<Item>('save', { item: current }); adopt(saved);
    } else { adopt(await request<Item>('clipboard', { ...data, itemId: draft.current?.id ?? null })); }
    await loadPage();
    if (draft.current) { const cached = await request<Record<string, string>>('previews', { ids: [draft.current.id] }); setPreviews(p => ({ ...p, ...cached })); }
  });
  const pasteRef = useRef(pasteData); pasteRef.current = pasteData;
  useEffect(() => {
    const handler = (event: ClipboardEvent) => {
      if (!infoRef.current || (event.target as HTMLElement).closest('input,textarea,[contenteditable]')) return;
      const file = Array.from(event.clipboardData?.files ?? []).find(f => f.type.startsWith('image/'));
      const text = event.clipboardData?.getData('text/plain');
      if (file) { event.preventDefault(); blobPixels(file).then(p => pasteRef.current(p)).catch(e => setError(String(e))); }
      else if (text) { event.preventDefault(); pasteRef.current(text); }
    }; window.addEventListener('paste', handler); return () => window.removeEventListener('paste', handler);
  }, []);

  useEffect(() => {
    if (!moveAsset) return;
    let active = true; request<Page>('list', { query: destQuery, filter: 'all', offset: 0, limit: 30 }).then(result => { if (active) setDestinations(result.items.filter(i => i.id !== draft.current?.id)); }).catch(e => setError(String(e)));
    return () => { active = false; };
  }, [moveAsset, destQuery]);

  const refresh = () => run(async () => { const report = await request<Report>('refresh'); setWarnings(report.warnings); await loadPage(); setNotice(`Checked ${report.indexed.toLocaleString()} items. External changes reconciled.`); if (draft.current) adopt(await request<Item>('get', { id: draft.current.id })); });
  const exportItems = (ids: string[]) => run(async () => {
    const absent = await request<Missing[]>('check', { ids }); if (absent.length) { setMissing(absent); setError('Export paused. Locate the missing files, or deselect their items and export again.'); await loadPage(); return; }
    const parent = await choose(true); if (typeof parent !== 'string') return;
    const result = await request<string>('export', { ids, parent }); setNotice(`Package ready: ${result}`);
  });
  const removeItems = (ids: string[]) => run(async () => {
    if (!await confirmation(`Remove ${ids.length} item(s)? Their manifests and managed images will move to .nabor/trash inside this collection. Linked source files stay in their original locations. You can recover item folders manually from trash.`)) return;
    await request('remove', { ids }); if (draft.current && ids.includes(draft.current.id)) adopt(null); setSelected(new Set()); await loadPage();
  });
  const changeRole = (assetId: string, role: Role) => run(async () => { adopt(await request<Item>('role', { itemId: draft.current!.id, assetId, role })); await loadPage(); });
  const virtual = useVirtualizer({ count: Math.ceil(page.items.length / columns), getScrollElement: () => grid.current, estimateSize: () => 272, overscan: 3 });
  const toggle = (id: string) => setSelected(old => { const next = new Set(old); if (next.has(id)) next.delete(id); else next.add(id); return next; });
  const locked = busy || saving;
  const collectionName = info?.root.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? 'Your collection';

  return <div className="app-shell" onDragOver={event => { event.preventDefault(); event.dataTransfer.dropEffect = 'copy'; setDragging(true); }} onDragLeave={event => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setDragging(false); }} onDrop={dropImages}>
    <aside className="sidebar">
      <a className="brand" href="#" onClick={e => e.preventDefault()} aria-label="Nabor home"><span className="brand-icon"><Layers3 size={23} /></span>nabor<span className="brand-dot">.</span></a>
      <button className="collection-switch" title={info?.root} onClick={() => openCollection(false)} disabled={locked}><span className="collection-avatar"><FolderOpen size={18} /></span><span><strong>{info ? collectionName : 'Open a collection'}</strong></span><ChevronDown size={15} /></button>
      <nav aria-label="Collection filters">{FILTERS.map(([value, label], index) => <button key={value} className={`nav-item ${filter === value ? 'active' : ''}`} disabled={!info || locked} onClick={() => { setFilter(value); setOffset(0); }}>{index === 0 ? <Layers3 size={18} /> : index === 1 ? <Clipboard size={18} /> : index === 2 ? <FileImage size={18} /> : <Link2 size={18} />}<span>{label}</span>{value === 'all' && filter === 'all' && info && <small>{page.total.toLocaleString()}</small>}</button>)}</nav>
      <div className="sidebar-divider" />
      <button className="nav-item" disabled={!info || locked} onClick={refresh}><RefreshCw size={18} /><span>Refresh & check files</span></button>
      <button className="nav-item" disabled={locked} onClick={() => openCollection(true)}><Plus size={18} /><span>New collection</span></button>
      <button className="nav-item" disabled={!info||locked} onClick={()=>run(async()=>{setCollectionSettings(true)})}><Settings size={18}/><span>Collection settings</span></button>
      <button className="nav-item" disabled={!info||locked} onClick={()=>run(async()=>{setDraftsView(true)})}><Clipboard size={18}/><span>Caption drafts</span>{batchStatus.draftCount>0&&<small>{batchStatus.draftCount}</small>}</button>
      <div className="sidebar-bottom"><button className="nav-item" disabled={locked} onClick={() => setAgentSettings(true)}><Settings size={18} /><span>Agent settings</span></button><button className="nav-item" onClick={() => setHelp(true)}><CircleHelp size={18} /><span>Help</span></button></div>
    </aside>
    <main className="workspace">
      {info && <div className="workspace-heading"><h1>{FILTERS.find(f => f[0] === filter)?.[1]}</h1><div className="heading-actions"><button className="button secondary" disabled={locked||batchStatus.active} onClick={()=>run(async()=>{setGenerationPreview({ids:null})})}>Generate entire collection</button><button className="button secondary" disabled={locked} onClick={() => run(async () => { adopt(await request<Item>('create')); await loadPage(); })}><Plus size={17} />Blank item</button><button className="button primary" disabled={locked} onClick={() => addFiles(null)}><ImagePlus size={18} />Add images</button></div></div>}
      {!info ? <div className="welcome">
        <h1>Open or create a collection</h1>
        <div className="welcome-actions"><button className="button primary" disabled={locked} onClick={() => openCollection(true)}><Plus size={18} />Create a collection</button><button className="button secondary" disabled={locked} onClick={() => openCollection(false)}><FolderOpen size={18} />Open existing</button></div>
      </div> : <>
        <div className="toolbar"><div className="search"><Search size={18} /><input aria-label="Search collection" placeholder="Search titles, captions, notes, files…" value={query} onChange={e => setQuery(e.target.value)} />{query && <button aria-label="Clear search" onClick={() => setQuery('')}><X size={14} /></button>}</div><div className="import-mode"><span>Import as</span><select aria-label="Import storage" value={storage} disabled={locked} onChange={e => setStorage(e.target.value as Storage)}><option value="copy">Copy files</option><option value="link">Link to files</option></select><button title="Save this import mode as the collection default" aria-label="Save import default" disabled={locked} onClick={() => run(async () => { const settings = await request<Info['settings']>('settings', { storage }); setInfo(old => old ? { ...old, settings } : old); setNotice(`Default import mode saved: ${storage === 'copy' ? 'copy files' : 'link to files'}.`); })}><Check size={16} /></button></div></div>
        <div className="list-meta"><label><input type="checkbox" aria-label="Select current page" checked={page.items.length > 0 && page.items.every(i => selected.has(i.id))} onChange={() => setSelected(old => { const next = new Set(old); const all = page.items.every(i => next.has(i.id)); page.items.forEach(i => all ? next.delete(i.id) : next.add(i.id)); return next; })} />{selected.size ? `${selected.size} selected` : `${page.total.toLocaleString()} items`}</label>{selected.size > 0 && <div><button disabled={locked||batchStatus.active} onClick={()=>run(async()=>{setGenerationPreview({ids:[...selected]})})}>Generate selected</button><button disabled={locked} onClick={() => exportItems([...selected])}><ArrowDownToLine size={15} />Export selected</button><button disabled={locked} onClick={() => removeItems([...selected])}><Trash2 size={15} />Remove</button><button onClick={() => setSelected(new Set())}>Clear</button></div>}</div>
        <div className={`content-area ${item ? 'with-detail' : ''}`}>
          <section className={`inbox ${dragging ? 'dragging' : ''}`} aria-label="Image inbox"><div className="grid-scroll" ref={grid}>
            {page.items.length ? <div className="virtual-grid" style={{ height: virtual.getTotalSize() }}>{virtual.getVirtualItems().map(row => <div className="grid-row" key={row.key} style={{ transform: `translateY(${row.start}px)`, gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}>{page.items.slice(row.index * columns, (row.index + 1) * columns).map(card => <article className={`item-card ${item?.id === card.id ? 'current' : ''} ${selected.has(card.id) ? 'selected' : ''}`} key={card.id} data-drop-item={card.id}>
              <div className="card-image" role="button" tabIndex={0} aria-label={`Open ${card.title}`} onClick={() => openItem(card.id)} onKeyDown={e => { if (e.key === 'Enter') openItem(card.id); }}>{card.thumbnailAsset && previews[card.thumbnailAsset] ? <img src={imageUrl(previews[card.thumbnailAsset])} alt={card.title} loading="lazy" /> : <div className="image-placeholder"><FileImage size={34} strokeWidth={1.1} /><span>{card.missingOriginal ? 'Add an original' : 'Preparing preview'}</span></div>}{card.missingFiles && <span className="missing-badge">Missing file</span>}</div>
              <input className="card-checkbox" type="checkbox" aria-label={`Select ${card.title}`} checked={selected.has(card.id)} onChange={() => toggle(card.id)} />
              <button className="card-description" onClick={() => openItem(card.id)}><strong>{card.title || 'Untitled item'}</strong><span>{card.caption || 'No caption yet'}</span><div className="card-footer"><span><Layers3 size={12} />{card.assetCount} {card.assetCount === 1 ? 'file' : 'files'}</span>{!card.emptyCaption && <span className="caption-check"><Check size={12} />Caption</span>}</div></button>
            </article>)}</div>)}</div> : <div className="empty-inbox"><h2>{search || filter !== 'all' ? 'No matching items' : 'No items'}</h2>{!search && filter === 'all' && <p>Drop images here or use Add images.</p>}</div>}
          </div><footer className="inbox-footer"><div><button aria-label="Previous page" disabled={offset === 0} onClick={() => setOffset(o => Math.max(0, o - PAGE_SIZE))}><ArrowLeft size={15} /></button><span>{page.total ? `${offset + 1}–${Math.min(offset + PAGE_SIZE, page.total)} of ${page.total.toLocaleString()}` : '0 items'}</span><button aria-label="Next page" disabled={offset + PAGE_SIZE >= page.total} onClick={() => setOffset(o => o + PAGE_SIZE)}><ArrowRight size={15} /></button></div></footer></section>
          {item && <aside className={`detail ${dragging ? 'dragging' : ''}`} data-item-tray data-drop-item={item.id} aria-label="Active collection tray">
            <div className="detail-header"><div><span className="tray-dot" />ACTIVE ITEM</div><button aria-label="Close item" disabled={locked} onClick={() => run(async () => { adopt(null); setMissing(null); await loadPage(); })}><X size={18} /></button></div>
            <div className="detail-scroll"><div className="detail-title"><input aria-label="Item title" value={item.title} disabled={locked} onChange={e => edit({ title: e.target.value })} /><span className={`save-status ${dirty ? 'unsaved' : ''}`}>{saving ? 'Saving…' : dirty ? 'Unsaved changes' : <><CheckCheck size={13} />Saved locally</>}</span></div>
            <button className="button secondary ask-item-action" disabled={locked || batchStatus.active} onClick={() => run(async () => { setConversationItem(draft.current!.id); })}>Ask agent</button>
            <label className="field-label" htmlFor="caption">CAPTION</label><textarea id="caption" rows={3} value={item.caption} disabled={locked} onChange={e => edit({ caption: e.target.value })} />
            <div className="caption-actions"><button className="button secondary" title={item.caption.trim() ? 'Open the saved caption for manual edits or agent revisions' : 'Send this item’s images and text to ChatGPT to draft a caption'} disabled={locked} onClick={() => run(async () => { setCaptionDraft(await agentRequest<CaptionDraft>(draft.current!.caption.trim() ? 'draft' : 'generate', { itemId: draft.current!.id })); })}>{item.caption.trim() ? 'Revise with agent' : 'Generate caption'}</button>{!!item.captionHistory?.length && <button className="button secondary" disabled={locked} onClick={() => run(async () => { setCaptionDraft(await agentRequest<CaptionDraft>('draft', { itemId: draft.current!.id })); })}>History</button>}</div>
            <label className="field-label" htmlFor="notes">INSTRUCTIONS</label><textarea id="notes" rows={2} value={item.notes} disabled={locked} onChange={e => edit({ notes: e.target.value })} />
            <div className="attachments-heading"><span className="field-label">FILES <span>{item.assets.length}</span></span><button disabled={locked} onClick={() => addFiles(item.id)}><Plus size={14} />Attach</button></div>
            {!item.assets.some(a => a.role === 'original') && <p className="inline-warning">Choose an attachment as the original image.</p>}
            {item.assets.map(asset => <div className="attachment" key={asset.id}><button className="attachment-preview" title="View full resolution" disabled={locked} onClick={() => run(async () => { const path = await request<string>('original', { itemId: item.id, assetId: asset.id }); setFullImage(path); })}>{previews[asset.id] ? <img src={imageUrl(previews[asset.id])} alt={asset.filename} /> : <FileImage size={23} />}</button><div className="attachment-body"><strong title={asset.filename}>{asset.filename}</strong><span className="storage-label">{asset.storage === 'link' ? <Link2 size={11} /> : <Copy size={11} />}{asset.storage === 'link' ? 'Linked file' : 'In collection'}</span><select aria-label={`Role for ${asset.filename}`} value={asset.role} disabled={locked} onChange={e => changeRole(asset.id, e.target.value as Role)}><option value="original">Original image</option><option value="caption-source">Caption source</option><option value="reference">Reference</option></select><div className="attachment-actions">{asset.sourceUrl && <button title={asset.sourceUrl} disabled={locked} onClick={() => run(async () => { await request('openSource', { itemId: item.id, assetId: asset.id }); })}>Source</button>}<button disabled={locked} onClick={() => setMoveAsset(asset.id)}>Move to item</button>{asset.storage === 'link' && <button disabled={locked} onClick={() => run(async () => { const paths = await choose(); const path = Array.isArray(paths) ? paths[0] : paths; if (path) { adopt(await request<Item>('reconnect', { itemId: item.id, assetId: asset.id, path })); setMissing(null); await loadPage(); } })}>Locate file</button>}<button disabled={locked} onClick={() => run(async () => { adopt(await request<Item>('detach', { itemId: item.id, assetId: asset.id })); await loadPage(); })}>Detach</button></div></div></div>)}
            <button className="paste-zone" title="Paste an image as a reference or text as a caption" disabled={locked} onClick={() => { clipboard().then(pasteData).catch(e => setError(String(e))); }}><Clipboard size={16} /><strong>Paste</strong></button>
            </div><div className="detail-footer"><button className="icon-button danger" aria-label="Remove this item" disabled={locked} onClick={() => removeItems([item.id])}><Trash2 size={17} /></button><button className="button primary" disabled={locked} onClick={() => exportItems([item.id])}><ArrowDownToLine size={17} />Export item</button></div>
          </aside>}
        </div>
      </>}
      {info&&<QueueProgress status={batchStatus} onChanged={()=>setBatchTick(v=>v+1)}/>}
      {(busy || saving) && <div className="working"><LoaderCircle size={14} className="spin" />{saving ? 'Saving item…' : 'Working…'}</div>}
      {error && <div className="notification error" role="alert"><span>{error}{dirty && ' Your edits remain here. Retry saving or discard them to refresh.'}</span><div>{dirty && <><button disabled={locked} onClick={() => run(async () => { await flush(); await loadPage(); }, false)}>Retry save</button><button disabled={locked} onClick={() => run(async () => { if (draft.current) adopt(await request<Item>('get', { id: draft.current.id })); }, false)}>Discard edits</button></>}<button aria-label="Dismiss error" onClick={() => setError('')}><X size={16} /></button></div></div>}
      {notice && <div className="notification success" role="status"><Check size={17} /><span>{notice}</span><button aria-label="Dismiss notice" onClick={() => setNotice('')}><X size={16} /></button></div>}
      {warnings.length > 0 && <div className="warnings"><strong>{warnings.length} manifest(s) could not be indexed. Files were left untouched.</strong><details><summary>Show details</summary>{warnings.map((w, i) => <p key={i}>{w}</p>)}</details><button onClick={() => setWarnings([])}>Dismiss</button></div>}
    </main>
    {agentSettings && <AgentSettings onClose={() => setAgentSettings(false)} />}
    {collectionSettings&&info&&<CollectionSettings info={info} onSave={settings=>setInfo(old=>old?{...old,settings}:old)} onClose={()=>setCollectionSettings(false)}/>}
    {conversationItem && <AskAgent itemId={conversationItem} onClose={() => setConversationItem(null)} onBusy={value => { busyRef.current = value; setBusy(value); }} onChanged={async saved => { if (draft.current?.id === saved.id) adopt(saved); await loadPage(); }} onCaption={value => { setConversationItem(null); setCaptionDraft(value); }} />}
    {generationPreview&&<GenerateBatch ids={generationPreview.ids} onStarted={()=>setBatchTick(v=>v+1)} onClose={()=>setGenerationPreview(null)}/>}
    {draftsView&&<CaptionDrafts revision={draftsRevision+batchStatus.draftCount} onReview={value=>setCaptionDraft(value)} onChanged={()=>{setBatchTick(v=>v+1);loadPage().catch(e=>setError(String(e)));if(draft.current&&!dirtyRef.current)request<Item>('get',{id:draft.current.id}).then(adopt).catch(()=>{})}} onClose={()=>setDraftsView(false)}/>}
    {captionDraft && <CaptionEditor initial={captionDraft} onBusy={value => { busyRef.current = value; setBusy(value); }} onClose={() => {setCaptionDraft(null);setDraftsRevision(v=>v+1);setBatchTick(v=>v+1)}} onSaved={async saved => { if(draft.current?.id===saved.id)adopt(saved);setCaptionDraft(null);setDraftsRevision(v=>v+1);setBatchTick(v=>v+1);await loadPage().catch(e => setError(String(e))); }} />}
    {moveAsset && <div className="modal-backdrop"><section className="modal" role="dialog" aria-modal="true" aria-label="Move attachment"><div className="modal-heading"><h2>Move to another item</h2><button aria-label="Close move dialog" onClick={() => setMoveAsset(null)}><X size={20} /></button></div><p>The attachment becomes a reference in the destination.</p><input aria-label="Find destination item" autoFocus placeholder="Search destination items…" value={destQuery} onChange={e => setDestQuery(e.target.value)} /><div className="destination-list">{destinations.map(d => <button key={d.id} disabled={locked} onClick={() => run(async () => { const source = draft.current!.id; await request('move', { from: source, to: d.id, assetId: moveAsset }); adopt(await request<Item>('get', { id: source })); setMoveAsset(null); await loadPage(); })}><FileImage size={18} /><span>{d.title}</span><ArrowRight size={16} /></button>)}{!destinations.length && <p>No other matching items. Create an item first.</p>}</div></section></div>}
    {missing && <div className="missing-panel" role="status"><div><strong>{missing.length} missing file(s)</strong><button aria-label="Dismiss missing files" onClick={() => setMissing(null)}><X size={16} /></button></div>{missing.map(m => <button key={m.assetId} disabled={locked} onClick={() => { setMissing(null); openItem(m.itemId); }}><Link2 size={14} /><span>{m.title} / {m.filename}</span><ArrowRight size={14} /></button>)}<p>Open an item and use Locate file for linked assets. To exclude an item from export, deselect its card.</p></div>}
    {fullImage && <div className="lightbox" role="dialog" aria-modal="true" aria-label="Full resolution image"><button aria-label="Close image" onClick={() => setFullImage(null)}><X size={24} /></button><img src={imageUrl(fullImage)} alt="Full resolution attachment" /></div>}
    {help && <div className="modal-backdrop"><section className="modal" role="dialog" aria-modal="true" aria-label="Help"><div className="modal-heading"><h2>Help</h2><button aria-label="Close help" onClick={() => setHelp(false)}><X size={20} /></button></div><p>Drop images in the inbox to create items. Drop on an open item to attach references. Set attachment roles using their menus.</p><p>Paste images as references and text as captions. Text fields use normal paste.</p><p>Copy files into the collection or link to their existing locations. Use Locate file to repair broken links.</p><p>Select items to export their files, captions, and instructions together.</p><p>Detached files remain in their item folder. Removed items move to <code>.nabor/trash</code>. Linked source files are never deleted.</p><p>Supported formats: PNG, JPEG, WebP, GIF, BMP, TIFF. Search covers typed text and filenames; OCR is not included.</p></section></div>}
  </div>;
}
