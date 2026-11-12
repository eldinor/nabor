use crate::State;
use nabor_agent::Agent;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs::File;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{Emitter, Manager};

pub struct AgentState {
    pub client: Mutex<Option<Agent>>,
    pub cancel: Arc<AtomicBool>,
    session_lock: Mutex<Option<File>>,
}
impl Default for AgentState {
    fn default() -> Self {
        Self {
            client: Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            session_lock: Mutex::new(None),
        }
    }
}
pub(crate) fn initialize(
    app: &tauri::AppHandle,
    state: &AgentState,
    client: &mut Option<Agent>,
) -> Result<(), String> {
    if client.is_some() {
        return Ok(());
    }
    use fs2::FileExt;
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Cannot locate agent settings directory")?;
    std::fs::create_dir_all(&dir).map_err(|_| "Cannot create agent settings directory")?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("agent-session.lock"))
        .map_err(|_| "Cannot lock the ChatGPT connection")?;
    lock.try_lock_exclusive().map_err(|_| {
        "The ChatGPT connection is active in another Nabor window. Close that window first."
    })?;
    let next = Agent::new()?;
    *state
        .session_lock
        .lock()
        .map_err(|_| "Agent session lock failed")? = Some(lock);
    *client = Some(next);
    Ok(())
}
#[derive(Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AgentRequest {
    Status,
    SignIn {
        account_id: Option<String>,
    },
    CancelSignIn,
    Models,
    SelectModel {
        model_id: String,
    },
    Usage,
    LaunchIntent,
    SelectAccount {
        account_id: String,
    },
    Disconnect,
    Generate {
        item_id: String,
    },
    Draft {
        item_id: String,
    },
    Revise {
        item_id: String,
        expected_hash: String,
        current_caption: String,
        instructions: String,
        #[serde(default)]
        draft_revision: Option<u64>,
    },
    Apply {
        item_id: String,
        expected_hash: String,
        caption: String,
        #[serde(default)]
        replace_existing: bool,
        #[serde(default)]
        draft_revision: Option<u64>,
    },
}

