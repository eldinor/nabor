use nabor_core::*;
use std::{fs, io::Write, path::Path};
fn setup() -> (tempfile::TempDir, Collection) {
    let t = tempfile::tempdir().unwrap();
    let (c, _) = Collection::open(&t.path().join("collection"), true).unwrap();
    (t, c)
}
fn pdf(text: bool) -> Vec<u8> {
    let stream = if text {
        "BT /F1 12 Tf 20 50 Td (PDF reference text) Tj ET"
    } else {
        ""
    };
    let objects=vec!["<< /Type /Catalog /Pages 2 0 R >>".to_string(),"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".into(),"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),format!("<< /Length {} >>\nstream\n{}\nendstream",stream.len(),stream)];
    let mut data = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (n, o) in objects.iter().enumerate() {
        offsets.push(data.len());
        data.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", n + 1, o).as_bytes());
    }
    let xref = data.len();
    data.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
    for off in offsets.iter().skip(1) {
        data.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    data.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            offsets.len(),
            xref
        )
        .as_bytes(),
    );
    data
}
fn docx() -> Vec<u8> {
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
    z.start_file(
        "word/document.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    z.write_all(br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>DOCX &amp; evidence</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Width</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>42 mm</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#).unwrap();
    z.finish().unwrap().into_inner()
}
fn extract(c: &Collection, i: &Item) -> DocumentText {
    prepare_document_text(&c.root, i, &i.documents[0], extract_document_text).unwrap()
}
#[test]
fn all_document_formats_extract_text_and_docx_tables_without_changing_originals() {
    let (t, c) = setup();
    for (name, bytes, expected) in [
        ("text.txt", b"Text reference".to_vec(), "Text reference"),
        (
            "notes.md",
            b"# Markdown evidence\n[link](https://example.com)".to_vec(),
            "# Markdown evidence",
        ),
        ("word.docx", docx(), "42 mm"),
        ("file.pdf", pdf(true), "PDF reference text"),
    ] {
        let i = c
            .import_document(None, name, &bytes, Storage::Copy, None)
            .unwrap();
        assert_eq!(i.schema_version, 3);
        assert!(i.assets.is_empty());
        let text = extract(&c, &i);
        assert_eq!(text.status, "ready", "{name}: {:?}", text.warning);
        assert!(text.text.contains(expected), "{}", text.text);
        if name.ends_with("docx") {
            assert!(text.text.contains("DOCX & evidence"));
        }
        assert_eq!(
            fs::read(document_path(&c.root, &i, &i.documents[0]).unwrap()).unwrap(),
            bytes
        );
        let exp = c.export(&[i.id.clone()], t.path()).unwrap();
        let out: Item = serde_json::from_slice(
            &fs::read(exp.join("items").join(&i.id).join("item.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            fs::read(exp.join("items").join(&i.id).join(&out.documents[0].path)).unwrap(),
            bytes
        );
    }
}
#[test]
fn linked_changes_missing_cache_reconnect_and_stale_caption_are_guarded() {
    let (t, c) = setup();
    let p = t.path().join("notes.md");
    fs::write(&p, "First text").unwrap();
    let i = c
        .import(None, &[p.to_string_lossy().into()], Storage::Link)
        .unwrap()
        .remove(0);
    let id = &i.documents[0].id;
    assert_eq!(extract(&c, &i).text, "First text");
    fs::write(&p, "Second text").unwrap();
    assert_eq!(extract(&c, &i).status, "changed");
    assert!(c
        .apply_generated_caption(&i.id, &i.source_hash, "Old caption".into())
        .is_err());
    assert!(c.export(&[i.id.clone()], t.path()).is_err());
    let updated = c.refresh_document(&i.id, id, &i.source_hash, None).unwrap();
    assert_eq!(extract(&c, &updated).text, "Second text");
    let moved = t.path().join("moved.md");
    fs::rename(&p, &moved).unwrap();
    let absent = extract(&c, &updated);
    assert_eq!(absent.status, "missing");
    assert_eq!(absent.text, "Second text");
    assert_eq!(c.check(&[i.id.clone()]).unwrap().len(), 1);
    let repaired = c
        .refresh_document(&i.id, id, &updated.source_hash, Some(&moved))
        .unwrap();
    assert_eq!(extract(&c, &repaired).status, "ready");
    c.remove_document(&i.id, id, &repaired.source_hash).unwrap();
    assert!(moved.is_file());
}
#[test]
fn empty_corrupt_large_and_utf16_inputs_report_honest_status() {
    let (_t, c) = setup();
    let empty = c
        .import_document(None, "scan.pdf", &pdf(false), Storage::Copy, None)
        .unwrap();
    assert_eq!(extract(&c, &empty).status, "empty");
    let broken = c
        .import_document(None, "broken.pdf", b"%PDF-invalid", Storage::Copy, None)
        .unwrap();
    assert_eq!(extract(&c, &broken).status, "error");
    let big = c
        .import_document(None, "large.txt", &vec![b'x'; 200000], Storage::Copy, None)
        .unwrap();
    let text = extract(&c, &big);
    assert!(text.truncated);
    assert_eq!(text.text.len(), MAX_DOCUMENT_TEXT);
    let utf16: Vec<_> = [0xff, 0xfe]
        .into_iter()
        .chain("Unicode document".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let i = c
        .import_document(None, "unicode.txt", &utf16, Storage::Copy, None)
        .unwrap();
    assert_eq!(extract(&c, &i).text, "Unicode document");
    assert!(c
        .import_document(None, "old.doc", b"legacy", Storage::Copy, None)
        .is_err());
}
#[test]
fn mixed_import_and_moves_rebuild_and_transfer_keep_relationships() {
    let (t, mut c) = setup();
    let image = t.path().join("photo.png");
    fs::write(&image, encode_clipboard(&[0, 0, 0, 255], 1, 1).unwrap()).unwrap();
    let doc = t.path().join("reference.md");
    fs::write(&doc, "Evidence").unwrap();
    let item = c.create_item().unwrap();
    let item = c
        .import(
            Some(&item.id),
            &[
                image.to_string_lossy().into(),
                doc.to_string_lossy().into(),
                image.to_string_lossy().into(),
            ],
            Storage::Copy,
        )
        .unwrap()
        .remove(0);
    assert_eq!(item.assets.len(), 2);
    assert_eq!(item.documents.len(), 1);
    let other = c.create_item().unwrap();
    c.move_asset(&item.id, &other.id, &item.documents[0].id)
        .unwrap();
    let mut other = c.load(&other.id).unwrap();
    let id = other.documents[0].id.clone();
    other = c
        .update_document(
            &other.id,
            &id,
            &other.source_hash,
            "Research title".into(),
            "Unique note".into(),
            Role::CaptionSource,
        )
        .unwrap();
    c.refresh().unwrap();
    assert_eq!(c.list("Unique note", "all", 0, 10).unwrap().total, 1);
    assert!(c.load(&item.id).unwrap().documents.is_empty());
    let root = c.root.clone();
    drop(c);
    let moved = t.path().join("moved collection");
    fs::rename(root, &moved).unwrap();
    let (c, _) = Collection::open(&moved, false).unwrap();
    let i = c.load(&other.id).unwrap();
    assert_eq!(extract(&c, &i).text, "Evidence");
    assert_eq!(i.documents[0].role, Role::CaptionSource);
}
#[test]
fn schema_backup_metadata_guards_and_cache_rebuilding_work() {
    let (_t, c) = setup();
    let blank = c.create_item().unwrap();
    let before = fs::read(c.root.join("items").join(&blank.id).join("item.json")).unwrap();
    let i = c
        .import_document(
            Some(&blank.id),
            "test.txt",
            b"Reference",
            Storage::Copy,
            None,
        )
        .unwrap();
    let backup = c
        .root
        .join("items")
        .join(&i.id)
        .join(format!("item.v1.{}.backup.json", &blank.source_hash[..16]));
    assert_eq!(fs::read(backup).unwrap(), before);
    assert!(c
        .remove_document(&i.id, &i.documents[0].id, &blank.source_hash)
        .is_err());
    let mut bad = i.clone();
    bad.documents.clear();
    assert!(c.update(&mut bad).is_err());
    let first = extract(&c, &i);
    fs::remove_file(
        c.root
            .join(".nabor/document-text")
            .join(format!("{}.json", i.documents[0].id)),
    )
    .unwrap();
    assert_eq!(extract(&c, &i).text, first.text);
    assert_eq!(c.queue_preview(Some(vec![i.id])).unwrap().counts.pending, 1);
    assert!(document_extension(Path::new("test.exe")).is_err());
}

#[test]
fn document_promotion_preserves_website_references_and_backs_up_schema_two() {
    let (_t, c) = setup();
    let blank = c.create_item().unwrap();
    let linked = c
        .save_website_link(
            &blank.id,
            &blank.source_hash,
            None,
            "https://example.com/evidence".into(),
            "Reference page".into(),
            "User notes".into(),
        )
        .unwrap();
    let original = fs::read(c.root.join("items").join(&linked.id).join("item.json")).unwrap();
    let mut item = c
        .import_document(
            Some(&linked.id),
            "evidence.md",
            b"# Document evidence",
            Storage::Copy,
            None,
        )
        .unwrap();
    assert_eq!(item.schema_version, 3);
    assert_eq!(item.links, linked.links);
    let backup = c
        .root
        .join("items")
        .join(&item.id)
        .join(format!("item.v2.{}.backup.json", &linked.source_hash[..16]));
    assert_eq!(fs::read(backup).unwrap(), original);
    item.caption = "Reviewed caption".into();
    c.update(&mut item).unwrap();
    assert_eq!(c.load(&item.id).unwrap().documents.len(), 1);
}
