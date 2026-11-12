use nabor_core::*;
use std::{fs, path::Path, time::Instant};

fn image(path: &Path) {
    image::RgbImage::from_pixel(48, 32, image::Rgb([42, 93, 61]))
        .save(path)
        .unwrap();
}

#[test]
fn generated_caption_requires_unchanged_empty_item() {
    let (_temp, c) = collection();
    let mut item = c.create_item().unwrap();
    let old_hash = item.source_hash.clone();
    item.notes = "Updated user instruction".into();
    c.update(&mut item).unwrap();
    assert!(c
        .apply_generated_caption(&item.id, &old_hash, "Stale result".into())
        .is_err());
    assert!(c.load(&item.id).unwrap().caption.is_empty());
    let saved = c
        .apply_generated_caption(&item.id, &item.source_hash, "Reviewed result".into())
        .unwrap();
    assert_eq!(saved.caption, "Reviewed result");
    assert!(c
        .apply_generated_caption(&saved.id, &saved.source_hash, "Overwrite".into())
        .is_err());
    assert_eq!(c.load(&saved.id).unwrap().caption, "Reviewed result");
}

#[test]
fn web_import_preserves_bytes_url_and_automatic_original_roles() {
    let (temp, c) = collection();
    let png = encode_clipboard(&[1, 2, 3, 255], 1, 1).unwrap();
    let source = "https://example.com/image.png?original=1";
    let first = c
        .import_image(None, "web.png", &png, Some(source.into()))
        .unwrap();
    assert_eq!(first.assets[0].role, Role::Original);
    assert_eq!(first.assets[0].storage, Storage::Copy);
    assert_eq!(first.assets[0].source_url.as_deref(), Some(source));
    assert_eq!(
        fs::read(c.resolve(&first, &first.assets[0]).unwrap()).unwrap(),
        png
    );
    let blank = c.create_item().unwrap();
    let filled = c
        .import_image(Some(&blank.id), "first.png", &png, Some(source.into()))
        .unwrap();
    assert_eq!(filled.assets[0].role, Role::Original);
    let filled = c
        .import_image(Some(&blank.id), "second.png", &png, Some(source.into()))
        .unwrap();
    assert_eq!(filled.assets[1].role, Role::Reference);
    c.move_asset(&blank.id, &first.id, &filled.assets[1].id)
        .unwrap();
    assert_eq!(
        c.load(&first.id).unwrap().assets[1].source_url.as_deref(),
        Some(source)
    );
    let exported = c.export(&[first.id.clone()], temp.path()).unwrap();
    let data: Item = serde_json::from_slice(
        &fs::read(exported.join("items").join(&first.id).join("item.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(data.assets[0].source_url.as_deref(), Some(source));
    let root = c.root.clone();
    drop(c);
    let (c, _) = Collection::open(&root, false).unwrap();
    assert_eq!(
        c.load(&first.id).unwrap().assets[0].source_url.as_deref(),
        Some(source)
    );
    assert!(c
        .import_image(
            None,
            "not.png",
            b"<html>not an image</html>",
            Some(source.into())
        )
        .is_err());
    assert!(c
        .import_image(None, "bad.png", &png, Some("javascript:alert(1)".into()))
        .is_err());
}

#[test]
fn revised_caption_history_survives_reopen_and_rejects_stale_replacements() {
    let (_temp, c) = collection();
    let mut item = c.create_item().unwrap();
    item.caption = "First caption".into();
    c.update(&mut item).unwrap();
    let stale = item.source_hash.clone();
    let revised = c
        .apply_caption_revision(&item.id, &item.source_hash, "Reviewed replacement".into())
        .unwrap();
    assert_eq!(revised.caption_history.len(), 1);
    assert_eq!(revised.caption_history[0].caption, "First caption");
    assert!(c
        .apply_caption_revision(&item.id, &stale, "Stale overwrite".into())
        .is_err());
    let root = c.root.clone();
    drop(c);
    let (c, _) = Collection::open(&root, false).unwrap();
    let item = c.load(&item.id).unwrap();
    assert_eq!(item.caption, "Reviewed replacement");
    assert_eq!(item.caption_history[0].caption, "First caption");
    let restored = c
        .apply_caption_revision(
            &item.id,
            &item.source_hash,
            item.caption_history[0].caption.clone(),
        )
        .unwrap();
    assert_eq!(restored.caption, "First caption");
    assert_eq!(
        restored.caption_history.last().unwrap().caption,
        "Reviewed replacement"
    );
}

#[test]
fn history_is_bounded_and_legacy_manifests_need_no_migration() {
    let (_temp, c) = collection();
    let mut item = c.create_item().unwrap();
    let file = c.root.join("items").join(&item.id).join("item.json");
    let old = fs::read(&file).unwrap();
    assert!(!String::from_utf8(old).unwrap().contains("captionHistory"));
    for n in 0..25 {
        item.caption = format!("Caption {n}");
        c.update(&mut item).unwrap();
    }
    assert_eq!(item.caption_history.len(), 20);
    assert_eq!(item.caption_history[0].caption, "Caption 4");
    let before = item.caption_history.len();
    item.notes = "Metadata change".into();
    c.update(&mut item).unwrap();
    assert_eq!(item.caption_history.len(), before);
}

#[test]
fn agent_image_is_bounded_and_preserves_original_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("large.png");
    image::RgbaImage::from_pixel(2000, 1000, image::Rgba([0, 0, 0, 0]))
        .save(&path)
        .unwrap();
    let original = fs::read(&path).unwrap();
    let prepared = agent_image(&path).unwrap();
    let decoded = image::load_from_memory(&prepared).unwrap();
    assert_eq!(decoded.width(), 1600);
    assert_eq!(decoded.height(), 800);
    assert!(decoded.to_rgb8().get_pixel(0, 0)[0] > 245);
    assert_eq!(fs::read(path).unwrap(), original);
}
fn collection() -> (tempfile::TempDir, Collection) {
    let temp = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&temp.path().join("collection"), true).unwrap();
    (temp, c)
}
fn import(c: &Collection, path: &Path, storage: Storage) -> Item {
    c.import(None, &[path.to_string_lossy().into()], storage)
        .unwrap()
        .remove(0)
}

#[test]
fn copied_and_linked_round_trip_and_rebuild() {
    let (temp, mut c) = collection();
    let source = temp.path().join("оригинал with spaces.png");
    image(&source);
    let mut a = import(&c, &source, Storage::Copy);
    let b = import(&c, &source, Storage::Link);
    a.caption = "A green original".into();
    a.notes = "Make a listing".into();
    c.update(&mut a).unwrap();
    c.set_default(Storage::Link).unwrap();
    let root = c.root.clone();
    drop(c);
    fs::remove_file(root.join(".nabor/index.sqlite")).unwrap();
    let (mut c, report) = Collection::open(&root, false).unwrap();
    assert_eq!(report.indexed, 2);
    assert_eq!(c.settings.default_storage, Storage::Link);
    assert_eq!(c.load(&a.id).unwrap().caption, "A green original");
    assert_eq!(c.load(&b.id).unwrap().assets[0].storage, Storage::Link);
    assert!(c.resolve(&a, &a.assets[0]).unwrap().is_file());
    assert_eq!(c.refresh().unwrap().indexed, 2);
    assert_eq!(c.list("listing", "all", 0, 100).unwrap().total, 1);
    assert_eq!(c.list("оригинал", "all", 0, 100).unwrap().total, 2);
}

#[test]
fn twenty_pairs_role_replacement_and_move_preserve_relationships() {
    let (temp, c) = collection();
    let path = temp.path().join("original.png");
    image(&path);
    let screenshot = temp.path().join("caption.png");
    image(&screenshot);
    let mut ids = vec![];
    for n in 0..20 {
        let mut item = import(&c, &path, Storage::Copy);
        item.caption = format!("Caption {n}");
        c.update(&mut item).unwrap();
        item = c
            .import(
                Some(&item.id),
                &[screenshot.to_string_lossy().into()],
                Storage::Copy,
            )
            .unwrap()
            .remove(0);
        item = c
            .set_role(&item.id, &item.assets[1].id, Role::CaptionSource)
            .unwrap();
        ids.push(item.id);
    }
    let first = c.load(&ids[0]).unwrap();
    c.move_asset(&ids[0], &ids[1], &first.assets[1].id).unwrap();
    assert_eq!(c.load(&ids[0]).unwrap().assets.len(), 1);
    let dest = c.load(&ids[1]).unwrap();
    assert_eq!(dest.assets.len(), 3);
    assert_eq!(dest.assets[2].filename, "caption.png");
    assert_eq!(dest.assets[2].role, Role::Reference);
    let updated = c
        .set_role(&dest.id, &dest.assets[2].id, Role::Original)
        .unwrap();
    assert_eq!(
        updated
            .assets
            .iter()
            .filter(|a| a.role == Role::Original)
            .count(),
        1
    );
    let root = c.root.clone();
    drop(c);
    let (c, _) = Collection::open(&root, false).unwrap();
    for (n, item_id) in ids.iter().enumerate() {
        assert_eq!(c.load(item_id).unwrap().caption, format!("Caption {n}"));
    }
}

#[test]
fn broken_links_cache_reconnect_and_delete_never_delete_sources() {
    let (temp, c) = collection();
    let path = temp.path().join("source.png");
    image(&path);
    let item = import(&c, &path, Storage::Link);
    let cached = thumbnail(&c.root, &item, &item.assets[0]).unwrap().unwrap();
    assert!(cached.exists());
    let moved = temp.path().join("moved.png");
    fs::rename(&path, &moved).unwrap();
    assert_eq!(c.check(&[item.id.clone()]).unwrap().len(), 1);
    assert_eq!(c.list("", "missing-files", 0, 100).unwrap().total, 1);
    assert_eq!(
        thumbnail(&c.root, &item, &item.assets[0]).unwrap().unwrap(),
        cached
    );
    let item = c.reconnect(&item.id, &item.assets[0].id, &moved).unwrap();
    assert!(c.check(&[item.id.clone()]).unwrap().is_empty());
    c.detach(&item.id, &item.assets[0].id).unwrap();
    assert!(moved.exists());
    let item = import(&c, &moved, Storage::Link);
    c.remove(&[item.id]).unwrap();
    assert!(moved.exists());
    assert_eq!(
        fs::read_dir(c.root.join(".nabor/trash")).unwrap().count(),
        1
    );
}

#[test]
fn export_is_self_contained_byte_exact_and_relative() {
    let (temp, c) = collection();
    let path = temp.path().join("original.png");
    image(&path);
    let mut item = import(&c, &path, Storage::Link);
    item.caption = "Caption belongs here".into();
    item.notes = "Create a card".into();
    c.update(&mut item).unwrap();
    item.caption = "Final caption belongs here".into();
    c.update(&mut item).unwrap();
    assert_eq!(item.caption_history.len(), 1);
    let extra = c
        .import(
            Some(&item.id),
            &[path.to_string_lossy().into()],
            Storage::Copy,
        )
        .unwrap()
        .remove(0);
    let dest = c.export(&[item.id.clone()], temp.path()).unwrap();
    let exported: Item = serde_json::from_slice(
        &fs::read(dest.join("items").join(&item.id).join("item.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(exported.assets.len(), extra.assets.len());
    assert_eq!(exported.caption, item.caption);
    assert!(exported.caption_history.is_empty());
    assert_eq!(exported.notes, item.notes);
    for a in &exported.assets {
        assert_eq!(a.storage, Storage::Copy);
        assert!(!Path::new(&a.path).is_absolute());
        assert_eq!(
            fs::read(dest.join("items").join(&item.id).join(&a.path)).unwrap(),
            fs::read(&path).unwrap()
        );
    }
    assert!(dest.join("package.json").exists());
    assert!(dest.join("README.md").exists());
    assert!(exported.source_hash.is_empty());
    fs::remove_file(&path).unwrap();
    assert!(c.export(&[item.id], temp.path()).is_err());
    assert!(!fs::read_dir(temp.path()).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".nabor-export-")));
}

#[test]
fn external_changes_are_detected_without_revision_bump() {
    let (_temp, mut c) = collection();
    let mut item = c.create_item().unwrap();
    let file = c.root.join("items").join(&item.id).join("item.json");
    let mut external = item.clone();
    external.title = "External edit".into();
    fs::write(&file, serde_json::to_vec(&external).unwrap()).unwrap();
    item.caption = "My pending edit".into();
    assert!(c
        .update(&mut item)
        .unwrap_err()
        .contains("changed externally"));
    c.refresh().unwrap();
    assert_eq!(c.load(&item.id).unwrap().title, "External edit");
}

#[test]
fn invalid_and_future_manifests_are_preserved() {
    let (_temp, mut c) = collection();
    let item = c.create_item().unwrap();
    let file = c.root.join("items").join(&item.id).join("item.json");
    let mut future = item.clone();
    future.schema_version = 900;
    let bytes = serde_json::to_vec(&future).unwrap();
    fs::write(&file, &bytes).unwrap();
    let report = c.refresh().unwrap();
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(fs::read(&file).unwrap(), bytes);
    assert_eq!(c.list("", "all", 0, 10).unwrap().total, 0);
    fs::write(&file, b"broken json").unwrap();
    assert_eq!(c.refresh().unwrap().warnings.len(), 1);
    assert_eq!(fs::read(file).unwrap(), b"broken json");
}

#[test]
fn failed_atomic_write_keeps_previous_data() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("item.json");
    atomic_write(&file, b"old").unwrap();
    atomic_write(&file, b"new").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"new");
    // Persisting over a directory must fail and cannot damage the existing manifest.
    let dir = temp.path().join("directory");
    fs::create_dir(&dir).unwrap();
    assert!(atomic_write(&dir, b"bad").is_err());
    assert_eq!(fs::read(&file).unwrap(), b"new");
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
}