fn perform(app: tauri::AppHandle, payload: AgentRequest) -> Result<Value, String> {
    let state = app.state::<AgentState>();
    if matches!(payload, AgentRequest::LaunchIntent) {
        return Ok(json!({"openSettings":std::env::args().any(|a|a=="--agent-settings")}));
    }
    if matches!(payload, AgentRequest::Usage) {
        open::that("https://chatgpt.com/settings/usage")
            .map_err(|_| "Cannot open ChatGPT Usage settings")?;
        return Ok(Value::Null);
    }
    if matches!(payload, AgentRequest::CancelSignIn) {
        state.cancel.store(true, Ordering::Relaxed);
        return Ok(Value::Null);
    }
    if let AgentRequest::Apply {
        item_id,
        expected_hash,
        caption,
        replace_existing,
        draft_revision,
    } = payload
    {
        let collection = app.state::<State>();
        let guard = collection
            .collection
            .lock()
            .map_err(|_| "Collection lock failed")?;
        let c = guard.as_ref().ok_or("Open a collection first")?;
        if let Some(revision) = draft_revision {
            return Ok(json!(c.accept_caption_draft(&item_id, revision, caption)?));
        }
        if replace_existing {
            return Ok(json!(c.apply_caption_revision(
                &item_id,
                &expected_hash,
                caption
            )?));
        }
        return Ok(json!(c.apply_generated_caption(
            &item_id,
            &expected_hash,
            caption
        )?));
    }
    if let AgentRequest::Draft { item_id } = payload {
        let collection = app.state::<State>();
        let guard = collection
            .collection
            .lock()
            .map_err(|_| "Collection lock failed")?;
        let c = guard.as_ref().ok_or("Open a collection first")?;
        let item = c.load(&item_id)?;
        if item.caption.trim().is_empty() {
            if let Some(d) = c.load_caption_draft(&item_id)? {
                return Ok(crate::batch::draft_view(c, d));
            }
        }
        return Ok(
            json!({"itemId":item.id,"expectedHash":item.source_hash,"caption":item.caption,"model":null,"replaceExisting":true,"history":item.caption_history}),
        );
    }
    if matches!(
        &payload,
        AgentRequest::Generate { .. } | AgentRequest::Revise { .. }
    ) {
        let (item_id, revision, draft_revision) = match payload {
            AgentRequest::Generate { item_id } => (item_id, None, None),
            AgentRequest::Revise {
                item_id,
                expected_hash,
                current_caption,
                instructions,
                draft_revision,
            } => {
                if instructions.trim().is_empty()
                    || instructions.len() > 8000
                    || current_caption.len() > 32_000
                {
                    return Err("Enter revision instructions (up to 8,000 bytes) and a caption up to 32,000 bytes.".into());
                }
                (
                    item_id,
                    Some((expected_hash, current_caption, instructions)),
                    draft_revision,
                )
            }
            _ => unreachable!(),
        };
        // Resolve only this item while holding its collection lock, then release
        // it before any network operation. Do not send source filesystem paths.
        let (snapshot, context, images, root, common, previous_draft) = {
            let collection = app.state::<State>();
            let guard = collection
                .collection
                .lock()
                .map_err(|_| "Collection lock failed")?;
            let c = guard.as_ref().ok_or("Open a collection first")?;
            let item = c.load(&item_id)?;
            let previous_draft = c.load_caption_draft(&item_id)?;
            if revision.is_none() {
                if let Some(d) = previous_draft {
                    return Ok(crate::batch::draft_view(c, d));
                }
            }
            if crate::batch_active(&app) {
                return Err(
                    "Pause batch generation before generating or revising an individual caption"
                        .into(),
                );
            }
            if item.caption.trim().is_empty()
                && previous_draft.as_ref().map(|d| d.draft_revision) != draft_revision
            {
                return Err("This draft changed. Reopen it before revising.".into());
            }
            if revision.is_none() && !item.caption.trim().is_empty() {
                return Err(
                    "This item already has a caption. Existing captions are preserved.".into(),
                );
            }
            if revision
                .as_ref()
                .is_some_and(|(expected, _, _)| expected != &item.source_hash)
            {
                return Err(
                    "This item changed. Reopen its caption before requesting a revision.".into(),
                );
            }
            if item.assets.is_empty() {
                return Err("Attach an image or reference before generating a caption".into());
            }
            if item.assets.len() > 20 {
                return Err("This first integration supports up to 20 images per item".into());
            }
            if !c.check(&[item_id])?.is_empty() {
                return Err("Locate missing files before generating a caption".into());
            }
            let mut images = vec![];
            for (n, asset) in item.assets.iter().enumerate() {
                let role = match asset.role {
                    nabor_core::Role::Original => "original image",
                    nabor_core::Role::CaptionSource => "caption source reference",
                    nabor_core::Role::Reference => "reference",
                };
                let bytes = nabor_core::agent_image(&c.resolve(&item, asset)?)?;
                images.push((format!("Image {}: {role}", n + 1), bytes));
            }
            let context = caption_context(
                &item.title,
                &item.notes,
                revision
                    .as_ref()
                    .map(|(_, caption, instructions)| (caption.as_str(), instructions.as_str())),
                &c.settings.caption_instructions,
            );
            (
                item,
                context,
                images,
                c.root.clone(),
                c.settings.caption_instructions.clone(),
                previous_draft,
            )
        };
        let mut guard = state.client.lock().map_err(|_| "Agent lock failed")?;
        initialize(&app, &state, &mut guard)?;
        let client = guard.as_mut().unwrap();
        let caption = client.generate(&context, &images)?;
        let status = client.status();
        drop(guard);
        if snapshot.caption.trim().is_empty() {
            let state = app.state::<State>();
            let guard = state
                .collection
                .lock()
                .map_err(|_| "Collection lock failed")?;
            let c = guard
                .as_ref()
                .filter(|c| c.root == root)
                .ok_or("Collection changed before the caption draft could be saved")?;
            let d = nabor_core::CaptionDraftRecord {
                schema_version: 1,
                item_id: snapshot.id.clone(),
                title: snapshot.title.clone(),
                expected_hash: snapshot.source_hash.clone(),
                caption,
                model: status.model.unwrap_or_default(),
                prompt: common,
                account_id: status.active.unwrap_or_default(),
                job_id: previous_draft.as_ref().and_then(|d| d.job_id.clone()),
                created_at: crate::batch::timestamp(),
                draft_revision: previous_draft
                    .as_ref()
                    .map(|d| d.draft_revision + 1)
                    .unwrap_or(1),
            };
            c.write_caption_draft(&d, previous_draft.map(|d| d.draft_revision))?;
            return Ok(crate::batch::draft_view(c, d));
        }
        return Ok(
            json!({"itemId":snapshot.id,"expectedHash":snapshot.source_hash,"caption":caption,"model":status.model,"replaceExisting":!snapshot.caption.trim().is_empty(),"history":snapshot.caption_history}),
        );
    }
    let mut guard = state.client.lock().map_err(|_| "Agent lock failed")?;
    if crate::batch_active(&app)
        && matches!(
            &payload,
            AgentRequest::SignIn { .. }
                | AgentRequest::SelectAccount { .. }
                | AgentRequest::SelectModel { .. }
                | AgentRequest::Disconnect
        )
    {
        return Err("Pause batch generation before changing agent settings".into());
    }
    initialize(&app, &state, &mut guard)?;
    let client = guard.as_mut().unwrap();
    match payload {
        AgentRequest::Status => Ok(json!(client.status())),
        AgentRequest::SignIn { account_id } => {
            state.cancel.store(false, Ordering::Relaxed);
            let pending = client.begin(account_id)?;
            let url = match pending.authorization_url() {
                Ok(url) => url,
                Err(e) => {
                    let _ = client.finish(pending, Err(e.clone()));
                    return Err(e);
                }
            };
            if open::that(&url).is_err() {
                let message =
                    "Could not open the sign-in browser. Check your default browser and try again."
                        .to_string();
                let _ = client.finish(pending, Err(message.clone()));
                return Err(message);
            }
            let status = client.status();
            let cancel = state.cancel.clone();
            let handle = app.clone();
            std::thread::spawn(move || {
                let result = pending.wait(&cancel);
                let state = handle.state::<AgentState>();
                if let Ok(mut guard) = state.client.lock() {
                    if let Some(client) = guard.as_mut() {
                        let _ = client.finish(pending, result);
                        let _ = handle.emit("agent-status", client.status());
                    }
                };
            });
            Ok(json!(status))
        }
        AgentRequest::Models => Ok(json!(client.models()?)),
        AgentRequest::SelectModel { model_id } => {
            client.set_model(model_id)?;
            Ok(json!(client.status()))
        }
        AgentRequest::SelectAccount { account_id } => {
            client.select_account(account_id)?;
            Ok(json!(client.status()))
        }
        AgentRequest::Disconnect => {
            Ok(json!({"message":client.disconnect()?,"status":client.status()}))
        }
        _ => unreachable!(),
    }
}

