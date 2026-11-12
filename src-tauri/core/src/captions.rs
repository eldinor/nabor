use super::*;
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionDraftRecord {
    pub schema_version: u32,
    pub item_id: String,
    pub title: String,
    pub expected_hash: String,
    pub caption: String,
    pub model: String,
    pub prompt: String,
    pub account_id: String,
    pub job_id: Option<String>,
    pub created_at: u64,
    pub draft_revision: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftSummary {
    pub item_id: String,
    pub title: String,
    pub caption: String,
    pub model: String,
    pub stale: bool,
    pub created_at: u64,
    pub draft_revision: u64,
}
#[derive(Serialize)]
pub struct DraftPage {
    pub items: Vec<DraftSummary>,
    pub total: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum QueueState {
    Running,
    Pausing,
    Paused,
    Cancelling,
    Cancelled,
    Completed,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum WorkState {
    Pending,
    Processing,
    Done,
    SkippedCaption,
    SkippedDraft,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    pub item_id: String,
    pub title: String,
    pub state: WorkState,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionQueue {
    pub schema_version: u32,
    pub id: String,
    pub scope: String,
    pub prompt: String,
    pub account_id: String,
    pub model: String,
    pub created_at: u64,
    pub state: QueueState,
    pub last_error: Option<String>,
    pub entries: Vec<QueueEntry>,
}
#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueCounts {
    pub total: usize,
    pub pending: usize,
    pub processing: usize,
    pub done: usize,
    pub skipped_caption: usize,
    pub skipped_draft: usize,
    pub failed: usize,
    pub cancelled: usize,
}
impl CaptionQueue {
    pub fn counts(&self) -> QueueCounts {
        let mut c = QueueCounts {
            total: self.entries.len(),
            ..Default::default()
        };
        for e in &self.entries {
            match e.state {
                WorkState::Pending => c.pending += 1,
                WorkState::Processing => c.processing += 1,
                WorkState::Done => c.done += 1,
                WorkState::SkippedCaption => c.skipped_caption += 1,
                WorkState::SkippedDraft => c.skipped_draft += 1,
                WorkState::Failed => c.failed += 1,
                WorkState::Cancelled => c.cancelled += 1,
            }
        }
        c
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueView {
    pub id: String,
    pub scope: String,
    pub prompt: String,
    pub model: String,
    pub state: QueueState,
    pub last_error: Option<String>,
    pub counts: QueueCounts,
    pub entries: Vec<QueueEntry>,
}
impl From<CaptionQueue> for QueueView {
    fn from(q: CaptionQueue) -> Self {
        let counts = q.counts();
        Self {
            id: q.id,
            scope: q.scope,
            prompt: q.prompt,
            model: q.model,
            state: q.state,
            last_error: q.last_error,
            counts,
            entries: q
                .entries
                .into_iter()
                .filter(|e| e.state == WorkState::Failed || e.state == WorkState::Processing)
                .take(100)
                .collect(),
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuePreview {
    pub scope: String,
    pub prompt: String,
    pub counts: QueueCounts,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDraftReport {
    pub saved: usize,
    pub conflicts: Vec<String>,
}

impl Collection {
    pub(crate) fn init_captions(&self) -> Result<()> {
        for relative in ["captions", "captions/drafts"] {
            let p = self.root.join(relative);
            fs::create_dir_all(&p).map_err(err)?;
            no_symlink(&p)?;
        }
        self.db.execute_batch("CREATE TABLE IF NOT EXISTS caption_drafts (item_id TEXT PRIMARY KEY,title TEXT,caption TEXT,model TEXT,created_at INTEGER,draft_revision INTEGER);").map_err(err)?;
        Ok(())
    }
    pub fn set_caption_instructions(&mut self, prompt: String) -> Result<()> {
        if prompt.len() > 16_000 {
            return Err("Collection caption instructions exceed 16,000 bytes".into());
        }
        let mut next = self.settings.clone();
        next.caption_instructions = prompt;
        atomic_write(
            &self.root.join("collection.json"),
            &serde_json::to_vec_pretty(&next).map_err(err)?,
        )?;
        self.settings = next;
        Ok(())
    }
    fn draft_path(&self, item_id: &str) -> Result<PathBuf> {
        valid_id(item_id)?;
        Ok(self
            .root
            .join("captions/drafts")
            .join(format!("{item_id}.json")))
    }
    pub fn load_caption_draft(&self, item_id: &str) -> Result<Option<CaptionDraftRecord>> {
        let p = self.draft_path(item_id)?;
        if !p.exists() {
            return Ok(None);
        }
        no_symlink(&p)?;
        let d: CaptionDraftRecord = serde_json::from_slice(&fs::read(&p).map_err(err)?)
            .map_err(|_| "Invalid caption draft; file left untouched")?;
        if d.schema_version != 1
            || d.item_id != item_id
            || d.caption.len() > 32_000
            || d.expected_hash.is_empty()
        {
            return Err("Unsupported or invalid caption draft; file left untouched".into());
        }
        Ok(Some(d))
    }
    fn index_draft(&self, d: &CaptionDraftRecord) -> Result<()> {
        self.db
            .execute(
                "INSERT OR REPLACE INTO caption_drafts VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    d.item_id,
                    d.title,
                    d.caption,
                    d.model,
                    d.created_at,
                    d.draft_revision
                ],
            )
            .map_err(err)?;
        Ok(())
    }
    pub(crate) fn rebuild_caption_index(&self) -> Result<Vec<String>> {
        self.db
            .execute("DELETE FROM caption_drafts", [])
            .map_err(err)?;
        let mut warnings = vec![];
        for entry in fs::read_dir(self.root.join("captions/drafts")).map_err(err)? {
            let entry = entry.map_err(err)?;
            if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let item_id = entry
                .path()
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            match self.load_caption_draft(&item_id) {
                Ok(Some(d)) => self.index_draft(&d)?,
                Ok(None) => {}
                Err(e) => warnings.push(format!("Caption draft {item_id}: {e}")),
            }
        }
        Ok(warnings)
    }
    pub fn write_caption_draft(
        &self,
        d: &CaptionDraftRecord,
        expected_revision: Option<u64>,
    ) -> Result<()> {
        if d.caption.trim().is_empty() || d.caption.len() > 32_000 {
            return Err("Caption draft is empty or too long".into());
        }
        let current = self.load_caption_draft(&d.item_id)?;
        match (current.as_ref(), expected_revision) {
            (None, None) => {}
            (Some(old), Some(expected)) if old.draft_revision == expected => {}
            _ => {
                return Err(
                    "A caption draft already exists or changed. Review it before replacing it."
                        .into(),
                )
            }
        }
        if d.draft_revision != current.as_ref().map(|d| d.draft_revision + 1).unwrap_or(1) {
            return Err("Invalid draft revision".into());
        }
        atomic_write(
            &self.draft_path(&d.item_id)?,
            &serde_json::to_vec_pretty(d).map_err(err)?,
        )?;
        self.index_draft(d)
    }
    pub fn keep_caption_draft(
        &self,
        item_id: &str,
        expected_revision: u64,
        caption: String,
    ) -> Result<CaptionDraftRecord> {
        let mut d = self
            .load_caption_draft(item_id)?
            .ok_or("Caption draft not found")?;
        if d.draft_revision != expected_revision {
            return Err("This draft changed. Reopen it before editing.".into());
        }
        if d.caption != caption {
            d.caption = caption;
            d.draft_revision += 1;
            self.write_caption_draft(&d, Some(expected_revision))?;
        }
        Ok(d)
    }
    pub fn discard_caption_draft(
        &self,
        item_id: &str,
        expected_revision: Option<u64>,
    ) -> Result<()> {
        if let Some(expected) = expected_revision {
            if self
                .load_caption_draft(item_id)?
                .is_some_and(|d| d.draft_revision != expected)
            {
                return Err("This draft changed. Reopen it before discarding.".into());
            }
        }
        let p = self.draft_path(item_id)?;
        if p.exists() {
            no_symlink(&p)?;
            fs::remove_file(p).map_err(err)?;
        }
        self.db
            .execute("DELETE FROM caption_drafts WHERE item_id=?1", [item_id])
            .map_err(err)?;
        Ok(())
    }
    pub fn list_caption_drafts(&self, offset: usize, limit: usize) -> Result<DraftPage> {
        let total = self
            .db
            .query_row("SELECT COUNT(*) FROM caption_drafts", [], |r| r.get(0))
            .map_err(err)?;
        let mut s=self.db.prepare("SELECT item_id,title,substr(caption,1,240),model,created_at,draft_revision FROM caption_drafts ORDER BY created_at DESC,item_id LIMIT ?1 OFFSET ?2").map_err(err)?;
        let rows = s
            .query_map(params![limit.min(100), offset], |r| {
                Ok(DraftSummary {
                    item_id: r.get(0)?,
                    title: r.get(1)?,
                    caption: r.get(2)?,
                    model: r.get(3)?,
                    created_at: r.get(4)?,
                    draft_revision: r.get(5)?,
                    stale: false,
                })
            })
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        let mut items = vec![];
        for mut summary in rows {
            let d = self
                .load_caption_draft(&summary.item_id)?
                .ok_or("Draft index changed; refresh the collection")?;
            summary.stale = self
                .load(&d.item_id)
                .map(|i| i.source_hash != d.expected_hash || !i.caption.trim().is_empty())
                .unwrap_or(true);
            items.push(summary)
        }
        Ok(DraftPage { items, total })
    }
    pub fn accept_caption_draft(
        &self,
        item_id: &str,
        expected_revision: u64,
        caption: String,
    ) -> Result<Item> {
        let d = self
            .load_caption_draft(item_id)?
            .ok_or("Caption draft not found")?;
        if d.draft_revision != expected_revision {
            return Err("This draft changed. Reopen it before saving.".into());
        }
        let item = self.apply_generated_caption(item_id, &d.expected_hash, caption)?;
        self.discard_caption_draft(item_id, Some(expected_revision))?;
        Ok(item)
    }
    pub fn accept_all_caption_drafts(&self) -> Result<SaveDraftReport> {
        let mut s = self
            .db
            .prepare("SELECT item_id FROM caption_drafts ORDER BY item_id")
            .map_err(err)?;
        let ids = s
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        drop(s);
        let mut report = SaveDraftReport {
            saved: 0,
            conflicts: vec![],
        };
        for id in ids {
            match self.load_caption_draft(&id) {
                Ok(Some(d)) => match self.accept_caption_draft(&id, d.draft_revision, d.caption) {
                    Ok(_) => report.saved += 1,
                    Err(e) => {
                        if report.conflicts.len() < 100 {
                            report.conflicts.push(format!("{}: {e}", d.title));
                        }
                    }
                },
                Err(e) => report.conflicts.push(e),
                _ => {}
            }
        }
        Ok(report)
    }
    pub fn collection_item_ids(&self) -> Result<Vec<String>> {
        let mut s = self
            .db
            .prepare("SELECT id FROM items ORDER BY id")
            .map_err(err)?;
        let ids = s
            .query_map([], |r| r.get(0))
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(ids)
    }
    pub fn caption_entries(&self, selected: Option<Vec<String>>) -> Result<Vec<QueueEntry>> {
        let ids = match selected {
            Some(ids) => ids,
            None => self.collection_item_ids()?,
        };
        let mut seen = HashSet::new();
        let mut entries = vec![];
        for item_id in ids {
            valid_id(&item_id)?;
            if !seen.insert(item_id.clone()) {
                continue;
            }
            let mut e = QueueEntry {
                item_id: item_id.clone(),
                title: item_id.clone(),
                state: WorkState::Pending,
                error: None,
            };
            match self.load(&item_id) {
                Ok(item) => {
                    e.title = item.title.clone();
                    if !item.caption.trim().is_empty() {
                        e.state = WorkState::SkippedCaption
                    } else if self.draft_path(&item_id)?.exists() {
                        e.state = WorkState::SkippedDraft
                    } else if item.assets.is_empty()
                        || item.assets.len() > 20
                        || item
                            .assets
                            .iter()
                            .any(|a| self.resolve(&item, a).map(|p| !p.is_file()).unwrap_or(true))
                    {
                        e.state = WorkState::Failed;
                        e.error = Some("Attach 1–20 available images before generating".into())
                    }
                }
                Err(error) => {
                    e.state = WorkState::Failed;
                    e.error = Some(error)
                }
            }
            entries.push(e);
        }
        Ok(entries)
    }
    pub fn queue_preview(&self, selected: Option<Vec<String>>) -> Result<QueuePreview> {
        let scope = if selected.is_some() {
            "selected"
        } else {
            "all"
        };
        let q = CaptionQueue {
            schema_version: 1,
            id: id(),
            scope: scope.into(),
            prompt: self.settings.caption_instructions.clone(),
            account_id: String::new(),
            model: String::new(),
            created_at: now(),
            state: QueueState::Paused,
            last_error: None,
            entries: self.caption_entries(selected)?,
        };
        Ok(QueuePreview {
            scope: q.scope.clone(),
            prompt: q.prompt.clone(),
            counts: q.counts(),
        })
    }
    pub fn read_caption_queue(&self) -> Result<Option<CaptionQueue>> {
        let p = self.root.join("captions/queue.json");
        if !p.exists() {
            return Ok(None);
        }
        no_symlink(&p)?;
        let q: CaptionQueue = serde_json::from_slice(&fs::read(p).map_err(err)?)
            .map_err(|_| "Caption queue is invalid; file left untouched")?;
        if q.schema_version != 1 {
            return Err("Unsupported caption queue version".into());
        }
        valid_id(&q.id)?;
        Ok(Some(q))
    }
    pub fn write_caption_queue(&self, q: &CaptionQueue) -> Result<()> {
        atomic_write(
            &self.root.join("captions/queue.json"),
            &serde_json::to_vec_pretty(q).map_err(err)?,
        )
    }
    pub fn recover_queue(&self) -> Result<()> {
        if let Some(mut q) = self.read_caption_queue()? {
            if matches!(
                q.state,
                QueueState::Running | QueueState::Pausing | QueueState::Cancelling
            ) {
                for e in &mut q.entries {
                    if e.state == WorkState::Processing {
                        if self
                            .load_caption_draft(&e.item_id)?
                            .is_some_and(|d| d.job_id.as_deref() == Some(&q.id))
                        {
                            e.state = WorkState::Done
                        } else {
                            e.state = WorkState::Pending
                        }
                    }
                }
                q.state = QueueState::Paused;
                q.last_error=Some("Previous run was interrupted. Completed drafts were retained; resume explicitly.".into());
                self.write_caption_queue(&q)?;
            }
        }
        Ok(())
    }
}