#[test]
fn traversal_and_multiple_originals_are_rejected() {
    let (temp, mut c) = collection();
    let path = temp.path().join("source.png");
    image(&path);
    let mut item = import(&c, &path, Storage::Copy);
    item.assets[0].path = "../../outside.png".into();
    assert!(c.update(&mut item).is_err());
    assert!(c.resolve(&item, &item.assets[0]).is_err());
    assert!(c.load("../outside").is_err());
    let mut item = c.load(&item.id).unwrap();
    let mut a = item.assets[0].clone();
    a.id = uuid::Uuid::new_v4().to_string();
    item.assets.push(a);
    let file = c.root.join("items").join(&item.id).join("item.json");
    fs::write(&file, serde_json::to_vec(&item).unwrap()).unwrap();
    assert_eq!(c.refresh().unwrap().warnings.len(), 1);
}

#[test]
fn clipboard_is_managed_and_portable_after_collection_move() {
    let (temp, c) = collection();
    let png = encode_clipboard(&[20, 30, 40, 255], 1, 1).unwrap();
    let item = c.clipboard(None, &png).unwrap();
    assert_eq!(item.assets[0].storage, Storage::Copy);
    assert_eq!(item.assets[0].role, Role::Original);
    let old = c.root.clone();
    drop(c);
    let moved = temp.path().join("moved collection");
    fs::rename(old, &moved).unwrap();
    let (c, _) = Collection::open(&moved, false).unwrap();
    let item = c.load(&item.id).unwrap();
    assert!(c.resolve(&item, &item.assets[0]).unwrap().exists());
}

