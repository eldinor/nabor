import { test, expect, type Page } from '@playwright/test';

async function fixture(page: Page, count = 3) {
  await page.addInitScript(({ count }) => {
    const w = window as any;
    const persisted = localStorage.getItem('fixture-items');
    let items: any[] = persisted ? JSON.parse(persisted) : Array.from({ length: count }, (_, i) => ({ schemaVersion: 1, id: `item-${i}`, title: `Photo ${String(i).padStart(5, '0')}`, caption: '', notes: '', revision: 1, createdAt: 0, updatedAt: 0, assets: [{ id: `asset-${i}`, filename: `Photo ${i}.png`, path: `assets/${i}.png`, storage: 'copy', role: 'original' }] }));
    w.calls = []; w.failSave = false; w.missing = [];
    w.agentStatus = { accounts: [], active: null, model: null, connecting: false, lastError: null }; w.agentError = '';
    w.conversations = JSON.parse(localStorage.getItem('fixture-conversations') || '{}');
    w.captionDrafts=JSON.parse(localStorage.getItem('fixture-drafts')||'{}');w.batchQueue=JSON.parse(localStorage.getItem('fixture-queue')||'null');w.commonPrompt=localStorage.getItem('fixture-prompt')||'';w.batchHold=false;
    if(w.batchQueue&&['running','pausing','cancelling'].includes(w.batchQueue.state)){w.batchQueue.state='paused';w.batchQueue.lastError='Previous run was interrupted. Resume explicitly.';}
    const persistBatch=()=>{localStorage.setItem('fixture-drafts',JSON.stringify(w.captionDrafts));localStorage.setItem('fixture-queue',JSON.stringify(w.batchQueue));};
    const entries=(ids:any)=>items.filter(i=>ids===null||ids.includes(i.id)).map(i=>({itemId:i.id,title:i.title,state:i.caption?'skipped-caption':w.captionDrafts[i.id]?'skipped-draft':'pending',error:null}));
    const counts=(rows:any[])=>({total:rows.length,pending:rows.filter(e=>e.state==='pending').length,processing:rows.filter(e=>e.state==='processing').length,done:rows.filter(e=>e.state==='done').length,skippedCaption:rows.filter(e=>e.state==='skipped-caption').length,skippedDraft:rows.filter(e=>e.state==='skipped-draft').length,failed:rows.filter(e=>e.state==='failed').length,cancelled:rows.filter(e=>e.state==='cancelled').length});
    const persist = () => localStorage.setItem('fixture-items', JSON.stringify(items));
    const summary = (i: any) => ({ id: i.id, title: i.title, caption: i.caption, assetCount: i.assets.length, missingOriginal: !i.assets.some((a: any) => a.role === 'original'), missingFiles: w.missing.some((m: any) => m.itemId === i.id), emptyCaption: !i.caption, thumbnailAsset: i.assets[0]?.id });
    const thumbnail = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aY1sAAAAASUVORK5CYII=';
    w.__NABOR_TEST__ = {
      talk: async (p: any) => {
        w.calls.push({ ...p, talk: true });
        const item = items.find(i => i.id === p.itemId);
        const c = w.conversations[p.itemId] ??= { schemaVersion: 1, itemId: p.itemId, revision: 0, turns: [] };
        const save = () => localStorage.setItem('fixture-conversations', JSON.stringify(w.conversations));
        const view = () => structuredClone({ conversation: c, itemId: item.id, title: item.title, sourceHash: 'fixture-hash', assetCount: item.assets.length });
        switch (p.action) {
          case 'get': return view();
          case 'ask': {
            const turn = p.retry ? c.turns.find((t: any) => t.id === p.retry) : { id: `turn-${c.turns.length}`, question: p.question, answer: null, error: null, model: 'test-model', sourceHash: 'fixture-hash', createdAt: 1 };
            if (!p.retry) c.turns.push(turn);
            turn.answer = w.talkError ? null : `Reply to: ${p.question}`; turn.error = w.talkError || null; c.revision++; save(); return view();
          }
          case 'clear': c.turns = []; c.revision++; save(); return view();
          case 'useCaption': return { itemId: item.id, expectedHash: 'fixture-hash', caption: c.turns.find((t: any) => t.id === p.turnId).answer, replaceExisting: !!item.caption, model: 'test-model', history: item.captionHistory ?? [] };
          case 'addNotes': item.notes += (item.notes ? '\n\n' : '') + c.turns.find((t: any) => t.id === p.turnId).answer; persist(); return structuredClone(item);
          default: throw new Error(`Unimplemented talk command ${p.action}`);
        }
      },
      batch:async(p:any)=>{
        w.calls.push({...p,batch:true});
        const queueView=()=>w.batchQueue?{...w.batchQueue,counts:counts(w.batchQueue.entries)}:null;
        const draftView=(d:any)=>({...d,replaceExisting:false,stale:!!items.find(i=>i.id===d.itemId)?.caption||!!d.stale,history:[]});
        switch(p.action){
          case 'status':{const q=w.batchQueue;if(q?.state==='running'&&!w.batchHold){const e=q.entries.find((e:any)=>e.state==='pending');if(e){if(w.batchFail){q.state='paused';q.lastError='subscription_sharing_usage_limit_exceeded';}else{e.state='done';w.calls.push({action:'mockGenerate',itemId:e.itemId});w.captionDrafts[e.itemId]={itemId:e.itemId,title:e.title,expectedHash:'fixture-hash',caption:`Draft for ${e.title}`,model:q.model,draftRevision:1,createdAt:1};}}if(!q.entries.some((e:any)=>e.state==='pending'))q.state='completed';persistBatch();}return{queue:queueView(),draftCount:Object.keys(w.captionDrafts).length,active:q?.state==='running'};}
          case 'preview':return{scope:p.ids?'selected':'all',prompt:w.commonPrompt,counts:counts(entries(p.ids))};
          case 'start':w.batchQueue={id:'test-job',scope:p.ids?'selected':'all',prompt:p.prompt,model:'test-model',state:'running',lastError:null,entries:entries(p.ids)};persistBatch();return queueView();
          case 'pause':w.batchQueue.state='paused';persistBatch();return queueView();
          case 'resume':w.batchQueue.state='running';w.batchQueue.lastError=null;persistBatch();return queueView();
          case 'cancel':w.batchQueue.state='cancelled';w.batchQueue.entries.forEach((e:any)=>{if(e.state==='pending')e.state='cancelled'});persistBatch();return queueView();
          case 'retryFailed':w.batchQueue.state='running';w.batchQueue.entries.forEach((e:any)=>{if(e.state==='failed')e.state='pending'});persistBatch();return queueView();
          case 'drafts':{const all=Object.values(w.captionDrafts).map((d:any)=>({...draftView(d),title:d.title}));return{items:all.slice(p.offset,p.offset+p.limit),total:all.length};}
          case 'getDraft':return draftView(w.captionDrafts[p.itemId]);
          case 'keepDraft':{const d=w.captionDrafts[p.itemId];if(d.draftRevision!==p.draftRevision)throw new Error('Draft changed');d.caption=p.caption;d.draftRevision++;persistBatch();return draftView(d);}
          case 'discardDraft':delete w.captionDrafts[p.itemId];persistBatch();return null;
          case 'saveAllDrafts':{let saved=0;const conflicts:string[]=[];for(const d of Object.values(w.captionDrafts) as any[]){const i=items.find(i=>i.id===d.itemId);if(i.caption||d.stale){conflicts.push(`${d.title}: source changed`);continue}i.caption=d.caption;delete w.captionDrafts[d.itemId];saved++;}persist();persistBatch();return{saved,conflicts};}
          default:throw new Error(`Unimplemented batch fixture command ${p.action}`);
        }
      },
      agent: async (p: any) => {
        w.calls.push({ ...p, agent: true });
        if (w.agentError && ['generate', 'models'].includes(p.action)) throw new Error(w.agentError);
        switch (p.action) {
          case 'status': return structuredClone(w.agentStatus);
          case 'signIn': w.agentStatus = { accounts: [{ id: 'test-registration', email: 'test@example.invalid', connected: true, planEnabled: true }], active: 'test-registration', model: null, connecting: false, lastError: null }; return structuredClone(w.agentStatus);
          case 'models': return [{ id: 'test-vision', name: 'Test vision model' }];
          case 'selectModel': w.agentStatus.model = p.modelId; return structuredClone(w.agentStatus);
          case 'draft': { const item = items.find(i => i.id === p.itemId); return { itemId: item.id, expectedHash: 'fixture-hash', caption: item.caption, replaceExisting: true, history: item.captionHistory ?? [], model: null }; }
          case 'generate': { const item = items.find(i => i.id === p.itemId); if (item.caption) throw new Error('Existing captions are preserved'); return { itemId: item.id, expectedHash: 'fixture-hash', caption: 'A generated caption from this item’s references.', model: w.agentStatus.model }; }
          case 'revise': {if (w.agentError) throw new Error(w.agentError);const caption=`Revised: ${p.currentCaption}`;const d=w.captionDrafts[p.itemId];if(d){d.caption=caption;d.draftRevision++;persistBatch();}return { itemId: p.itemId, expectedHash: p.expectedHash, caption, model: w.agentStatus.model,draftRevision:d?.draftRevision };}
          case 'apply': { const item = items.find(i => i.id === p.itemId); if (item.caption && !p.replaceExisting) throw new Error('Item already has a caption'); if (item.caption && item.caption !== p.caption) item.captionHistory = [...(item.captionHistory ?? []), { caption: item.caption, savedAt: 1 }].slice(-20); item.caption = p.caption;delete w.captionDrafts[p.itemId];persistBatch();persist(); return structuredClone(item); }
          case 'disconnect': w.agentStatus.accounts[0].connected = false; w.agentStatus.model = null; return { message: 'Signed out', status: structuredClone(w.agentStatus) };
          default: throw new Error(`Unimplemented agent fixture command ${p.action}`);
        }
      },
      choose: async (directory: boolean) => directory ? 'C:/Test collection' : ['C:/source/New image.png'],
      confirm: async () => true,
      request: async (p: any) => {
        w.calls.push(p);
        switch (p.action) {
          case 'open': return { root: 'C:/Test collection', settings: { schemaVersion: 1, defaultStorage: 'copy',captionInstructions:w.commonPrompt }, report: { indexed: items.length, warnings: [] } };
          case 'captionInstructions':w.commonPrompt=p.prompt;localStorage.setItem('fixture-prompt',p.prompt);return{schemaVersion:1,defaultStorage:'copy',captionInstructions:p.prompt};
          case 'list': { const q = p.query.toLowerCase(); const found = items.filter(i => [i.title, i.caption, i.notes, ...i.assets.map((a: any) => a.filename)].join(' ').toLowerCase().includes(q)).map(summary).filter(i => p.filter === 'all' || p.filter === 'empty-caption' && i.emptyCaption || p.filter === 'missing-original' && i.missingOriginal || p.filter === 'missing-files' && i.missingFiles); return { items: found.slice(p.offset, p.offset + p.limit), total: found.length }; }
          case 'previews': return Object.fromEntries(items.filter(i => p.ids.includes(i.id)).flatMap(i => i.assets.map((a: any) => [a.id, thumbnail])));
          case 'get': return structuredClone(items.find(i => i.id === p.id));
          case 'save': { if (w.failSave) throw new Error('Disk is read-only'); const saved = { ...p.item, revision: p.item.revision + 1 }; items = items.map(i => i.id === saved.id ? saved : i); persist(); return structuredClone(saved); }
          case 'create': { const created = { schemaVersion: 1, id: `new-${items.length}`, title: 'Untitled item', caption: '', notes: '', assets: [], revision: 1, createdAt: 0, updatedAt: 0 }; items.push(created); persist(); return structuredClone(created); }
          case 'import': { const a = { id: `attachment-${items.length}-${Date.now()}`, role: 'original', storage: p.storage, path: p.paths[0], filename: 'New image.png' }; if (p.itemId) { const i = items.find(i => i.id === p.itemId); a.role = i.assets.some((a: any) => a.role === 'original') ? 'reference' : 'original'; i.assets.push(a); persist(); return [structuredClone(i)]; } const i = { schemaVersion: 1, id: `new-${items.length}`, title: 'New image', caption: '', notes: '', revision: 1, assets: [a] }; items.push(i); persist(); return [structuredClone(i)]; }
          case 'importWebImage':
          case 'importImage': { const a = { id: `drop-${Date.now()}`, role: 'original', storage: 'copy', path: 'assets/dropped.png', filename: p.filename || 'dropped.png', sourceUrl: p.url || p.sourceUrl }; if (p.itemId) { const item=items.find(i => i.id === p.itemId); a.role=item.assets.some((a: any) => a.role === 'original') ? 'reference' : 'original'; item.assets.push(a); persist(); return structuredClone(item); } const item={schemaVersion:1,id:`drop-item-${items.length}`,title:'Dropped image',caption:'',notes:'',revision:1,assets:[a]};items.push(item);persist();return structuredClone(item); }
          case 'role': { const i = items.find(i => i.id === p.itemId); i.assets.forEach((a: any) => { if (p.role === 'original' && a.role === 'original') a.role = 'reference'; if (a.id === p.assetId) a.role = p.role; }); persist(); return structuredClone(i); }
          case 'detach': { const i = items.find(i => i.id === p.itemId); i.assets = i.assets.filter((a: any) => a.id !== p.assetId); persist(); return structuredClone(i); }
          case 'move': { const from = items.find(i => i.id === p.from); const to = items.find(i => i.id === p.to); const a = from.assets.find((a: any) => a.id === p.assetId); to.assets.push({ ...a, role: 'reference' }); from.assets = from.assets.filter((a: any) => a.id !== p.assetId); persist(); return null; }
          case 'check': return w.missing.filter((m: any) => p.ids.includes(m.itemId));
          case 'refresh': return { indexed: items.length, warnings: [] };
          case 'settings': return { schemaVersion: 1, defaultStorage: p.storage };
          case 'remove': items = items.filter(i => !p.ids.includes(i.id)); persist(); return null;
          case 'export': return 'C:/Exports/Nabor-export';
          default: throw new Error(`Unimplemented fixture command ${p.action}`);
        }
      },
    };
  }, { count });
  await page.goto('/');
}
async function open(page: Page) { await page.getByRole('button', { name: 'Open existing', exact: true }).click(); await expect(page.getByLabel('Search collection')).toBeVisible(); }

