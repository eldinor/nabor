use nabor_core::*;
use std::fs;

#[test]
fn discussion_is_durable_separate_portable_and_not_exported() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("collection");
    let (c, _) = Collection::open(&root, true).unwrap();
    let item = c.create_item().unwrap();
    let original_hash = item.source_hash.clone();
    let started = c
        .begin_question(
            &item.id,
            0,
            "What is missing?".into(),
            None,
            "test-model".into(),
        )
        .unwrap();
    let done = c
        .finish_question(
            &item.id,
            started.revision,
            &started.turns[0].id,
            Ok("Add a reference image.".into()),
        )
        .unwrap();
    assert_eq!(c.load(&item.id).unwrap().source_hash, original_hash);
    assert_eq!(
        done.turns[0].answer.as_deref(),
        Some("Add a reference image.")
    );
    drop(c);
    let moved = temp.path().join("moved collection");
    fs::rename(root, &moved).unwrap();
    let (c, _) = Collection::open(&moved, false).unwrap();
    assert_eq!(
        c.conversation(&item.id).unwrap().turns[0].question,
        "What is missing?"
    );
    let export = c.export(&[item.id.clone()], temp.path()).unwrap();
    assert!(!export
        .join("items")
        .join(&item.id)
        .join("conversation.json")
        .exists());
    assert!(c.load(&item.id).unwrap().caption.is_empty());
    let other = c.create_item().unwrap();
    assert!(c.conversation(&other.id).unwrap().turns.is_empty());
    assert!(c.conversation_reply(&other.id, &done.turns[0].id).is_err());
}

#[test]
fn interrupted_and_failed_questions_retry_explicitly_without_duplicate_turns() {
    let temp = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&temp.path().join("c"), true).unwrap();
    let item = c.create_item().unwrap();
    let q = c
        .begin_question(&item.id, 0, "Question".into(), None, "m1".into())
        .unwrap();
    let root = c.root.clone();
    drop(c);
    let (c, _) = Collection::open(&root, false).unwrap();
    let interrupted = c.conversation(&item.id).unwrap();
    assert!(interrupted.turns[0].answer.is_none());
    let retry = c
        .begin_question(
            &item.id,
            interrupted.revision,
            "Question".into(),
            Some(&q.turns[0].id),
            "m2".into(),
        )
        .unwrap();
    assert_eq!(retry.turns.len(), 1);
    assert_eq!(retry.turns[0].model, "m2");
    let failed = c
        .finish_question(
            &item.id,
            retry.revision,
            &q.turns[0].id,
            Err("Usage limit".into()),
        )
        .unwrap();
    assert_eq!(failed.turns[0].error.as_deref(), Some("Usage limit"));
    assert!(c
        .begin_question(&item.id, q.revision, "Old state".into(), None, "m".into())
        .is_err());
    let retry = c
        .begin_question(
            &item.id,
            failed.revision,
            "Question".into(),
            Some(&q.turns[0].id),
            "m2".into(),
        )
        .unwrap();
    let completed = c
        .finish_question(
            &item.id,
            retry.revision,
            &q.turns[0].id,
            Ok("Answer".into()),
        )
        .unwrap();
    assert!(completed.turns[0].error.is_none());
    assert!(c
        .begin_question(
            &item.id,
            completed.revision,
            "Question".into(),
            Some(&q.turns[0].id),
            "m".into()
        )
        .is_err());
}

#[test]
fn notes_append_requires_explicit_current_item_and_conversation_clear_is_guarded() {
    let temp = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&temp.path().join("c"), true).unwrap();
    let mut item = c.create_item().unwrap();
    item.notes = "User notes".into();
    item.caption = "User caption".into();
    c.update(&mut item).unwrap();
    let q = c
        .begin_question(&item.id, 0, "Explain".into(), None, "m".into())
        .unwrap();
    let done = c
        .finish_question(
            &item.id,
            q.revision,
            &q.turns[0].id,
            Ok("Agent reply".into()),
        )
        .unwrap();
    assert!(c
        .reply_to_notes(&item.id, &q.turns[0].id, "stale-hash")
        .is_err());
    let updated = c
        .reply_to_notes(&item.id, &q.turns[0].id, &item.source_hash)
        .unwrap();
    assert_eq!(updated.notes, "User notes\n\nAgent reply");
    assert_eq!(updated.caption, "User caption");
    assert!(c.clear_conversation(&item.id, q.revision).is_err());
    let cleared = c.clear_conversation(&item.id, done.revision).unwrap();
    assert!(cleared.turns.is_empty());
    assert_eq!(c.load(&item.id).unwrap().caption, "User caption");
}

#[test]
fn invalid_conversation_and_invalid_input_are_preserved_without_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&temp.path().join("c"), true).unwrap();
    let item = c.create_item().unwrap();
    assert!(c
        .begin_question(&item.id, 0, " ".into(), None, "m".into())
        .is_err());
    assert!(c
        .begin_question(&item.id, 0, "a".repeat(8001), None, "m".into())
        .is_err());
    let path = c
        .root
        .join("items")
        .join(&item.id)
        .join("conversation.json");
    let invalid = br#"{"schemaVersion":99,"itemId":"wrong","revision":0,"turns":[]}"#;
    fs::write(&path, invalid).unwrap();
    assert!(c.conversation(&item.id).is_err());
    assert!(c.clear_conversation(&item.id, 0).is_err());
    assert_eq!(fs::read(path).unwrap(), invalid);
}
