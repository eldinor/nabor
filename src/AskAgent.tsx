import { useEffect, useRef, useState } from 'react';
import { LoaderCircle, X } from 'lucide-react';
import { confirmClearConversation, copyText, talkRequest } from './api';
import type { CaptionDraft, ConversationTurn, ConversationView, Item } from './types';
import { ResizeHandle, usePanelWidth } from './ResizablePanel';

export default function AskAgent({ itemId, itemVersion, locked, initialQuestion, onQuestionChange, onViewImage, onClose, onBusy, onChanged, onCaption }: {
  itemVersion?: number; locked: boolean; initialQuestion: string; onQuestionChange: (value: string) => void;
  onViewImage?: () => void;
  itemId: string; onClose: () => void; onBusy: (busy: boolean) => void;
  onChanged: (item: Item) => Promise<void>; onCaption: (draft: CaptionDraft) => void;
}) {
  const [view, setView] = useState<ConversationView | null>(null);
  const [question, setQuestionValue] = useState(initialQuestion);
  const setQuestion = (value: string) => { setQuestionValue(value); onQuestionChange(value); };
  const panel = usePanelWidth('nabor.agentWidth', 520);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const messages = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let active = true;
    talkRequest<ConversationView>('get', { itemId }).then(v => { if (active) setView(v); }).catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [itemId, itemVersion]);
  useEffect(() => { if (messages.current) messages.current.scrollTop = messages.current.scrollHeight; }, [view?.conversation.revision, busy]);
  const run = async (operation: () => Promise<void>) => {
    setBusy(true); onBusy(true); setError(''); setNotice('');
    try { await operation(); } catch (e) { setError(String(e)); }
    finally { setBusy(false); onBusy(false); }
  };
  const send = (retry?: ConversationTurn) => run(async () => {
    const next = await talkRequest<ConversationView>('ask', { itemId, revision: view!.conversation.revision, question: retry?.question ?? question, retry: retry?.id ?? null });
    setView(next);
    if (!retry) setQuestion('');
  });
  const replyAction = (turn: ConversationTurn, action: 'useCaption' | 'addNotes') => run(async () => {
    const args = { itemId, turnId: turn.id, expectedHash: view!.sourceHash };
    if (action === 'useCaption') { onCaption(await talkRequest<CaptionDraft>(action, args)); }
    else {
      const item = await talkRequest<Item>(action, args);
      await onChanged(item);
      setView(await talkRequest<ConversationView>('get', { itemId }));
      setNotice('Added to instructions.');
    }
  });
  return <section ref={panel.ref} style={{ width: panel.width }} className="ask-agent agent-panel" role="region" aria-label="Ask agent" data-drop-item={itemId}>
    <ResizeHandle label="Resize agent panel" width={panel.width} max={panel.max} onChange={panel.change} onReset={panel.reset} />
    <div className="modal-heading"><h2>Ask agent</h2><button aria-label="Close conversation" disabled={busy || locked} onClick={onClose}><X size={20} /></button></div>
    {view && <><div className="conversation-scope"><strong>{view.title}</strong><span>{view.assetCount} image{view.assetCount === 1 ? '' : 's'} · {view.documentCount ?? 0} document{view.documentCount === 1 ? '' : 's'} · {view.linkCount ?? 0} link{view.linkCount === 1 ? '' : 's'}</span>{onViewImage && <button disabled={busy || locked} onClick={onViewImage}>View image</button>}<button disabled={busy || locked || !view.conversation.turns.length} onClick={() => run(async () => { if (await confirmClearConversation()) setView(await talkRequest<ConversationView>('clear', { itemId, revision: view.conversation.revision })); })}>Clear conversation</button></div>
    <div className="conversation-messages" ref={messages} aria-label="Saved conversation" aria-live="polite">
      {view.conversation.turns.map((turn, index) => <article className="conversation-turn" key={turn.id}>
        <div className="conversation-question"><span>You</span><p>{turn.question}</p></div>
        {turn.answer !== null ? <div className="conversation-answer"><div className="conversation-answer-heading"><span>Agent</span><small>{turn.model}</small>{turn.sourceHash !== view.sourceHash && <small title="This reply used an earlier version of the item">Item changed</small>}</div><p>{turn.answer}</p><div className="conversation-actions"><button disabled={busy || locked} onClick={() => replyAction(turn, 'useCaption')}>Use as caption</button><button disabled={busy || locked} title="Append this reply to the item's Instructions field" onClick={() => replyAction(turn, 'addNotes')}>Add to notes</button><button disabled={busy || locked} onClick={() => run(async () => { await copyText(turn.answer!); setNotice('Copied.'); })}>Copy</button></div></div> : <div className="conversation-failure"><p>{turn.error ?? 'No completed reply was saved. Retry to send this question again.'}</p>{index === view.conversation.turns.length - 1 && <button disabled={busy || locked} onClick={() => send(turn)}>Retry question</button>}</div>}
      </article>)}
      {busy && <LoaderCircle className="spin" size={18} aria-label="Agent operation in progress" />}
    </div>
    <form className="conversation-composer" onSubmit={e => { e.preventDefault(); if (!busy && !locked && question.trim()) send(); }}><textarea aria-label="Question for agent" rows={3} value={question} disabled={busy || locked} placeholder="Ask about this item" onChange={e => setQuestion(e.target.value)} onKeyDown={e => { if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); if (!busy && !locked && question.trim()) send(); } }} /><div><small title="Sends this item's images, extracted document text, caption, instructions, website references, and up to 20 recent exchanges (60,000 bytes) through your selected ChatGPT account. Earlier exchanges remain saved locally.">Current item · recent conversation · links not fetched</small><button className="button primary" type="submit" disabled={busy || locked || !question.trim()}>Send</button></div></form></>}
    {notice && <p className="conversation-notice" role="status">{notice}</p>}
    {error && <p className="agent-error" role="alert">{error}</p>}
  </section>;
}