test('collection start screen exposes create and open actions', async ({ page }) => {
  await page.goto('/'); await expect(page.getByRole('heading', { name: 'Open or create a collection' })).toBeVisible();
  await page.getByRole('button', { name: 'Create a collection', exact: true }).click(); await expect(page.getByRole('alert')).toContainText('desktop app');
});

test('capture, edit, attach, assign roles and reopen', async ({ page }) => {
  await fixture(page); await open(page); await page.getByRole('button', { name: 'Open Photo 00000', exact: true }).click();
  await page.getByLabel('CAPTION', { exact: true }).fill('A caption for the first image');
  await expect(page.getByText('Saved locally')).toBeVisible();
  await page.getByLabel('Import storage').selectOption('link'); await page.getByRole('button', { name: 'Attach', exact: true }).click();
  await expect(page.getByText('Linked file', { exact: true })).toBeVisible(); await page.getByLabel('Role for New image.png').selectOption('caption-source');
  await page.getByLabel('Role for New image.png').selectOption('original'); await expect(page.getByLabel('Role for Photo 0.png')).toHaveValue('reference');
  await page.getByLabel('Close item').click(); await page.reload(); await open(page); await page.getByRole('button', { name: 'Open Photo 00000', exact: true }).click();
  await expect(page.getByLabel('CAPTION', { exact: true })).toHaveValue('A caption for the first image'); await expect(page.getByLabel('Role for New image.png')).toHaveValue('original');
  await page.screenshot({ path: 'test-results/collection.png', fullPage: true });
});

