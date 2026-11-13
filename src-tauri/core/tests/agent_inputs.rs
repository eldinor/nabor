use nabor_core::*;
use std::fs;

#[test]
fn attachment_only_requests_exclude_original_bytes_and_ignore_missing_originals() {
    let temp = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&temp.path().join("collection"), true).unwrap();
    let i = c
        .import_image(
            None,
            "main.png",
            &encode_clipboard(&[255, 0, 0, 255], 1, 1).unwrap(),
            None,
        )
        .unwrap();
    let i = c
        .import_image(
            Some(&i.id),
            "evidence.png",
            &encode_clipboard(&[0, 0, 255, 255], 1, 1).unwrap(),
            None,
        )
        .unwrap();
    let all = c.agent_images(&i, false, true).unwrap();
    let references = c.agent_images(&i, true, true).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(references.len(), 1);
    assert_eq!(references[0].1, all[1].1);
    assert_ne!(references[0].1, all[0].1);
    fs::remove_file(c.resolve(&i, &i.assets[0]).unwrap()).unwrap();
    assert_eq!(c.agent_images(&i, true, true).unwrap(), references);
    assert!(c.agent_images(&i, false, true).is_err());
    assert_eq!(
        c.queue_preview_for_agent(None, true)
            .unwrap()
            .counts
            .pending,
        1
    );
    assert_eq!(c.queue_preview(None).unwrap().counts.failed, 1);
    fs::remove_file(c.resolve(&i, &i.assets[1]).unwrap()).unwrap();
    assert!(c.agent_images(&i, true, true).is_err());
    assert_eq!(
        c.queue_preview_for_agent(None, true).unwrap().counts.failed,
        1
    );
}

#[test]
fn caption_requires_included_material_but_questions_can_use_text_and_roles_remain_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&temp.path().join("collection"), true).unwrap();
    let i = c
        .import_image(
            None,
            "main.png",
            &encode_clipboard(&[2, 4, 6, 255], 1, 1).unwrap(),
            None,
        )
        .unwrap();
    assert!(c
        .agent_images(&i, true, true)
        .unwrap_err()
        .contains("Send only attachments"));
    assert!(c.agent_images(&i, true, false).unwrap().is_empty());
    assert_eq!(
        c.queue_preview_for_agent(None, true).unwrap().counts.failed,
        1
    );
    let i = c
        .set_role(&i.id, &i.assets[0].id, Role::CaptionSource)
        .unwrap();
    assert_eq!(c.agent_images(&i, true, true).unwrap().len(), 1);
    assert!(c.agent_images(&i, true, true).unwrap()[0]
        .0
        .contains("CaptionSource"));
    assert_eq!(
        c.queue_preview_for_agent(None, true)
            .unwrap()
            .counts
            .pending,
        1
    );
}

#[test]
fn image_limit_counts_only_images_being_sent() {
    let temp = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&temp.path().join("collection"), true).unwrap();
    let png = encode_clipboard(&[2, 4, 6, 255], 1, 1).unwrap();
    let mut i = c.import_image(None, "main.png", &png, None).unwrap();
    for _ in 0..20 {
        i = c
            .import_image(Some(&i.id), "reference.png", &png, None)
            .unwrap();
    }
    assert_eq!(c.agent_assets(&i, true, true).unwrap().len(), 20);
    assert!(c.agent_assets(&i, false, true).is_err());
    i = c
        .import_image(Some(&i.id), "too-many.png", &png, None)
        .unwrap();
    assert!(c.agent_assets(&i, true, true).is_err());
}
