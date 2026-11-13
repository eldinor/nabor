import { useEffect, useState } from 'react';
import { LoaderCircle, RefreshCw, X } from 'lucide-react';
import { agentRequest } from './api';
import type { AgentModel, AgentStatus } from './types';

export default function AgentSettings({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [models, setModels] = useState<AgentModel[]>([]);
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [notice, setNotice] = useState('');
  const account = status?.accounts.find(a => a.id === status.active);
  const run = async (operation: () => Promise<void>) => {
    setBusy(true); setError(''); setNotice('');
    try { await operation(); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  useEffect(() => { let active = true; agentRequest<AgentStatus>('status').then(s => { if (active) setStatus(s); }).catch(e => { if (active) setError(String(e)); }); return () => { active = false; }; }, []);
  useEffect(() => {
    if (!status?.connecting) return;
    let active = true; let pending = false;
    const timer = setInterval(async () => { if (pending) return; pending = true; try { const next = await agentRequest<AgentStatus>('status'); if (active) { setStatus(next); if (next.lastError) setError(next.lastError); } } catch (e) { if (active) setError(String(e)); } finally { pending = false; } }, 1000);
    return () => { active = false; clearInterval(timer); };
  }, [status?.connecting]);
  const fetchModels = async () => { setModels(await agentRequest<AgentModel[]>('models')); };
  useEffect(() => { setModels([]); if (account?.connected && account.planEnabled) run(fetchModels); }, [account?.id, account?.connected, account?.planEnabled]);
  const signIn = (accountId: string | null) => run(async () => { setStatus(await agentRequest<AgentStatus>('signIn', { accountId })); });
  return <div className="modal-backdrop"><section className="modal agent-settings" role="dialog" aria-modal="true" aria-label="Agent settings">
    <div className="modal-heading"><h2>Agent settings</h2><button aria-label="Close agent settings" onClick={onClose}><X size={20} /></button></div>
    <label className="field-label" htmlFor="agent-account">ChatGPT account</label>
    {status?.accounts.length ? <select id="agent-account" disabled={busy || status.connecting} value={status.active ?? ''} onChange={e => run(async () => { setStatus(await agentRequest<AgentStatus>('selectAccount', { accountId: e.target.value })); })}>{status.accounts.map((a, i) => <option key={a.id} value={a.id}>{a.email} · Account {i + 1}{!a.connected ? ' (signed out)' : ''}</option>)}</select> : <p className="agent-status">Not connected</p>}
    {status?.connecting ? <div className="agent-connection"><span><LoaderCircle className="spin" size={15} />Complete authorization in your browser.</span><button className="button secondary" disabled={busy} onClick={() => run(async () => { await agentRequest('cancelSignIn'); })}>Cancel</button></div> : <div className="agent-buttons">
      <button className="button primary" data-oai-siwc-button="" disabled={busy || !status} onClick={() => signIn(account?.id ?? null)}>Continue with ChatGPT</button>
      {!!status?.accounts.length && <button className="button secondary" disabled={busy} onClick={() => signIn(null)}>Add account</button>}
      {account?.connected && <button className="button secondary" disabled={busy} onClick={() => run(async () => { const result = await agentRequest<{ message: string; status: AgentStatus }>('disconnect'); setStatus(result.status); setNotice(result.message); })}>Sign out</button>}
    </div>}
    {account?.connected && <p className="agent-status">{account.planEnabled ? 'ChatGPT plan access granted' : 'Signed in. ChatGPT plan access was not granted.'}</p>}
    <label className="field-label" htmlFor="agent-model">Model</label><div className="agent-model-row"><select id="agent-model" disabled={busy || !models.length || status?.connecting} value={status?.model ?? ''} onChange={e => run(async () => { setStatus(await agentRequest<AgentStatus>('selectModel', { modelId: e.target.value })); })}><option value="" disabled>Select a model</option>{models.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}</select><button aria-label="Refresh available models" title="Refresh available models" disabled={busy || !account?.planEnabled || status?.connecting} onClick={() => run(fetchModels)}><RefreshCw size={16} /></button></div>
    <label className="agent-input-setting" title="Exclude the main original image from captions, revisions, batch generation, and Ask agent. Reference images, document text, links, and notes are still included."><input type="checkbox" checked={status?.sendOnlyAttachments ?? false} disabled={busy || !status} onChange={e => { const enabled = e.target.checked; run(async () => { setStatus(await agentRequest<AgentStatus>('setSendOnlyAttachments', { enabled })); }); }} />Send only attachments</label>
    <label className="field-label">Caption behavior</label><p className="agent-status">Generate if empty. Saved captions change only after accepting a revision.</p>
    <p className="agent-disclosure">Generating sends this item’s images, title, and instructions to OpenAI using your ChatGPT plan. Originals remain unchanged. Website links are not opened automatically.</p>
    <a className="agent-usage" href="https://chatgpt.com/settings/usage" target="_blank" rel="noreferrer" onClick={e => { e.preventDefault(); agentRequest('usage').catch(err => setError(String(err))); }}>ChatGPT usage settings</a>
    {busy && <p className="agent-status"><LoaderCircle className="spin" size={14} />Working…</p>}
    {error && <p className="agent-error" role="alert">{error}</p>}{notice && <p className="agent-status" role="status">{notice}</p>}
  </section></div>;
}
