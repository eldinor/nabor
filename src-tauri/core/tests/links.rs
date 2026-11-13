use nabor_core::*;
use std::fs;

fn fixture() -> (tempfile::TempDir, Collection, Item) {
    let temp = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&temp.path().join("collection"), true).unwrap();
    let item = c
        .import_image(
            None,
            "original.png",
            &encode_clipboard(&[1, 2, 3, 255], 1, 1).unwrap(),
            None,
        )
        .unwrap();
    (temp, c, item)
}
#[test]
fn website_attachments_survive_edits_rebuild_reopen_and_collection_transfer() {
    let (temp, mut c, item) = fixture();
    let original = fs::read(c.root.join("items").join(&item.id).join("item.json")).unwrap();
    let mut linked = c
        .save_website_link(
            &item.id,
            &item.source_hash,
            None,
            "https://example.com/reference?q=1#detail".into(),
            "Product specifications".into(),
            "Dimensions are listed in the table".into(),
        )
        .unwrap();
    assert_eq!(linked.schema_version, 2);
    assert_eq!(linked.assets.len(), 1);
    let backup = c
        .root
        .join("items")
        .join(&item.id)
        .join(format!("item.v1.{}.backup.json", &item.source_hash[..16]));
    assert_eq!(fs::read(backup).unwrap(), original);
    let id = linked.links[0].id.clone();
    linked.caption = "User caption".into();
    linked.notes = "User instructions".into();
    c.update(&mut linked).unwrap();
    c.refresh().unwrap();
    for query in [
        "example.com",
        "Product specifications",
        "listed in the table",
    ] {
        assert_eq!(c.list(query, "all", 0, 10).unwrap().total, 1);
    }
    assert_eq!(c.list("", "all", 0, 10).unwrap().items[0].asset_count, 2);
    let moved = temp.path().join("moved");
    let root = c.root.clone();
    drop(c);
    fs::rename(root, &moved).unwrap();
    let (c, _) = Collection::open(&moved, false).unwrap();
    let linked = c.load(&item.id).unwrap();
    assert_eq!(linked.links[0].id, id);
    assert_eq!(linked.caption, "User caption");
    assert_eq!(
        c.website_link_url(&item.id, &id).unwrap(),
        linked.links[0].url
    );
    let removed = c
        .remove_website_link(&item.id, &linked.source_hash, &id)
        .unwrap();
    assert!(removed.links.is_empty());
    assert_eq!(removed.schema_version, 2);
    assert_eq!(removed.assets.len(), 1);
    assert!(c.resolve(&removed, &removed.assets[0]).unwrap().is_file());
}
#[test]
fn unsafe_urls_invalid_changes_and_stale_commands_cannot_overwrite_attachments() {
    let (_temp, c, item) = fixture();
    for url in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "data:text/html,test",
        "mailto:test@example.com",
        "ftp://example.com",
        "https://user:secret@example.com",
        "example.com",
        "https://example.com/\nnext",
    ] {
        assert!(
            c.save_website_link(
                &item.id,
                &item.source_hash,
                None,
                url.into(),
                "".into(),
                "".into()
            )
            .is_err(),
            "{url}"
        );
        assert_eq!(c.load(&item.id).unwrap().source_hash, item.source_hash);
    }
    assert_eq!(
        website_url(" http://localhost:3000/reference ").unwrap(),
        "http://localhost:3000/reference"
    );
    assert!(website_url("https://例え.テスト/資料")
        .unwrap()
        .starts_with("https://xn--"));
    let linked = c
        .save_website_link(
            &item.id,
            &item.source_hash,
            None,
            "https://example.com".into(),
            "".into(),
            "".into(),
        )
        .unwrap();
    let id = &linked.links[0].id;
    assert!(c
        .remove_website_link(&item.id, &item.source_hash, id)
        .is_err());
    assert!(c
        .save_website_link(
            &item.id,
            &item.source_hash,
            Some(id),
            "https://other.example".into(),
            "".into(),
            "".into()
        )
        .is_err());
    let mut smuggled = linked.clone();
    smuggled.links.clear();
    assert!(c.update(&mut smuggled).is_err());
    assert!(c
        .apply_generated_caption(
            &item.id,
            &item.source_hash,
            "Stale generated caption".into()
        )
        .is_err());
    assert_eq!(c.load(&item.id).unwrap().links, linked.links);
}
#[test]
fn export_retains_website_relationships_and_image_bytes_with_explicit_version() {
    let (temp, c, item) = fixture();
    let linked = c
        .save_website_link(
            &item.id,
            &item.source_hash,
            None,
            "https://example.com/website".into(),
            "Website title".into(),
            "User supplied evidence".into(),
        )
        .unwrap();
    let export = c.export(&[item.id.clone()], temp.path()).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(export.join("package.json")).unwrap()).unwrap();
    assert_eq!(manifest["schemaVersion"], 2);
    let dir = export.join("items").join(&item.id);
    let exported: Item = serde_json::from_slice(&fs::read(dir.join("item.json")).unwrap()).unwrap();
    assert_eq!(exported.links, linked.links);
    assert_eq!(
        fs::read(dir.join(&exported.assets[0].path)).unwrap(),
        fs::read(c.resolve(&linked, &linked.assets[0]).unwrap()).unwrap()
    );
    assert!(fs::read_to_string(export.join("README.md"))
        .unwrap()
        .contains("Website contents have not been fetched"));
    assert_eq!(fs::read_dir(dir.join("assets")).unwrap().count(), 1);
}
#[test]
fn first_link_backup_failure_keeps_the_legacy_manifest_unchanged() {
    let (_temp, c, item) = fixture();
    let dir = c.root.join("items").join(&item.id);
    let path = dir.join(format!("item.v1.{}.backup.json", &item.source_hash[..16]));
    // A pre-existing directory cannot be treated as a completed backup.
    fs::create_dir(&path).unwrap();
    assert!(c
        .save_website_link(
            &item.id,
            &item.source_hash,
            None,
            "https://example.com".into(),
            "".into(),
            "".into()
        )
        .is_err());
    let after = c.load(&item.id).unwrap();
    assert_eq!(after.schema_version, 1);
    assert_eq!(after.source_hash, item.source_hash);
    assert!(after.links.is_empty());
}
#[test]
fn link_notes_reach_agent_context_and_link_only_items_are_eligible_without_fetching() {
    let (_temp, c, _) = fixture();
    let blank = c.create_item().unwrap();
    let linked = c
        .save_website_link(
            &blank.id,
            &blank.source_hash,
            None,
            "https://example.com/context".into(),
            "Reference".into(),
            "User notes for this URL".into(),
        )
        .unwrap();
    let text = linked.agent_reference_notes();
    assert!(text.contains("https://example.com/context"));
    assert!(text.contains("User notes for this URL"));
    assert!(text.contains("NOT been fetched"));
    assert_eq!(
        c.queue_preview(Some(vec![blank.id.clone()]))
            .unwrap()
            .counts
            .pending,
        1
    );
    assert!(c.check(&[blank.id]).unwrap().is_empty());
    assert!(c
        .save_website_link(
            &linked.id,
            &linked.source_hash,
            None,
            "https://example.com".into(),
            "x".repeat(513),
            "".into()
        )
        .is_err());
    assert!(c
        .save_website_link(
            &linked.id,
            &linked.source_hash,
            None,
            "https://example.com".into(),
            "".into(),
            "x".repeat(8001)
        )
        .is_err());
}

#[test]
fn moving_a_website_attachment_preserves_its_text_and_both_items_images() {
    let (_temp, c, source) = fixture();
    let target = c.create_item().unwrap();
    let linked = c
        .save_website_link(
            &source.id,
            &source.source_hash,
            None,
            "https://example.com/move".into(),
            "Move this link".into(),
            "Evidence notes".into(),
        )
        .unwrap();
    c.move_asset(&source.id, &target.id, &linked.links[0].id)
        .unwrap();
    let moved = c.load(&target.id).unwrap();
    let source = c.load(&source.id).unwrap();
    assert_eq!(moved.schema_version, 2);
    assert_eq!(moved.links[0].url, linked.links[0].url);
    assert_eq!(moved.links[0].notes, "Evidence notes");
    assert!(source.links.is_empty());
    assert_eq!(source.assets.len(), 1);
}