test('save failure retains edits and blocks closing until retry', async ({ page }) => {
  await fixture(page); await open(page); await page.getByRole('button', { name: 'Open Photo 00000', exact: true }).click();
  await page.evaluate(() => { (window as any).failSave = true; }); await page.getByLabel('CAPTION', { exact: true }).fill('Keep this text');
  await expect(page.getByRole('alert')).toContainText('Disk is read-only'); await page.getByLabel('Close item').click();
  await expect(page.getByLabel('CAPTION', { exact: true })).toHaveValue('Keep this text');
  await page.evaluate(() => { (window as any).failSave = false; }); await page.getByRole('button', { name: 'Retry save' }).click(); await expect(page.getByText('Saved locally')).toBeVisible();
});

test('search, batch export, missing-file exclusion and removal', async ({ page }) => {
  await fixture(page); await open(page); await page.getByLabel('Search collection').fill('00001');
  await expect(page.getByRole('button', { name: 'Open Photo 00000', exact: true })).toHaveCount(0); await expect(page.getByRole('button', { name: 'Open Photo 00001', exact: true })).toBeVisible();
  await page.getByLabel('Select Photo 00001', { exact: true }).check();
  await page.evaluate(() => { (window as any).missing = [{ itemId: 'item-1', assetId: 'asset-1', title: 'Photo 00001', filename: 'Photo 1.png' }]; });
  await page.getByRole('button', { name: 'Export selected', exact: true }).click(); await expect(page.getByRole('alert')).toContainText('Export paused');
  expect(await page.evaluate(() => (window as any).calls.filter((p: any) => p.action === 'export').length)).toBe(0);
  await page.evaluate(() => { (window as any).missing = []; }); await page.getByRole('button', { name: 'Export selected', exact: true }).click(); await expect(page.getByText(/Package ready/)).toBeVisible();
  await page.getByRole('button', { name: 'Remove', exact: true }).click(); await expect(page.getByRole('button', { name: 'Open Photo 00001', exact: true })).toHaveCount(0);
});

