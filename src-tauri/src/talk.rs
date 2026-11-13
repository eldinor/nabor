use crate::{
    agent::{self, AgentState},
    State,
};
use nabor_core::{Collection, Conversation};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::Manager;

#[derive(Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TalkRequest {
    Get {
        item_id: String,
    },
    Ask {
        item_id: String,
        revision: u64,
        question: String,
        retry: Option<String>,
    },
    Clear {
        item_id: String,
        revision: u64,
    },
    UseCaption {
        item_id: String,
        turn_id: String,
        expected_hash: String,
    },
    AddNotes {
        item_id: String,
        turn_id: String,
        expected_hash: String,
    },
}
fn view(c: &Collection, item_id: &str) -> Result<Value, String> {
    let item = c.load(item_id)?;
    Ok(
        json!({"conversation":c.conversation(item_id)?,"itemId":item.id,"title":item.title,"sourceHash":item.source_hash,"assetCount":item.assets.len(),"documentCount":item.documents.len(),"linkCount":item.links.len()}),
    )
}
fn history(c: &Conversation) -> Vec<(String, String)> {
    let mut result = vec![];
    let mut bytes = 0;
    for t in c.turns.iter().rev() {
        if let Some(a) = &t.answer {
            let size = t.question.len() + a.len();
            if result.len() >= 20 || bytes + size > 60_000 {
                break;
            }
            bytes += size;
            result.push((t.question.clone(), a.clone()));
        }
    }
    result.reverse();
    result
}
fn perform(app: tauri::AppHandle, payload: TalkRequest) -> Result<Value, String> {
    let (item_id, revision, question, retry) = match payload {
        TalkRequest::Ask {
            item_id,
            revision,
            question,
            retry,
        } => (item_id, revision, question, retry),
        other => {
            let state = app.state::<State>();
            let guard = state
                .collection
                .lock()
                .map_err(|_| "Collection lock failed")?;
            let c = guard.as_ref().ok_or("Open a collection first")?;
            return match other {
                TalkRequest::Get { item_id } => view(c, &item_id),
                TalkRequest::Clear { item_id, revision } => {
                    c.clear_conversation(&item_id, revision)?;
                    view(c, &item_id)
                }
                TalkRequest::AddNotes {
                    item_id,
                    turn_id,
                    expected_hash,
                } => Ok(json!(c.reply_to_notes(
                    &item_id,
                    &turn_id,
                    &expected_hash
                )?)),
                TalkRequest::UseCaption {
                    item_id,
                    turn_id,
                    expected_hash,
                } => {
                    let item = c.load(&item_id)?;
                    if item.source_hash != expected_hash {
                        return Err(
                            "Item changed. Reopen the conversation before using this reply.".into(),
                        );
                    }
                    Ok(
                        json!({"itemId":item.id,"caption":c.conversation_reply(&item_id,&turn_id)?,"expectedHash":item.source_hash,"model":null,"replaceExisting":!item.caption.trim().is_empty(),"history":item.caption_history}),
                    )
                }
                _ => unreachable!(),
            };
        }
    };
    if crate::batch_active(&app) {
        return Err("Pause batch generation before asking the agent".into());
    }
    let state = app.state::<AgentState>();
    let mut client = state
        .client
        .try_lock()
        .map_err(|_| "Agent is busy. Wait for the current request to finish.")?;
    agent::initialize(&app, &state, &mut client)?;
    let agent = client.as_mut().unwrap();
    let model = agent
        .status()
        .model
        .ok_or("Choose a model in Agent settings")?;
    let (root, conversation, context, images, snapshot) = {
        let state = app.state::<State>();
        let guard = state
            .collection
            .lock()
            .map_err(|_| "Collection lock failed")?;
        let c = guard.as_ref().ok_or("Open a collection first")?;
        // Recheck after obtaining the agent lock so a newly started batch wins.
        if crate::batch_active(&app) {
            return Err("Pause batch generation before asking the agent".into());
        }
        let item = c.load(&item_id)?;
        let only_attachments = agent.status().send_only_attachments;
        let images = c.agent_images(&item, only_attachments, false)?;
        let sources: Vec<_> = item
            .assets
            .iter()
            .filter(|a| !only_attachments || a.role != nabor_core::Role::Original)
            .filter_map(|a| a.source_url.as_ref())
            .collect();
        let context=format!("Current item material:\nTitle: {}\nCaption: {}\nItem instructions and notes:\n{}\nCollection caption instructions (background context):\n{}\nImage source URLs (not fetched):\n{}{}",item.title,item.caption,item.agent_reference_notes(),c.settings.caption_instructions,serde_json::to_string(&sources).unwrap_or_default(),agent::image_scope(only_attachments));
        if context.len() > 200_000 {
            return Err(
                "This item's text exceeds the conversation input limit of 200,000 bytes".into(),
            );
        }
        let conversation = c.begin_question(
            &item_id,
            revision,
            question.clone(),
            retry.as_deref(),
            model,
        )?;
        (c.root.clone(), conversation, context, images, item)
    };
    let answer = crate::documents::context(&app, &root, &snapshot).and_then(|extra| {
        agent.ask(
            &format!("{context}{extra}"),
            &images,
            &history(&conversation),
            &question,
        )
    });
    drop(client);
    let state = app.state::<State>();
    let guard = state
        .collection
        .lock()
        .map_err(|_| "Collection lock failed")?;
    let c = guard
        .as_ref()
        .filter(|c| c.root == root)
        .ok_or("Collection changed before the reply could be saved")?;
    c.finish_question(
        &item_id,
        conversation.revision,
        &conversation.turns.last().unwrap().id,
        answer,
    )?;
    view(c, &item_id)
}
#[tauri::command]
pub async fn talk_request(app: tauri::AppHandle, payload: TalkRequest) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || perform(app, payload))
        .await
        .map_err(|_| "Agent conversation worker failed".to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_chronological_bounded_and_excludes_unanswered_questions() {
        let mut c = Conversation {
            schema_version: 1,
            item_id: "test".into(),
            revision: 1,
            turns: vec![],
        };
        for n in 0..25 {
            c.turns.push(nabor_core::ConversationTurn {
                id: n.to_string(),
                question: format!("q{n}"),
                answer: Some(format!("a{n}")),
                error: None,
                model: "m".into(),
                source_hash: "h".into(),
                created_at: 0,
            });
        }
        c.turns.last_mut().unwrap().answer = None;
        let h = history(&c);
        assert_eq!(h.len(), 20);
        assert_eq!(h.first().unwrap().0, "q4");
        assert_eq!(h.last().unwrap().0, "q23");
        c.turns[23].answer = Some("x".repeat(59_996));
        assert_eq!(history(&c).len(), 1);
    }
}
