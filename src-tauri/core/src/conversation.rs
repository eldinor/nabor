use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurn {
    pub id: String,
    pub question: String,
    pub answer: Option<String>,
    pub error: Option<String>,
    pub model: String,
    pub source_hash: String,
    pub created_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub schema_version: u32,
    pub item_id: String,
    pub revision: u64,
    pub turns: Vec<ConversationTurn>,
}
impl Collection {
    pub fn conversation(&self, item_id: &str) -> Result<Conversation> {
        self.load(item_id)?;
        let path = self.item_dir(item_id)?.join("conversation.json");
        if !path.exists() {
            return Ok(Conversation {
                schema_version: 1,
                item_id: item_id.into(),
                revision: 0,
                turns: vec![],
            });
        }
        no_symlink(&path)?;
        if fs::metadata(&path).map_err(err)?.len() > 2_000_000 {
            return Err("Conversation file is too large; file left untouched".into());
        }
        let c: Conversation = serde_json::from_slice(&fs::read(path).map_err(err)?)
            .map_err(|_| "Invalid conversation; file left untouched")?;
        if c.schema_version != 1 || c.item_id != item_id || c.turns.len() > 100 {
            return Err("Unsupported or invalid conversation; file left untouched".into());
        }
        for t in &c.turns {
            valid_id(&t.id)?;
            if t.question.len() > 8_000 || t.answer.as_ref().is_some_and(|a| a.len() > 32_000) {
                return Err("Invalid conversation turn; file left untouched".into());
            }
        }
        Ok(c)
    }
    fn write_conversation(&self, c: &Conversation) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(c).map_err(err)?;
        if bytes.len() > 2_000_000 {
            return Err("Conversation is full. Clear it before sending another question.".into());
        }
        atomic_write(
            &self.item_dir(&c.item_id)?.join("conversation.json"),
            &bytes,
        )
    }
    pub fn begin_question(
        &self,
        item_id: &str,
        revision: u64,
        question: String,
        retry: Option<&str>,
        model: String,
    ) -> Result<Conversation> {
        if question.trim().is_empty() || question.len() > 8_000 {
            return Err("Enter a question up to 8,000 bytes".into());
        }
        let item = self.load(item_id)?;
        let mut c = self.conversation(item_id)?;
        if c.revision != revision {
            return Err("Conversation changed. Reopen it before sending.".into());
        }
        if let Some(turn_id) = retry {
            let t = c
                .turns
                .last_mut()
                .filter(|t| t.id == turn_id && t.answer.is_none())
                .ok_or("Only the latest unanswered question can be retried")?;
            if t.question != question {
                return Err("Retry must use the original question".into());
            }
            t.error = None;
            t.source_hash = item.source_hash;
            t.model = model;
        } else {
            if c.turns.len() >= 100 {
                return Err(
                    "Conversation is full. Clear it before sending another question.".into(),
                );
            }
            c.turns.push(ConversationTurn {
                id: id(),
                question,
                answer: None,
                error: None,
                model,
                source_hash: item.source_hash,
                created_at: now(),
            });
        }
        c.revision += 1;
        // Leave space for a fully escaped reply before making a paid request.
        if serde_json::to_vec_pretty(&c).map_err(err)?.len() > 1_790_000 {
            return Err("Conversation is full. Clear it before sending another question.".into());
        }
        self.write_conversation(&c)?;
        Ok(c)
    }
    pub fn finish_question(
        &self,
        item_id: &str,
        revision: u64,
        turn_id: &str,
        result: Result<String>,
    ) -> Result<Conversation> {
        let mut c = self.conversation(item_id)?;
        if c.revision != revision {
            return Err("Conversation changed before the reply could be saved".into());
        }
        let t = c
            .turns
            .last_mut()
            .filter(|t| t.id == turn_id && t.answer.is_none())
            .ok_or("Question changed before the reply could be saved")?;
        match result {
            Ok(text) if !text.trim().is_empty() && text.len() <= 32_000 => {
                t.answer = Some(text);
                t.error = None;
            }
            Ok(_) => {
                t.error = Some("Reply was empty or exceeded 32,000 bytes".into());
            }
            Err(error) => {
                t.error = Some(error.chars().take(2000).collect());
            }
        }
        c.revision += 1;
        self.write_conversation(&c)?;
        Ok(c)
    }
    pub fn clear_conversation(&self, item_id: &str, revision: u64) -> Result<Conversation> {
        let mut c = self.conversation(item_id)?;
        if c.revision != revision {
            return Err("Conversation changed. Reopen it before clearing.".into());
        }
        c.turns.clear();
        c.revision += 1;
        self.write_conversation(&c)?;
        Ok(c)
    }
    pub fn conversation_reply(&self, item_id: &str, turn_id: &str) -> Result<String> {
        self.conversation(item_id)?
            .turns
            .into_iter()
            .find(|t| t.id == turn_id)
            .and_then(|t| t.answer)
            .ok_or("Reply not found".into())
    }
    pub fn reply_to_notes(
        &self,
        item_id: &str,
        turn_id: &str,
        expected_hash: &str,
    ) -> Result<Item> {
        let reply = self.conversation_reply(item_id, turn_id)?;
        let mut item = self.load(item_id)?;
        if item.source_hash != expected_hash {
            return Err("Item changed. Reopen the conversation before adding notes.".into());
        }
        if !item.notes.trim().is_empty() {
            item.notes.push_str("\n\n");
        }
        item.notes.push_str(&reply);
        self.update(&mut item)?;
        Ok(item)
    }
}
