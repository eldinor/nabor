use crate::{
    agent::{self, AgentState},
    State,
};
use nabor_core::*;
use serde::Deserialize;
use serde_json::{json, Value};
use std::result::Result;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};
use tauri::{Emitter, Manager};

#[derive(Default)]
pub struct Runtime {
    pub active: AtomicBool,
    control: AtomicU8,
}
#[derive(Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BatchRequest {
    Status,
    Preview {
        ids: Option<Vec<String>>,
    },
    Start {
        ids: Option<Vec<String>>,
        prompt: String,
    },
    Pause,
    Resume,
    Cancel,
    RetryFailed,
    Drafts {
        offset: usize,
        limit: usize,
    },
    GetDraft {
        item_id: String,
    },
    KeepDraft {
        item_id: String,
        draft_revision: u64,
        caption: String,
    },
    DiscardDraft {
        item_id: String,
        draft_revision: u64,
    },
    SaveAllDrafts,
}
pub fn draft_view(c: &Collection, d: CaptionDraftRecord) -> Value {
    let item = c.load(&d.item_id).ok();
    let stale = item
        .as_ref()
        .is_none_or(|i| i.source_hash != d.expected_hash || !i.caption.trim().is_empty());
    json!({"itemId":d.item_id,"expectedHash":d.expected_hash,"caption":d.caption,"model":d.model,"replaceExisting":false,"draftRevision":d.draft_revision,"stale":stale,"history":item.map(|i|i.caption_history).unwrap_or_default()})
}
fn perform(app: tauri::AppHandle, payload: BatchRequest) -> Result<Value, String> {
    let rt = app.state::<Runtime>();
    let mut identity = None;
    if matches!(
        &payload,
        BatchRequest::Start { .. } | BatchRequest::Resume | BatchRequest::RetryFailed
    ) {
        if rt.active.load(Ordering::SeqCst) {
            return Err("A generation worker is already running".into());
        }
        let state = app.state::<AgentState>();
        let mut guard = state.client.lock().map_err(|_| "Agent lock failed")?;
        agent::initialize(&app, &state, &mut guard)?;
        let status = guard.as_ref().unwrap().status();
        let account = status.active.ok_or("Connect ChatGPT in Agent settings")?;
        if !status
            .accounts
            .iter()
            .any(|a| a.id == account && a.connected && a.plan_enabled)
        {
            return Err("ChatGPT plan access is not enabled for the selected account".into());
        }
        identity = Some((account, status.model));
    }
    let state = app.state::<State>();
    let guard = state
        .collection
        .lock()
        .map_err(|_| "Collection lock failed")?;
    let c = guard.as_ref().ok_or("Open a collection first")?;
    match payload {
        BatchRequest::Status => Ok(
            json!({"queue":c.read_caption_queue()?.map(QueueView::from),"draftCount":c.list_caption_drafts(0,0)?.total,"active":rt.active.load(Ordering::SeqCst)}),
        ),
        BatchRequest::Preview { ids } => Ok(json!(c.queue_preview(ids)?)),
        BatchRequest::Drafts { offset, limit } => Ok(json!(c.list_caption_drafts(offset, limit)?)),
        BatchRequest::GetDraft { item_id } => Ok(draft_view(
            c,
            c.load_caption_draft(&item_id)?.ok_or("Draft not found")?,
        )),
        BatchRequest::KeepDraft {
            item_id,
            draft_revision,
            caption,
        } => {
            let d = c.keep_caption_draft(&item_id, draft_revision, caption)?;
            Ok(draft_view(c, d))
        }
        BatchRequest::DiscardDraft {
            item_id,
            draft_revision,
        } => {
            c.discard_caption_draft(&item_id, Some(draft_revision))?;
            Ok(Value::Null)
        }
        BatchRequest::SaveAllDrafts => Ok(json!(c.accept_all_caption_drafts()?)),
        BatchRequest::Start { ids, prompt } => {
            if prompt.len() > 16_000 {
                return Err("Caption prompt is too long".into());
            }
            if c.read_caption_queue()?.is_some_and(|q| {
                matches!(
                    q.state,
                    QueueState::Paused
                        | QueueState::Running
                        | QueueState::Pausing
                        | QueueState::Cancelling
                ) && q.counts().pending + q.counts().processing > 0
            }) {
                return Err("Resume or cancel the unfinished run before starting another".into());
            }
            let (account, model) = identity.unwrap();
            let q = CaptionQueue {
                schema_version: 1,
                id: uuid::Uuid::new_v4().to_string(),
                scope: if ids.is_some() { "selected" } else { "all" }.into(),
                prompt,
                account_id: account,
                model: model.ok_or("Choose a model in Agent settings")?,
                created_at: timestamp(),
                state: QueueState::Running,
                last_error: None,
                entries: c.caption_entries(ids)?,
            };
            if q.counts().pending == 0 {
                return Err("No eligible items. Existing captions and drafts are skipped.".into());
            }
            c.write_caption_queue(&q)?;
            launch(&app, &rt, c.root.clone(), q.id.clone())?;
            Ok(json!(QueueView::from(q)))
        }
        BatchRequest::Pause | BatchRequest::Cancel => {
            let cancel = matches!(payload, BatchRequest::Cancel);
            let mut q = c.read_caption_queue()?.ok_or("No caption run exists")?;
            if rt.active.load(Ordering::SeqCst) {
                rt.control
                    .store(if cancel { 2 } else { 1 }, Ordering::SeqCst);
                q.state = if cancel {
                    QueueState::Cancelling
                } else {
                    QueueState::Pausing
                };
            } else {
                q.state = if cancel {
                    QueueState::Cancelled
                } else {
                    QueueState::Paused
                };
                if cancel {
                    for e in &mut q.entries {
                        if e.state == WorkState::Pending {
                            e.state = WorkState::Cancelled
                        }
                    }
                }
            }
            c.write_caption_queue(&q)?;
            Ok(json!(QueueView::from(q)))
        }
        BatchRequest::Resume | BatchRequest::RetryFailed => {
            let retry = matches!(payload, BatchRequest::RetryFailed);
            let mut q = c.read_caption_queue()?.ok_or("No caption run exists")?;
            if identity.as_ref().unwrap().0 != q.account_id {
                return Err("Select this run's ChatGPT account before resuming".into());
            }
            if retry {
                if q.counts().pending > 0 {
                    return Err("Resume the paused run before retrying failed items".into());
                }
                for e in &mut q.entries {
                    if e.state == WorkState::Failed {
                        e.state = WorkState::Pending;
                        e.error = None
                    }
                }
            }
            if q.counts().pending == 0 {
                return Err("No pending items to process".into());
            }
            q.state = QueueState::Running;
            q.last_error = None;
            c.write_caption_queue(&q)?;
            launch(&app, &rt, c.root.clone(), q.id.clone())?;
            Ok(json!(QueueView::from(q)))
        }
    }
}
fn launch(
    app: &tauri::AppHandle,
    rt: &Runtime,
    root: PathBuf,
    job_id: String,
) -> Result<(), String> {
    if rt.active.swap(true, Ordering::SeqCst) {
        return Err("A generation worker is already running".into());
    }
    rt.control.store(0, Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || {
        let outcome = worker(&app, &root, &job_id);
        if let Err(message) = outcome {
            let state = app.state::<State>();
            if let Ok(guard) = state.collection.lock() {
                if let Some(c) = guard.as_ref().filter(|c| c.root == root) {
                    if let Ok(Some(mut q)) = c.read_caption_queue() {
                        if q.id == job_id {
                            q.state = QueueState::Paused;
                            q.last_error = Some(message);
                            for e in &mut q.entries {
                                if e.state == WorkState::Processing {
                                    e.state = WorkState::Pending
                                }
                            }
                            let _ = c.write_caption_queue(&q);
                        }
                    }
                }
            };
        }
        app.state::<Runtime>().active.store(false, Ordering::SeqCst);
        let _ = app.emit("caption-queue-updated", ());
    });
    Ok(())
}
struct Work {
    item_id: String,
    prompt: String,
    account_id: String,
    model: String,
}
fn take_work(c: &Collection, job_id: &str, control: u8) -> Result<Option<Work>, String> {
    let mut q = c.read_caption_queue()?.ok_or("Caption run disappeared")?;
    if q.id != job_id {
        return Err("Caption run changed".into());
    }
    if control != 0 {
        q.state = if control == 2 {
            QueueState::Cancelled
        } else {
            QueueState::Paused
        };
        if control == 2 {
            for e in &mut q.entries {
                if e.state == WorkState::Pending {
                    e.state = WorkState::Cancelled
                }
            }
        }
        c.write_caption_queue(&q)?;
        return Ok(None);
    }
    if q.state != QueueState::Running {
        return Ok(None);
    }
    let mut selected = None;
    for e in &mut q.entries {
        if e.state != WorkState::Pending {
            continue;
        }
        match c.load(&e.item_id) {
            Ok(i) if !i.caption.trim().is_empty() => {
                e.state = WorkState::SkippedCaption;
                continue;
            }
            Err(error) => {
                e.state = WorkState::Failed;
                e.error = Some(error);
                continue;
            }
            _ => {}
        }
        if let Some(d) = c.load_caption_draft(&e.item_id)? {
            e.state = if d.job_id.as_deref() == Some(job_id) {
                WorkState::Done
            } else {
                WorkState::SkippedDraft
            };
            continue;
        }
        e.state = WorkState::Processing;
        selected = Some(e.item_id.clone());
        break;
    }
    if selected.is_none() {
        q.state = QueueState::Completed;
        c.write_caption_queue(&q)?;
        return Ok(None);
    }
    c.write_caption_queue(&q)?;
    Ok(Some(Work {
        item_id: selected.unwrap(),
        prompt: q.prompt,
        account_id: q.account_id,
        model: q.model,
    }))
}
fn finish_work(
    c: &Collection,
    job_id: &str,
    item_id: &str,
    result: Result<CaptionDraftRecord, String>,
    control: u8,
) -> Result<bool, String> {
    let mut q = c.read_caption_queue()?.ok_or("Caption run disappeared")?;
    if q.id != job_id {
        return Err("Caption run changed".into());
    }
    let entry = q
        .entries
        .iter_mut()
        .find(|e| e.item_id == item_id)
        .ok_or("Queue entry disappeared")?;
    match result {
        Ok(d) => {
            if d.item_id != item_id {
                return Err("Draft belongs to another item".into());
            }
            if c.load_caption_draft(item_id)?.is_some() {
                entry.state = WorkState::SkippedDraft
            } else {
                c.write_caption_draft(&d, None)?;
                entry.state = WorkState::Done
            }
            entry.error = None;
        }
        Err(error) => {
            if pause_for_error(&error) {
                entry.state = WorkState::Pending;
                entry.error = Some(error.clone());
                q.state = QueueState::Paused;
                q.last_error = Some(error);
                if control == 2 {
                    q.state = QueueState::Cancelled;
                    for e in &mut q.entries {
                        if e.state == WorkState::Pending {
                            e.state = WorkState::Cancelled
                        }
                    }
                }
                c.write_caption_queue(&q)?;
                return Ok(false);
            }
            entry.state = WorkState::Failed;
            entry.error = Some(error);
        }
    }
    c.write_caption_queue(&q)?;
    Ok(true)
}
fn worker(app: &tauri::AppHandle, root: &PathBuf, job_id: &str) -> Result<(), String> {
    loop {
        let rt = app.state::<Runtime>();
        let work = {
            let state = app.state::<State>();
            let guard = state
                .collection
                .lock()
                .map_err(|_| "Collection lock failed")?;
            let c = guard
                .as_ref()
                .filter(|c| &c.root == root)
                .ok_or("Collection changed; run paused")?;
            take_work(c, job_id, rt.control.load(Ordering::SeqCst))?
        };
        let Some(work) = work else { return Ok(()) };
        let _ = app.emit("caption-queue-updated", ());
        let result = agent::generate_for_batch(
            app,
            root,
            &work.item_id,
            &work.prompt,
            &work.account_id,
            &work.model,
            job_id,
        );
        let proceed = {
            let state = app.state::<State>();
            let guard = state
                .collection
                .lock()
                .map_err(|_| "Collection lock failed")?;
            let c = guard
                .as_ref()
                .filter(|c| &c.root == root)
                .ok_or("Collection changed; run paused")?;
            finish_work(
                c,
                job_id,
                &work.item_id,
                result,
                rt.control.load(Ordering::SeqCst),
            )?
        };
        let _ = app.emit("caption-queue-updated", ());
        if !proceed {
            return Ok(());
        }
    }
}
pub fn timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn pause_for_error(error: &str) -> bool {
    [
        "HTTP 401",
        "HTTP 403",
        "HTTP 429",
        "HTTP 503",
        "HTTP 500",
        "HTTP 502",
        "HTTP 504",
        "subscription_sharing_",
        "model_not_found",
        "model_not_supported",
        "usage_limit",
        "Sign in",
        "sign in",
        "session expired",
        "Connect a ChatGPT",
        "Choose a model",
        "account used by this run",
        "could not connect",
        "Cannot renew",
        "plan access",
    ]
    .iter()
    .any(|s| error.contains(s))
}
#[tauri::command]
pub async fn batch_request(app: tauri::AppHandle, payload: BatchRequest) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || perform(app, payload))
        .await
        .map_err(|_| "Caption queue worker failed".to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Collection, CaptionQueue) {
        let temp = tempfile::tempdir().unwrap();
        let (c, _) = Collection::open(&temp.path().join("collection"), true).unwrap();
        for _ in 0..3 {
            c.import_image(
                None,
                "test.png",
                &nabor_core::encode_clipboard(&[1, 2, 3, 255], 1, 1).unwrap(),
                None,
            )
            .unwrap();
        }
        let q = CaptionQueue {
            schema_version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            scope: "all".into(),
            prompt: "Frozen prompt".into(),
            account_id: "test-account".into(),
            model: "test-model".into(),
            created_at: 1,
            state: QueueState::Running,
            last_error: None,
            entries: c.caption_entries(None).unwrap(),
        };
        c.write_caption_queue(&q).unwrap();
        (temp, c, q)
    }
    fn output(c: &Collection, q: &CaptionQueue, w: &Work) -> CaptionDraftRecord {
        let i = c.load(&w.item_id).unwrap();
        CaptionDraftRecord {
            schema_version: 1,
            item_id: i.id,
            title: i.title,
            expected_hash: i.source_hash,
            caption: "Generated".into(),
            model: w.model.clone(),
            prompt: w.prompt.clone(),
            account_id: w.account_id.clone(),
            job_id: Some(q.id.clone()),
            created_at: 1,
            draft_revision: 1,
        }
    }
    #[test]
    fn worker_pause_resume_and_cancel_keep_completed_drafts_without_repeat() {
        let (_temp, c, q) = fixture();
        let first = take_work(&c, &q.id, 0).unwrap().unwrap();
        assert_eq!(first.prompt, "Frozen prompt");
        finish_work(&c, &q.id, &first.item_id, Ok(output(&c, &q, &first)), 0).unwrap();
        assert!(take_work(&c, &q.id, 1).unwrap().is_none());
        let mut paused = c.read_caption_queue().unwrap().unwrap();
        assert_eq!(paused.state, QueueState::Paused);
        assert_eq!(paused.counts().done, 1);
        paused.state = QueueState::Running;
        c.write_caption_queue(&paused).unwrap();
        let second = take_work(&c, &q.id, 0).unwrap().unwrap();
        assert_ne!(first.item_id, second.item_id);
        finish_work(&c, &q.id, &second.item_id, Ok(output(&c, &q, &second)), 2).unwrap();
        assert!(take_work(&c, &q.id, 2).unwrap().is_none());
        let stopped = c.read_caption_queue().unwrap().unwrap();
        assert_eq!(stopped.state, QueueState::Cancelled);
        assert_eq!(stopped.counts().done, 2);
        assert_eq!(stopped.counts().cancelled, 1);
        assert_eq!(c.list_caption_drafts(0, 100).unwrap().total, 2);
    }
    #[test]
    fn worker_stops_on_usage_limit_and_continues_past_item_failures() {
        let (_temp, c, q) = fixture();
        let first = take_work(&c, &q.id, 0).unwrap().unwrap();
        assert!(!finish_work(
            &c,
            &q.id,
            &first.item_id,
            Err("subscription_sharing_usage_limit_exceeded".into()),
            0
        )
        .unwrap());
        assert!(take_work(&c, &q.id, 0).unwrap().is_none());
        let mut paused = c.read_caption_queue().unwrap().unwrap();
        assert_eq!(paused.counts().pending, 3);
        paused.state = QueueState::Running;
        c.write_caption_queue(&paused).unwrap();
        let first_again = take_work(&c, &q.id, 0).unwrap().unwrap();
        assert_eq!(first.item_id, first_again.item_id);
        assert!(finish_work(&c, &q.id, &first.item_id, Err("Broken image".into()), 0).unwrap());
        let next = take_work(&c, &q.id, 0).unwrap().unwrap();
        assert_ne!(next.item_id, first.item_id);
        assert_eq!(c.read_caption_queue().unwrap().unwrap().counts().failed, 1);
    }
    #[test]
    fn usage_and_auth_pause_instead_of_looping() {
        assert!(pause_for_error("subscription_sharing_usage_limit_exceeded"));
        assert!(pause_for_error("Request rejected HTTP 403"));
        assert!(!pause_for_error("Image is corrupt"));
    }
}