test('move attachment to another item', async ({ page }) => {
  await fixture(page); await open(page); await page.getByRole('button', { name: 'Open Photo 00000', exact: true }).click();
  await page.getByRole('button', { name: 'Move to item', exact: true }).click(); await page.getByRole('button', { name: 'Photo 00001', exact: false }).last().click();
  await expect(page.getByLabel('Role for Photo 0.png')).toHaveCount(0); await page.getByLabel('Close item').click(); await page.getByRole('button', { name: 'Open Photo 00001', exact: true }).click();
  await expect(page.getByLabel('Role for Photo 0.png')).toHaveValue('reference');
});

test('ten thousand items paginate and only visible cards render', async ({ page }) => {
  await fixture(page, 10_000); await open(page); await expect(page.getByText('1–100 of 10,000')).toBeVisible();
  expect(await page.locator('.item-card').count()).toBeLessThan(40);
  await page.getByLabel('Next page').click(); await expect(page.getByText('101–200 of 10,000')).toBeVisible();
  await page.getByLabel('Search collection').fill('09999'); await expect(page.getByRole('button', { name: 'Open Photo 09999', exact: true })).toBeVisible();
});

test('connect ChatGPT, choose an account model, and review before saving a caption', async ({ page }) => {
  await fixture(page); await open(page);
  await page.getByRole('button', { name: 'Agent settings', exact: true }).click();
  await page.getByRole('button', { name: 'Continue with ChatGPT', exact: true }).click();
  await expect(page.getByText('ChatGPT plan access granted', { exact: true })).toBeVisible();
  await page.getByLabel('MODEL', { exact: true }).selectOption('test-vision');
  await page.getByLabel('Close agent settings').click();
  await page.getByRole('button', { name: 'Open Photo 00000', exact: true }).click();
  await page.getByRole('button', { name: 'Generate caption', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Review caption' })).toBeVisible();
  await expect(page.getByLabel('CAPTION', { exact: true })).toHaveValue('');
  await page.getByLabel('Caption draft', { exact: true }).fill('Edited agent caption');
  await page.getByRole('button', { name: 'Save caption', exact: true }).click();
  await expect(page.getByLabel('CAPTION', { exact: true })).toHaveValue('Edited agent caption');
  await expect(page.getByRole('button', { name: 'Revise with agent', exact: true })).toBeEnabled();
});

test('revise the manually edited draft, restore a version, and retain text after a failed revision', async ({ page }) => {
  await fixture(page); await open(page); await page.getByRole('button', { name: 'Open Photo 00000', exact: true }).click();
  await page.getByRole('button', { name: 'Generate caption', exact: true }).click();
  const editor=page.getByRole('dialog', { name: 'Review caption' });
  await editor.getByLabel('Caption draft', { exact: true }).fill('Manual correction: 25 cm');
  await editor.getByLabel('REVISION INSTRUCTIONS').fill('Make it shorter');
  await editor.getByRole('button', { name: 'Revise with agent', exact: true }).click();
  await expect(editor.getByLabel('Caption draft', { exact: true })).toHaveValue('Revised: Manual correction: 25 cm');
  const sent=await page.evaluate(() => (window as any).calls.filter((p: any) => p.agent && p.action==='revise').at(-1));
  expect(sent.currentCaption).toBe('Manual correction: 25 cm'); expect(sent.instructions).toBe('Make it shorter');
  await editor.getByLabel('Caption history').selectOption({ label: '1. First draft — A generated caption from this item’s references.' });
  await editor.getByRole('button', { name: 'Restore', exact: true }).click();
  await expect(editor.getByLabel('Caption draft', { exact: true })).toHaveValue('A generated caption from this item’s references.');
  await editor.getByLabel('Caption draft', { exact: true }).fill('Keep these manual edits');
  await editor.getByLabel('REVISION INSTRUCTIONS').fill('Try another style');
  await page.evaluate(() => { (window as any).agentError='Generation interrupted'; });
  await editor.getByRole('button', { name: 'Revise with agent', exact: true }).click();
  await expect(editor.getByRole('alert')).toContainText('Generation interrupted');
  await expect(editor.getByLabel('Caption draft', { exact: true })).toHaveValue('Keep these manual edits');
  await expect(editor.getByLabel('REVISION INSTRUCTIONS')).toHaveValue('Try another style');
  await expect(page.getByLabel('CAPTION', { exact: true })).toHaveValue('');
  await page.screenshot({path:'test-results/caption-revision.png'});
});

test('revise saved captions explicitly, discard replacements, and restore saved history after reopening', async ({ page }) => {
  await fixture(page); await open(page); await page.getByRole('button', { name: 'Open Photo 00000', exact: true }).click();
  await page.getByLabel('CAPTION', {exact:true}).fill('Original saved caption');await expect(page.getByText('Saved locally')).toBeVisible();
  await page.getByRole('button', { name: 'Revise with agent', exact: true }).click();
  let editor=page.getByRole('dialog', {name:'Review caption'});
  await editor.getByLabel('Caption draft', {exact:true}).fill('Discard this replacement');await editor.getByRole('button',{name:'Discard',exact:true}).click();
  await expect(page.getByLabel('CAPTION',{exact:true})).toHaveValue('Original saved caption');
  await page.getByRole('button', {name:'Revise with agent',exact:true}).click();editor=page.getByRole('dialog',{name:'Review caption'});
  await editor.getByLabel('Caption draft',{exact:true}).fill('Accepted replacement');await editor.getByRole('button',{name:'Save caption',exact:true}).click();
  await expect(page.getByLabel('CAPTION',{exact:true})).toHaveValue('Accepted replacement');
  await page.reload();await open(page);await page.getByRole('button',{name:'Open Photo 00000',exact:true}).click();
  await page.getByRole('button',{name:'History',exact:true}).click();editor=page.getByRole('dialog',{name:'Review caption'});
  await editor.getByLabel('Caption history').selectOption('saved-0');await editor.getByRole('button',{name:'Restore',exact:true}).click();
  await editor.getByRole('button',{name:'Save caption',exact:true}).click();await expect(page.getByLabel('CAPTION',{exact:true})).toHaveValue('Original saved caption');
});

test('agent failures leave the caption empty and drafts can be discarded', async ({ page }) => {
  await fixture(page); await open(page); await page.getByRole('button', { name: 'Open Photo 00000', exact: true }).click();
  await page.evaluate(() => { (window as any).agentError = 'ChatGPT plan usage is unavailable for this account'; });
  await page.getByRole('button', { name: 'Generate caption', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('unavailable for this account');
  await expect(page.getByLabel('CAPTION', { exact: true })).toHaveValue('');
  await page.evaluate(() => { (window as any).agentError = ''; });
  await page.getByRole('button', { name: 'Generate caption', exact: true }).click();
  await page.getByRole('button', { name: 'Discard', exact: true }).click();
  await expect(page.getByLabel('CAPTION', { exact: true })).toHaveValue('');
});

async function webDrop(page:Page,selector:string,html:string,uri:string){
  await page.locator(selector).first().evaluate((element,{html,uri})=>{
    const data=new DataTransfer();if(html)data.setData('text/html',html);if(uri)data.setData('text/uri-list',uri);
    element.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer:data}));
  },{html,uri});
}

test('browser image HTML selects the image URL, creates an inbox item, and retains its source',async({page})=>{
  await fixture(page);await open(page);
  await webDrop(page,'.inbox','<a href="https://example.com/product"><img src="https://images.example.com/photo.jpg?size=large"></a>','https://example.com/product');
  await expect(page.getByRole('button',{name:'Open Dropped image',exact:true})).toBeVisible();
  const sent=await page.evaluate(()=>(window as any).calls.find((p:any)=>p.action==='importWebImage'));
  expect(sent.url).toBe('https://images.example.com/photo.jpg?size=large');expect(sent.itemId).toBeNull();
  await page.getByRole('button',{name:'Open Dropped image',exact:true}).click();
  await expect(page.getByLabel('Role for dropped.png')).toHaveValue('original');
  await expect(page.getByRole('button',{name:'Source',exact:true})).toHaveAttribute('title','https://images.example.com/photo.jpg?size=large');
});

test('browser drops fill a blank original then attach references, including drops on cards',async({page})=>{
  await fixture(page);await open(page);await page.getByRole('button',{name:'Blank item',exact:true}).click();
  await webDrop(page,'[data-item-tray]','','https://images.example.com/first.png');
  await expect(page.getByLabel('Role for dropped.png')).toHaveValue('original');
  await webDrop(page,'[data-item-tray]','','https://images.example.com/second.png');
  await expect(page.getByLabel('Role for dropped.png').nth(1)).toHaveValue('reference');
  await page.getByLabel('Close item').click();
  await webDrop(page,'[data-drop-item="item-0"]','','https://images.example.com/third.png');
  await page.getByRole('button',{name:'Open Photo 00000',exact:true}).click();
  await expect(page.getByLabel('Role for dropped.png')).toHaveValue('reference');
});

test('local file byte drops still work and reject unsupported browser-only data',async({page})=>{
  await fixture(page);await open(page);
  await page.locator('.inbox').evaluate(element=>{
    const data=new DataTransfer();data.items.add(new File([new Uint8Array([1,2,3])],'local.png',{type:'image/png'}));
    element.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer:data}));
  });
  await expect(page.getByRole('button',{name:'Open Dropped image',exact:true})).toBeVisible();
  const sent=await page.evaluate(()=>(window as any).calls.find((p:any)=>p.action==='importImage'));
  expect(sent.bytesBase64).toBe('AQID');expect(sent.filename).toBe('local.png');expect(sent.sourceUrl).toBeNull();
  await webDrop(page,'.inbox','<img src="blob:https://example.com/private-image">','');
  await expect(page.getByRole('alert')).toContainText('saved or copied first');
});

