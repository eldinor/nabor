// Windows-only packaged-WebView smoke test. Uses real Rust IPC/filesystem and
// asset protocol. Native dialog and system clipboard testing remain manual.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, copyFile, readFile, rename, access, writeFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';

if (process.platform !== 'win32') throw new Error('This smoke harness requires Windows WebView2.');
const root = process.cwd();
await mkdir(path.join(root, 'test-results'), { recursive: true });
const fixture = await mkdtemp(path.join(root, 'test-results', 'native-'));
const collection = path.join(fixture, 'collection');
const original = path.join(fixture, 'original image.png');
const screenshot = path.join(fixture, 'caption screenshot.png');
await copyFile(path.join(root, 'src-tauri/icons/128x128.png'), original);
await copyFile(original, screenshot);
const executable = process.env.NABOR_EXE || path.join(root, 'src-tauri/target/debug/nabor.exe');
const processHandle = spawn(executable, [], { windowsHide: true, env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9223' }, stdio: ['ignore', 'pipe', 'pipe'] });
processHandle.stdout.on('data', chunk => process.stdout.write(chunk));
processHandle.stderr.on('data', chunk => process.stderr.write(chunk));
processHandle.on('error', error => console.error('Native launch error:', error));
let browser;
let currentPage;
try {
  for (let attempt = 0; attempt < 100; attempt++) {
    try { browser = await chromium.connectOverCDP('http://127.0.0.1:9223'); break; }
    catch { if (processHandle.exitCode !== null) throw new Error(`Native app exited: ${processHandle.exitCode}`); await new Promise(resolve => setTimeout(resolve, 200)); }
  }
  assert(browser, 'WebView2 debug connection did not start');
  const context = browser.contexts()[0];
  let page = context.pages()[0];
  if (!page) page = await context.waitForEvent('page');
  currentPage=page;
  page.setDefaultTimeout(45_000);
  await page.waitForFunction(() => !!window.__TAURI_INTERNALS__);
  await page.getByRole('heading', { name: 'Open or create a collection' }).waitFor();
  if (process.env.NABOR_SKIP_AGENT !== '1') {
  await page.getByRole('button', { name: 'Agent settings', exact: true }).click();
  await page.getByRole('button', { name: 'Continue with ChatGPT', exact: true }).waitFor();
  const agentStatus=await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('agent_request',{payload:{action:'status'}}));
  assert.equal(typeof agentStatus.connecting,'boolean');
  assert(!JSON.stringify(agentStatus).includes('access_token'));
  await page.getByLabel('Close agent settings').click();
  }
  const rpc = payload => page.evaluate(payload => window.__TAURI_INTERNALS__.invoke('request', { payload }), payload);
  await rpc({ action: 'open', path: collection, create: true });
  await rpc({ action: 'import', itemId: null, paths: [original], storage: 'copy' });
  await page.reload();
  await page.getByRole('button', { name: 'Open original image', exact: true }).click();
  await page.getByLabel('CAPTION', { exact: true }).fill('Native caption — kept with the right image.');
  await page.getByText('Saved locally', { exact: true }).waitFor();
  const listed = await rpc({ action: 'list', query: '', filter: 'all', offset: 0, limit: 100 });
  assert.equal(listed.total, 1);
  const itemId = listed.items[0].id;
  await rpc({ action: 'import', itemId, paths: [screenshot], storage: 'link' });
  const item = await rpc({ action: 'get', id: itemId });
  assert.equal(item.caption, 'Native caption — kept with the right image.');
  await rpc({ action: 'role', itemId, assetId: item.assets[1].id, role: 'caption-source' });
  const previewEvent = page.evaluate(() => new Promise((resolve, reject) => {
    const callback = window.__TAURI_INTERNALS__.transformCallback(event => resolve(event.payload), true);
    window.__TAURI_INTERNALS__.invoke('plugin:event|listen', { event: 'thumbnail-ready', target: { kind: 'Any' }, handler: callback }).catch(reject);
  }));
  await rpc({ action: 'previews', ids: [itemId] });
  await previewEvent;
  await page.getByRole('button', { name: 'Close item' }).click();
  await page.getByRole('button', { name: 'Open original image', exact: true }).click();
  // Check that individually granted file URLs load through the actual asset protocol.
  await page.waitForFunction(() => [...document.querySelectorAll('.card-image img, .attachment img')].some(img => img.complete && img.naturalWidth > 0));
  const exported = await rpc({ action: 'export', ids: [itemId], parent: fixture });
  const manifest = JSON.parse(await readFile(path.join(exported, 'items', itemId, 'item.json'), 'utf8'));
  assert.equal(manifest.assets.length, 2);
  for (const asset of manifest.assets) {
    assert.equal(asset.storage, 'copy');
    assert.deepEqual(await readFile(path.join(exported, 'items', itemId, asset.path)), await readFile(original));
  }
  await rename(screenshot, `${screenshot}.moved.png`);
  assert.equal((await rpc({ action: 'check', ids: [itemId] })).length, 1);
  await rpc({ action: 'reconnect', itemId, assetId: item.assets[1].id, path: `${screenshot}.moved.png` });
  assert.equal((await rpc({ action: 'check', ids: [itemId] })).length, 0);
  await page.getByRole('button',{name:'Revise with agent',exact:true}).click();
  let editor=page.getByRole('dialog',{name:'Review caption'});
  await editor.getByLabel('Caption draft',{exact:true}).fill('Native reviewed replacement');
  await editor.getByRole('button',{name:'Save caption',exact:true}).click();
  const revised=await rpc({action:'get',id:itemId});
  assert.equal(revised.caption,'Native reviewed replacement');
  assert.equal(revised.captionHistory.at(-1).caption,'Native caption — kept with the right image.');
  await page.getByRole('button',{name:'History',exact:true}).click();
  editor=page.getByRole('dialog',{name:'Review caption'});
  await editor.getByLabel('Caption history').selectOption('saved-0');
  await editor.getByRole('button',{name:'Restore',exact:true}).click();
  await editor.getByRole('button',{name:'Save caption',exact:true}).click();
  assert.equal((await rpc({action:'get',id:itemId})).caption,'Native caption — kept with the right image.');
  await page.getByLabel('Close item').click();
  const webSource='https://www.w3.org/Icons/w3c_home.png';
  await page.locator('.inbox').evaluate((element,source)=>{const data=new DataTransfer();data.setData('text/html',`<a href="https://www.w3.org/"><img src="${source}"></a>`);data.setData('text/uri-list','https://www.w3.org/');element.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer:data}));},webSource);
  await page.getByRole('button',{name:/^Open w3c/}).waitFor();
  const webItems=await rpc({action:'list',query:'w3c',filter:'all',offset:0,limit:100});
  const webItem=await rpc({action:'get',id:webItems.items[0].id});
  assert.equal(webItem.assets[0].role,'original');assert.equal(webItem.assets[0].sourceUrl,webSource);
  const webPackage=await rpc({action:'export',ids:[webItem.id],parent:fixture});
  const webManifest=JSON.parse(await readFile(path.join(webPackage,'items',webItem.id,'item.json'),'utf8'));
  assert.equal(webManifest.assets[0].sourceUrl,webSource);
  await page.getByRole('button',{name:'Blank item',exact:true}).click();
  const localBytes=Array.from(await readFile(original));
  await page.locator('[data-item-tray]').evaluate((element,bytes)=>{const data=new DataTransfer();data.items.add(new File([new Uint8Array(bytes)],'local-drop.png',{type:'image/png'}));element.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer:data}));},localBytes);
  await page.getByLabel('Role for local-drop.png').waitFor();
  assert.equal(await page.getByLabel('Role for local-drop.png').inputValue(),'original');
  await page.locator('[data-item-tray]').evaluate((element,bytes)=>{const data=new DataTransfer();data.items.add(new File([new Uint8Array(bytes)],'local-reference.png',{type:'image/png'}));element.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer:data}));},localBytes);
  await page.getByLabel('Role for local-reference.png').waitFor();
  assert.equal(await page.getByLabel('Role for local-reference.png').inputValue(),'reference');
  // Exercise durable drafts through real IPC without sending an inference request.
  const batch = payload => page.evaluate(payload => window.__TAURI_INTERNALS__.invoke('batch_request', { payload }), payload);
  await rpc({action:'captionInstructions',prompt:'Use concise factual captions'});
  assert.equal((await batch({action:'preview',ids:null})).prompt,'Use concise factual captions');
  assert.equal((await batch({action:'preview',ids:[webItem.id]})).counts.pending,1);
  const record={schemaVersion:1,itemId:webItem.id,title:webItem.title,expectedHash:webItem.sourceHash,caption:'Native persistent draft',model:'test-model',prompt:'Use concise factual captions',accountId:'test-account',jobId:null,createdAt:Math.floor(Date.now()/1000),draftRevision:1};
  await writeFile(path.join(collection,'captions','drafts',`${webItem.id}.json`),JSON.stringify(record));
  await rpc({action:'refresh'});
  assert.equal((await batch({action:'status'})).draftCount,1);
  assert.equal((await batch({action:'preview',ids:[webItem.id]})).counts.skippedDraft,1);
  assert.equal((await rpc({action:'get',id:webItem.id})).caption,'');
  await page.getByLabel('Close item').click();
  await page.getByRole('button',{name:/^Caption drafts/}).click();
  await page.getByRole('button',{name:'Review',exact:true}).click();
  editor=page.getByRole('dialog',{name:'Review caption'});
  await editor.getByLabel('Caption draft',{exact:true}).fill('Native retained edit');
  await editor.getByRole('button',{name:'Keep draft',exact:true}).click();
  assert.equal((await batch({action:'getDraft',itemId:webItem.id})).caption,'Native retained edit');
  await page.getByRole('button',{name:'Save all drafts',exact:true}).click();
  await page.getByRole('heading',{name:'Caption drafts (0)',exact:true}).waitFor();
  assert.equal((await rpc({action:'get',id:webItem.id})).caption,'Native retained edit');
  assert.equal((await batch({action:'status'})).draftCount,0);
  await page.getByLabel('Close caption drafts').click();
  // Seed a saved conversation to exercise native reply actions without inference.
  const currentItem=await rpc({action:'get',id:itemId});
  const turnId=randomUUID();
  const chat={schemaVersion:1,itemId,revision:2,turns:[{id:turnId,question:'What is missing?',answer:'Native discussion reply',error:null,model:'test-model',sourceHash:currentItem.sourceHash,createdAt:1}]};
  await writeFile(path.join(collection,'items',itemId,'conversation.json'),JSON.stringify(chat));
  await page.getByRole('button',{name:'Open original image',exact:true}).click();
  await page.getByRole('button',{name:'Ask agent',exact:true}).click();
  const discussion=page.getByRole('dialog',{name:'Ask agent',exact:true});
  await discussion.getByText('Native discussion reply',{exact:true}).waitFor();
  await discussion.getByRole('button',{name:'Add to notes',exact:true}).click();
  await discussion.getByText('Added to instructions.',{exact:true}).waitFor();
  assert.equal((await rpc({action:'get',id:itemId})).notes,'Native discussion reply');
  assert.equal((await rpc({action:'get',id:itemId})).caption,currentItem.caption);
  await page.screenshot({path:path.join(root,'test-results','native-conversation.png')});
  await discussion.getByRole('button',{name:'Use as caption',exact:true}).click();
  editor=page.getByRole('dialog',{name:'Review caption',exact:true});
  assert.equal(await editor.getByLabel('Caption draft',{exact:true}).inputValue(),'Native discussion reply');
  await editor.getByRole('button',{name:'Discard',exact:true}).click();
  assert.equal((await rpc({action:'get',id:itemId})).caption,currentItem.caption);
  const conversationExport=await rpc({action:'export',ids:[itemId],parent:fixture});
  await assert.rejects(access(path.join(conversationExport,'items',itemId,'conversation.json')));
  await page.getByRole('button',{name:'Ask agent',exact:true}).click();
  await discussion.getByText('Native discussion reply',{exact:true}).waitFor();
  await page.getByLabel('Close conversation').click();
  await page.getByLabel('Close item').click();
  await page.screenshot({ path: path.join(root, 'test-results', 'native-desktop.png') });
  await rpc({ action: 'remove', ids: [itemId] });
  await access(`${screenshot}.moved.png`);
  console.log(JSON.stringify({ result: 'passed', realNativeIPC: true, assetProtocol: true, byteExactExport: true, linkedSourcePreserved: true, fixture, exported }, null, 2));
} catch (error) {
  if (currentPage) { console.error('Native UI alerts:', await currentPage.locator('[role="alert"]').allTextContents().catch(() => [])); await currentPage.screenshot({path:path.join(root,'test-results','native-failure.png')}).catch(()=>{}); }
  throw error;
} finally {
  if (browser) await browser.close();
  processHandle.kill();
}