fn caption_context(
    title: &str,
    notes: &str,
    revision: Option<(&str, &str)>,
    common: &str,
) -> String {
    let mut context=format!("Collection caption instructions:\n{common}\n\nItem title: {title}\nItem-specific instructions and notes (take priority over collection instructions):\n{notes}\nPreserve the source language unless the user instructions specify another language.");
    if let Some((caption, instructions)) = revision {
        context.push_str(&format!("\n\nCurrent draft (including the user's manual edits):\n{caption}\n\nLatest revision instructions:\n{instructions}\n\nRevise this current draft using the supplied references. Preserve existing details and manual edits unless the latest instructions ask to change them. The latest revision instructions take priority over earlier caption instructions."));
    } else {
        context.push_str("\nGenerate the empty final caption using the supplied item material.");
    }
    context
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revision_context_uses_current_draft_and_latest_prompt() {
        let c = caption_context(
            "Title",
            "Earlier instruction",
            Some(("User edited draft", "Make it shorter")),
            "Shared prompt",
        );
        assert!(c.contains("User edited draft"));
        assert!(c.contains("Latest revision instructions:\nMake it shorter"));
        assert!(c.contains("Earlier instruction"));
        assert!(!c.contains("Generate the empty final caption"));
        assert!(c.contains("Shared prompt"));
        assert!(c.contains("take priority over collection instructions"));
    }
}

pub(crate) fn generate_for_batch(
    app: &tauri::AppHandle,
    root: &std::path::Path,
    item_id: &str,
    prompt: &str,
    account_id: &str,
    model: &str,
    job_id: &str,
) -> Result<nabor_core::CaptionDraftRecord, String> {
    let (item, context, images) = {
        let state = app.state::<State>();
        let guard = state
            .collection
            .lock()
            .map_err(|_| "Collection lock failed")?;
        let c = guard
            .as_ref()
            .filter(|c| c.root == root)
            .ok_or("Collection changed")?;
        let item = c.load(item_id)?;
        if !item.caption.trim().is_empty() {
            return Err("Item now has a caption".into());
        }
        if item.assets.is_empty()
            || item.assets.len() > 20
            || !c.check(&[item_id.into()])?.is_empty()
        {
            return Err("Item needs 1–20 available images".into());
        }
        let mut images = vec![];
        for (n, a) in item.assets.iter().enumerate() {
            images.push((
                format!("Image {}: {:?}", n + 1, a.role),
                nabor_core::agent_image(&c.resolve(&item, a)?)?,
            ));
        }
        let context = caption_context(&item.title, &item.notes, None, prompt);
        (item, context, images)
    };
    let state = app.state::<AgentState>();
    let mut guard = state.client.lock().map_err(|_| "Agent lock failed")?;
    initialize(app, &state, &mut guard)?;
    let caption = guard
        .as_mut()
        .unwrap()
        .generate_for(&context, &images, account_id, model)?;
    Ok(nabor_core::CaptionDraftRecord {
        schema_version: 1,
        item_id: item.id,
        title: item.title,
        expected_hash: item.source_hash,
        caption,
        model: model.into(),
        prompt: prompt.into(),
        account_id: account_id.into(),
        job_id: Some(job_id.into()),
        created_at: crate::batch::timestamp(),
        draft_revision: 1,
    })
}

#[tauri::command]
pub async fn agent_request(app: tauri::AppHandle, payload: AgentRequest) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || perform(app, payload))
        .await
        .map_err(|_| "Agent worker failed".to_string())?
}