test('common prompt and entire collection preview include items outside search and current page',async({page})=>{
  await fixture(page,125);await open(page);
  await page.getByRole('button',{name:'Collection settings',exact:true}).click();await page.getByLabel('CAPTION INSTRUCTIONS').fill('Use concise English captions');await page.getByRole('button',{name:'Save instructions',exact:true}).click();
  await page.getByLabel('Search collection').fill('00124');await expect(page.getByRole('button',{name:'Open Photo 00124',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Generate entire collection',exact:true}).click();const modal=page.getByRole('dialog',{name:'Generate captions',exact:true});
  await expect(modal.getByText('125 eligible', {exact:true})).toBeVisible();await expect(modal.locator('.prompt-preview')).toHaveText('Use concise English captions');
  const sent=await page.evaluate(()=>(window as any).calls.filter((p:any)=>p.batch&&p.action==='preview').at(-1));expect(sent.ids).toBeNull();
  await page.getByLabel('Close generation preview').click();await page.getByLabel('Clear search').click();await page.getByLabel('Select Photo 00000',{exact:true}).check();await page.getByLabel('Next page').click();await page.getByLabel('Select Photo 00100',{exact:true}).check();await page.getByRole('button',{name:'Generate selected',exact:true}).click();await expect(page.getByText('2 eligible',{exact:true})).toBeVisible();const selected=await page.evaluate(()=>(window as any).calls.filter((p:any)=>p.batch&&p.action==='preview').at(-1));expect(new Set(selected.ids)).toEqual(new Set(['item-0','item-100']));
});

test('selected run can pause, survive reload, resume, and keep results as reviewable drafts',async({page})=>{
  await fixture(page);await open(page);await page.evaluate(()=>{(window as any).batchHold=true});
  await page.getByLabel('Select Photo 00000',{exact:true}).check();await page.getByLabel('Select Photo 00001',{exact:true}).check();
  await page.getByRole('button',{name:'Generate selected',exact:true}).click();await expect(page.getByText('2 eligible',{exact:true})).toBeVisible();await page.getByRole('button',{name:'Start generation',exact:true}).click();
  await expect(page.getByLabel('Caption generation progress')).toContainText('running');await page.getByRole('button',{name:'Pause',exact:true}).click();await expect(page.getByLabel('Caption generation progress')).toContainText('paused');
  await page.reload();await open(page);await expect(page.getByLabel('Caption generation progress')).toContainText('paused');await page.getByRole('button',{name:'Resume',exact:true}).click();
  await expect(page.getByLabel('Caption generation progress')).toContainText('completed',{timeout:12000});
  await page.getByRole('button',{name:/^Caption drafts/}).click();let panel=page.getByRole('dialog',{name:'Caption drafts',exact:true});await expect(panel.getByRole('heading',{name:'Caption drafts (2)',exact:true})).toBeVisible();
  await panel.getByRole('button',{name:'Review',exact:true}).first().click();const editor=page.getByRole('dialog',{name:'Review caption',exact:true});await editor.getByLabel('Caption draft',{exact:true}).fill('Manual draft improvement');await editor.getByRole('button',{name:'Keep draft',exact:true}).click();await expect(panel).toContainText('Manual draft improvement');
  await page.getByLabel('Close caption drafts').click();await page.getByRole('button',{name:'Open Photo 00000',exact:true}).click();await expect(page.getByLabel('CAPTION',{exact:true})).toHaveValue('');await page.getByLabel('Close item').click();
  await page.getByRole('button',{name:/^Caption drafts/}).click();panel=page.getByRole('dialog',{name:'Caption drafts',exact:true});await panel.getByRole('button',{name:'Save all drafts',exact:true}).click();await expect(panel.getByRole('heading',{name:'Caption drafts (0)',exact:true})).toBeVisible();
});

test('batch usage limits pause and existing drafts are skipped on subsequent runs',async({page})=>{
  await fixture(page);await open(page);await page.evaluate(()=>{(window as any).batchFail=true});
  await page.getByRole('button',{name:'Generate entire collection',exact:true}).click();await page.getByRole('button',{name:'Start generation',exact:true}).click();
  await expect(page.getByLabel('Caption generation progress')).toContainText('usage_limit_exceeded',{timeout:8000});
  await page.evaluate(()=>{(window as any).batchFail=false});await page.getByRole('button',{name:'Resume',exact:true}).click();await expect(page.getByLabel('Caption generation progress')).toContainText('completed',{timeout:12000});
  await page.getByRole('button',{name:'Generate entire collection',exact:true}).click();const preview=page.getByRole('dialog',{name:'Generate captions',exact:true});await expect(preview.getByText('3 existing drafts',{exact:true})).toBeVisible();await expect(preview.getByRole('button',{name:'Start generation',exact:true})).toBeDisabled();
});

test('retry failed items and cancel pending work without discarding completed drafts',async({page})=>{
  await fixture(page);await open(page);await page.evaluate(()=>{const w=window as any;w.batchQueue={id:'failed-job',scope:'all',prompt:'Stored prompt',model:'test-model',state:'completed',lastError:null,entries:[{itemId:'item-0',title:'Photo 00000',state:'failed',error:'Unavailable image'}]};});
  await expect(page.getByRole('button',{name:'Retry failed',exact:true})).toBeVisible({timeout:6000});await page.getByRole('button',{name:'Retry failed',exact:true}).click();await expect(page.getByLabel('Caption generation progress')).toContainText('1 drafts',{timeout:6000});
  await page.evaluate(()=>{(window as any).batchHold=true});await page.getByRole('button',{name:'Generate entire collection',exact:true}).click();await expect(page.getByText('1 existing drafts',{exact:true})).toBeVisible();await page.getByRole('button',{name:'Start generation',exact:true}).click();await expect(page.getByRole('button',{name:'Cancel',exact:true})).toBeVisible();await page.getByRole('button',{name:'Cancel',exact:true}).click();await expect(page.getByLabel('Caption generation progress')).toContainText('cancelled');
  await page.getByRole('button',{name:/^Caption drafts/}).click();await expect(page.getByRole('heading',{name:'Caption drafts (1)',exact:true})).toBeVisible();
});

test('item conversation survives reopen and keeps discussion separate from saved text', async ({page}) => {
  await fixture(page); await open(page);
  await page.getByRole('button',{name:'Open Photo 00000',exact:true}).click();
  await page.getByRole('button',{name:'Ask agent',exact:true}).click();
  const panel = page.getByRole('dialog',{name:'Ask agent',exact:true});
  await expect(panel).toContainText('Photo 00000');
  await panel.getByLabel('Question for agent').fill('What is missing?'); await panel.getByRole('button',{name:'Send',exact:true}).click();
  await expect(panel).toContainText('Reply to: What is missing?');
  await panel.getByLabel('Question for agent').fill('Explain the second point'); await panel.getByLabel('Question for agent').press('Control+Enter');
  await expect(panel).toContainText('Reply to: Explain the second point');
  await page.getByLabel('Close conversation').click();
  await expect(page.getByLabel('CAPTION',{exact:true})).toHaveValue(''); await expect(page.getByLabel('INSTRUCTIONS',{exact:true})).toHaveValue('');
  await page.reload(); await open(page); await page.getByRole('button',{name:'Open Photo 00000',exact:true}).click(); await page.getByRole('button',{name:'Ask agent',exact:true}).click();
  await expect(panel.locator('.conversation-answer')).toHaveCount(2);
  await page.getByLabel('Close conversation').click(); await page.getByLabel('Close item').click();
  await page.getByRole('button',{name:'Open Photo 00001',exact:true}).click(); await page.getByRole('button',{name:'Ask agent',exact:true}).click();
  await expect(panel).toContainText('Photo 00001'); await expect(panel.locator('.conversation-turn')).toHaveCount(0);
});

test('conversation reply actions append notes, copy, and require caption review', async ({page,context}) => {
  await context.grantPermissions(['clipboard-read','clipboard-write']);
  await fixture(page); await open(page); await page.getByRole('button',{name:'Open Photo 00000',exact:true}).click();
  await page.getByLabel('CAPTION',{exact:true}).fill('User caption'); await page.getByLabel('INSTRUCTIONS',{exact:true}).fill('User notes');
  await page.getByRole('button',{name:'Ask agent',exact:true}).click(); const panel=page.getByRole('dialog',{name:'Ask agent',exact:true});
  await panel.getByLabel('Question for agent').fill('Suggest a caption'); await panel.getByRole('button',{name:'Send',exact:true}).click();
  await panel.getByRole('button',{name:'Copy',exact:true}).click(); await expect(panel.getByRole('status')).toHaveText('Copied.'); expect(await page.evaluate(()=>navigator.clipboard.readText())).toBe('Reply to: Suggest a caption');
  await panel.getByRole('button',{name:'Add to notes',exact:true}).click(); await expect(panel.getByRole('status')).toHaveText('Added to instructions.');
  await panel.getByRole('button',{name:'Use as caption',exact:true}).click();
  let editor=page.getByRole('dialog',{name:'Review caption',exact:true}); await expect(editor.getByLabel('Caption draft',{exact:true})).toHaveValue('Reply to: Suggest a caption'); await editor.getByRole('button',{name:'Discard',exact:true}).click();
  await expect(page.getByLabel('CAPTION',{exact:true})).toHaveValue('User caption'); await expect(page.getByLabel('INSTRUCTIONS',{exact:true})).toHaveValue('User notes\n\nReply to: Suggest a caption');
  await page.getByRole('button',{name:'Ask agent',exact:true}).click(); await panel.getByRole('button',{name:'Use as caption',exact:true}).click();
  editor=page.getByRole('dialog',{name:'Review caption',exact:true}); await editor.getByRole('button',{name:'Save caption',exact:true}).click();
  await expect(page.getByLabel('CAPTION',{exact:true})).toHaveValue('Reply to: Suggest a caption');
  await page.getByRole('button',{name:'Ask agent',exact:true}).click(); await panel.getByRole('button',{name:'Clear conversation',exact:true}).click(); await expect(panel.locator('.conversation-turn')).toHaveCount(0);
  await page.getByLabel('Close conversation').click(); await expect(page.getByLabel('CAPTION',{exact:true})).toHaveValue('Reply to: Suggest a caption');
});

test('failed questions remain saved for retry without repeating completed replies', async ({page}) => {
  await fixture(page); await open(page); await page.getByRole('button',{name:'Open Photo 00000',exact:true}).click(); await page.getByRole('button',{name:'Ask agent',exact:true}).click();
  const panel=page.getByRole('dialog',{name:'Ask agent',exact:true});
  await page.evaluate(()=>{(window as any).talkError='ChatGPT usage is limited'});
  await panel.getByLabel('Question for agent').fill('Identify the material'); await panel.getByRole('button',{name:'Send',exact:true}).click(); await expect(panel).toContainText('ChatGPT usage is limited');
  await page.getByLabel('Close conversation').click(); await page.getByRole('button',{name:'Ask agent',exact:true}).click();
  await panel.getByRole('button',{name:'Retry question',exact:true}).click(); await expect(panel).toContainText('ChatGPT usage is limited');
  await page.evaluate(()=>{(window as any).talkError=''}); await panel.getByRole('button',{name:'Retry question',exact:true}).click();
  await expect(panel.locator('.conversation-turn')).toHaveCount(1); await expect(panel).toContainText('Reply to: Identify the material'); await expect(panel.getByRole('button',{name:'Retry question',exact:true})).toHaveCount(0);
});