#[test]
fn collection_lock_and_corrupt_cache_recovery() {
    let (_temp, c) = collection();
    assert!(Collection::open(&c.root, false).is_err());
    let root = c.root.clone();
    drop(c);
    fs::write(root.join(".nabor/index.sqlite"), b"corrupt").unwrap();
    let (c, report) = Collection::open(&root, false).unwrap();
    assert_eq!(report.indexed, 0);
    assert_eq!(c.list("", "all", 0, 100).unwrap().total, 0);
}

#[test]
fn ten_thousand_items_search_and_page_under_one_second() {
    let (_temp, mut c) = collection();
    let start = Instant::now();
    for n in 0..10_000 {
        let mut item = Item::empty();
        item.title = format!("Image {n:05}");
        item.caption = format!("Caption for image {n:05}");
        if n == 7_777 {
            item.notes = "needle performance search".into();
        }
        let folder = c.root.join("items").join(&item.id);
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("item.json"), serde_json::to_vec(&item).unwrap()).unwrap();
    }
    assert_eq!(c.refresh().unwrap().indexed, 10_000);
    let indexed = start.elapsed();
    let begin = Instant::now();
    assert_eq!(c.list("", "all", 0, 100).unwrap().items.len(), 100);
    let page = begin.elapsed();
    let begin = Instant::now();
    assert_eq!(c.list("needle", "all", 0, 100).unwrap().total, 1);
    let search = begin.elapsed();
    println!(
        "10,000 manifests: seed+rebuild={indexed:?}, first page={page:?}, search={search:?}; {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    assert!(page.as_secs_f64() < 1.0);
    assert!(search.as_secs_f64() < 1.0);
}
