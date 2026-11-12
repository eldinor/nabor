use nabor_core::*;
use std::fs;
fn setup() -> (tempfile::TempDir, Collection) {
    let temp = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&temp.path().join("collection"), true).unwrap();
    (temp, c)
}
fn item(c: &Collection) -> Item {
    c.import_image(
        None,
        "image.png",
        &encode_clipboard(&[1, 2, 3, 255], 1, 1).unwrap(),
        None,
    )
    .unwrap()
}
fn draft(i: &Item, job: Option<String>) -> CaptionDraftRecord {
    CaptionDraftRecord {
        schema_version: 1,
        item_id: i.id.clone(),
        title: i.title.clone(),
        expected_hash: i.source_hash.clone(),
        caption: "Generated draft".into(),
        model: "test-model".into(),
        prompt: "Common instructions".into(),
        account_id: "test-account".into(),
        job_id: job,
        created_at: 1,
        draft_revision: 1,
    }
}
#[test]
fn all_scope_ignores_pagination_and_selected_scope_deduplicates() {
    let (_temp, mut c) = setup();
    for _ in 0..125 {
        item(&c);
    }
    c.set_caption_instructions("Common instructions".into())
        .unwrap();
    assert_eq!(c.list("", "all", 0, 100).unwrap().items.len(), 100);
    let all = c.queue_preview(None).unwrap();
    assert_eq!(all.counts.total, 125);
    assert_eq!(all.counts.pending, 125);
    assert_eq!(all.prompt, "Common instructions");
    let ids = c.collection_item_ids().unwrap();
    assert_eq!(
        c.queue_preview(Some(vec![ids[0].clone(), ids[0].clone(), ids[124].clone()]))
            .unwrap()
            .counts
            .total,
        2
    );
}
#[test]
fn preview_skips_captions_drafts_and_unavailable_sources() {
    let (_temp, c) = setup();
    let mut a = item(&c);
    let b = item(&c);
    let d = c.create_item().unwrap();
    a.caption = "User caption".into();
    c.update(&mut a).unwrap();
    c.write_caption_draft(&draft(&b, None), None).unwrap();
    let p = c.queue_preview(None).unwrap();
    assert_eq!(p.counts.skipped_caption, 1);
    assert_eq!(p.counts.skipped_draft, 1);
    assert_eq!(p.counts.failed, 1);
    assert_eq!(p.counts.pending, 0);
    assert!(c.load(&d.id).unwrap().caption.is_empty());
}
#[test]
fn persistent_drafts_have_separate_revisions_and_do_not_change_final_captions() {
    let (_temp, c) = setup();
    let i = item(&c);
    let d = draft(&i, None);
    c.write_caption_draft(&d, None).unwrap();
    assert!(c.load(&i.id).unwrap().caption.is_empty());
    assert!(c.write_caption_draft(&d, None).is_err());
    let edited = c
        .keep_caption_draft(&i.id, 1, "Manually edited draft".into())
        .unwrap();
    assert_eq!(edited.draft_revision, 2);
    assert!(c.keep_caption_draft(&i.id, 1, "Stale edit".into()).is_err());
    let root = c.root.clone();
    drop(c);
    let (c, _) = Collection::open(&root, false).unwrap();
    assert_eq!(
        c.load_caption_draft(&i.id).unwrap().unwrap().caption,
        "Manually edited draft"
    );
    let saved = c
        .accept_caption_draft(&i.id, 2, "Reviewed final".into())
        .unwrap();
    assert_eq!(saved.caption, "Reviewed final");
    assert!(c.load_caption_draft(&i.id).unwrap().is_none());
}
#[test]
fn save_all_keeps_source_conflicts_and_never_overwrites_existing_captions() {
    let (_temp, c) = setup();
    let a = item(&c);
    let mut b = item(&c);
    c.write_caption_draft(&draft(&a, None), None).unwrap();
    c.write_caption_draft(&draft(&b, None), None).unwrap();
    b.caption = "User wrote this meanwhile".into();
    c.update(&mut b).unwrap();
    let p = c.list_caption_drafts(0, 100).unwrap();
    assert!(p.items.iter().find(|d| d.item_id == b.id).unwrap().stale);
    let report = c.accept_all_caption_drafts().unwrap();
    assert_eq!(report.saved, 1);
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(c.load(&b.id).unwrap().caption, "User wrote this meanwhile");
    assert!(c.load_caption_draft(&b.id).unwrap().is_some());
}
#[test]
fn interrupted_run_retains_committed_drafts_and_requires_explicit_resume() {
    let (_temp, mut c) = setup();
    let a = item(&c);
    let b = item(&c);
    let mut q = CaptionQueue {
        schema_version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        scope: "all".into(),
        prompt: "Frozen run instructions".into(),
        account_id: "test-account".into(),
        model: "test-model".into(),
        created_at: 1,
        state: QueueState::Running,
        last_error: None,
        entries: c.caption_entries(None).unwrap(),
    };
    for e in &mut q.entries {
        if e.item_id == a.id {
            e.state = WorkState::Processing
        }
    }
    c.write_caption_queue(&q).unwrap();
    c.write_caption_draft(&draft(&a, Some(q.id.clone())), None)
        .unwrap();
    c.set_caption_instructions("Changed later".into()).unwrap();
    let root = c.root.clone();
    drop(c);
    let (c, _) = Collection::open(&root, false).unwrap();
    let q = c.read_caption_queue().unwrap().unwrap();
    assert_eq!(q.state, QueueState::Paused);
    assert_eq!(q.prompt, "Frozen run instructions");
    assert_eq!(q.model, "test-model");
    assert_eq!(
        q.entries.iter().find(|e| e.item_id == a.id).unwrap().state,
        WorkState::Done
    );
    assert_eq!(
        q.entries.iter().find(|e| e.item_id == b.id).unwrap().state,
        WorkState::Pending
    );
    assert_eq!(c.queue_preview(None).unwrap().counts.skipped_draft, 1);
    assert_eq!(c.settings.caption_instructions, "Changed later");
}
#[test]
fn draft_index_rebuild_and_export_do_not_publish_unreviewed_output() {
    let (temp, mut c) = setup();
    let i = item(&c);
    let d = draft(&i, None);
    c.write_caption_draft(&d, None).unwrap();
    c.refresh().unwrap();
    assert_eq!(c.list_caption_drafts(0, 1).unwrap().total, 1);
    let exported = c.export(&[i.id.clone()], temp.path()).unwrap();
    assert!(!exported.join("captions").exists());
    let final_item: Item = serde_json::from_slice(
        &fs::read(exported.join("items").join(&i.id).join("item.json")).unwrap(),
    )
    .unwrap();
    assert!(final_item.caption.is_empty());
    c.discard_caption_draft(&i.id, Some(1)).unwrap();
    assert_eq!(c.list_caption_drafts(0, 1).unwrap().total, 0);
}
