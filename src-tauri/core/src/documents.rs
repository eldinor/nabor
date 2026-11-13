use super::*;
use std::io::Read;

pub const MAX_DOCUMENT_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_DOCUMENT_TEXT: usize = 128_000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentAttachment {
    pub id: String,
    pub filename: String,
    pub format: String,
    pub storage: Storage,
    pub path: String,
    pub title: String,
    pub notes: String,
    pub role: Role,
    pub content_hash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentText {
    pub schema_version: u32,
    pub document_id: String,
    pub source_hash: String,
    pub status: String,
    pub text: String,
    pub warning: Option<String>,
    pub truncated: bool,
}
pub fn document_extension(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["pdf", "docx", "md", "txt"].contains(&ext.as_str()) {
        return Err("Supported documents: PDF, DOCX, Markdown (.md), text (.txt)".into());
    }
    Ok(ext)
}
pub fn document_bytes(path: &Path) -> Result<Vec<u8>> {
    let mut f = fs::File::open(path).map_err(|_| "Document is missing or cannot be read")?;
    if f.metadata().map_err(err)?.len() > MAX_DOCUMENT_BYTES {
        return Err("Document exceeds the 20 MB limit".into());
    }
    let mut bytes = vec![];
    Read::by_ref(&mut f)
        .take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("Document exceeds the 20 MB limit".into());
    }
    Ok(bytes)
}
pub fn document_hash(path: &Path) -> Result<String> {
    Ok(hash(&document_bytes(path)?))
}
pub fn validate_document_sources(root: &Path, item: &Item) -> Result<()> {
    for d in &item.documents {
        if document_hash(&document_path(root, item, d)?)? != d.content_hash {
            return Err(format!(
                "{} changed. Re-extract its text before saving a generated caption or exporting.",
                d.filename
            ));
        }
    }
    Ok(())
}
pub(crate) fn validate_documents(docs: &[DocumentAttachment]) -> Result<()> {
    if docs.len() > 10 {
        return Err("Attach up to 10 documents per item".into());
    }
    for d in docs {
        valid_id(&d.id)?;
        if !["pdf", "docx", "md", "txt"].contains(&d.format.as_str())
            || d.role == Role::Original
            || d.content_hash.len() != 64
        {
            return Err("Invalid document attachment".into());
        }
        if d.title.len() > 512 || d.notes.len() > 8000 {
            return Err("Document title exceeds 512 bytes or notes exceed 8,000 bytes".into());
        }
        if d.storage == Storage::Copy {
            safe_relative(&d.path)?;
        } else if !Path::new(&d.path).is_absolute() {
            return Err("Linked document paths must be absolute".into());
        }
    }
    Ok(())
}
pub fn document_path(root: &Path, item: &Item, d: &DocumentAttachment) -> Result<PathBuf> {
    valid_id(&item.id)?;
    valid_id(&d.id)?;
    if d.storage == Storage::Link {
        return Ok(PathBuf::from(&d.path));
    }
    safe_relative(&d.path)?;
    let dir = root.join("items").join(&item.id);
    no_symlink(&dir)?;
    let p = dir.join(&d.path);
    if p.exists()
        && !p
            .canonicalize()
            .map_err(err)?
            .starts_with(dir.canonicalize().map_err(err)?)
    {
        return Err("Managed document escapes its item folder".into());
    }
    Ok(p)
}
fn cache_path(root: &Path, id: &str) -> Result<PathBuf> {
    valid_id(id)?;
    Ok(root.join(".nabor/document-text").join(format!("{id}.json")))
}
pub fn cached_document_text(root: &Path, d: &DocumentAttachment) -> Option<DocumentText> {
    let p = cache_path(root, &d.id).ok()?;
    no_symlink(&p).ok()?;
    if fs::metadata(&p).ok()?.len() > 1_000_000 {
        return None;
    }
    let c: DocumentText = serde_json::from_slice(&fs::read(p).ok()?).ok()?;
    (c.schema_version == 1
        && c.document_id == d.id
        && c.source_hash == d.content_hash
        && c.text.len() <= MAX_DOCUMENT_TEXT
        && ["ready", "empty", "error"].contains(&c.status.as_str()))
    .then_some(c)
}
pub fn prepare_document_text(
    root: &Path,
    item: &Item,
    d: &DocumentAttachment,
    extract: impl FnOnce(&Path, &str) -> Result<String>,
) -> Result<DocumentText> {
    let mut c = cached_document_text(root, d).unwrap_or(DocumentText {
        schema_version: 1,
        document_id: d.id.clone(),
        source_hash: d.content_hash.clone(),
        status: "error".into(),
        text: String::new(),
        warning: None,
        truncated: false,
    });
    let p = document_path(root, item, d)?;
    if !p.is_file() {
        c.status = "missing".into();
        c.warning = Some("File is missing. Locate it to use this reference.".into());
        return Ok(c);
    }
    let actual = document_hash(&p)?;
    if actual != d.content_hash {
        c.status = "changed".into();
        c.warning = Some("File changed. Re-extract text before using it with the agent.".into());
        return Ok(c);
    }
    if let Some(c) = cached_document_text(root, d) {
        return Ok(c);
    }
    match extract(&p, &d.format) {
        Ok(text) => {
            c.truncated = text.len() > MAX_DOCUMENT_TEXT;
            c.text = bounded_text(&text, MAX_DOCUMENT_TEXT);
            c.status = if c.text.trim().is_empty() {
                "empty"
            } else {
                "ready"
            }
            .into();
            c.warning = if c.status == "empty" {
                Some("No extractable text. Scanned PDFs need OCR or image references.".into())
            } else if c.truncated {
                Some("Extracted text was truncated to 128,000 bytes; the original file is unchanged.".into())
            } else {
                None
            };
        }
        Err(e) => {
            c.status = "error".into();
            c.warning = Some(e);
            c.text.clear();
        }
    }
    if document_hash(&p)? != actual {
        return Err("Document changed during extraction. Re-extract it again.".into());
    }
    if !root.join("collection.json").is_file() {
        return Err("Collection moved before extracted text could be cached".into());
    }
    let dir = root.join(".nabor/document-text");
    fs::create_dir_all(&dir).map_err(err)?;
    no_symlink(&dir)?;
    atomic_write(
        &cache_path(root, &d.id)?,
        &serde_json::to_vec(&c).map_err(err)?,
    )?;
    Ok(c)
}
pub fn bounded_text(text: &str, limit: usize) -> String {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].into()
}
pub fn extract_document_text(path: &Path, format: &str) -> Result<String> {
    let bytes = document_bytes(path)?;
    match format {
        "md" | "txt" => {
            let s = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
                if (bytes.len() - 2) % 2 != 0 {
                    return Err("Invalid UTF-16 document".into());
                }
                let le = bytes[0] == 0xff;
                String::from_utf16(
                    &bytes[2..]
                        .chunks_exact(2)
                        .map(|b| {
                            if le {
                                u16::from_le_bytes([b[0], b[1]])
                            } else {
                                u16::from_be_bytes([b[0], b[1]])
                            }
                        })
                        .collect::<Vec<_>>(),
                )
                .map_err(|_| "Invalid UTF-16 document")?
            } else {
                String::from_utf8(bytes)
                    .map_err(|_| "Save this text document as UTF-8 or UTF-16")?
            };
            if s.contains('\0') {
                return Err("This file is not a supported text document".into());
            }
            Ok(bounded_text(
                s.trim_start_matches('\u{feff}'),
                MAX_DOCUMENT_TEXT + 4,
            ))
        }
        "pdf" => {
            if !bytes.starts_with(b"%PDF-") {
                return Err("Invalid PDF document".into());
            }
            pdf_extract::extract_text_from_mem(&bytes)
                .map(|s| bounded_text(&s, MAX_DOCUMENT_TEXT + 4))
                .map_err(|_| {
                    "Cannot extract this PDF. It may be damaged, encrypted, or unsupported.".into()
                })
        }
        "docx" => docx_text(bytes),
        _ => Err("Unsupported document format".into()),
    }
}
fn docx_text(bytes: Vec<u8>) -> Result<String> {
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "Invalid DOCX archive")?;
    if zip.len() > 2048 {
        return Err("DOCX archive has too many entries".into());
    }
    let mut names: Vec<_> = zip
        .file_names()
        .filter(|n| {
            *n == "word/document.xml"
                || *n == "word/footnotes.xml"
                || *n == "word/endnotes.xml"
                || (n.starts_with("word/header") || n.starts_with("word/footer"))
                    && n.ends_with(".xml")
        })
        .map(str::to_owned)
        .collect();
    if !names.iter().any(|n| n == "word/document.xml") {
        return Err("DOCX has no document body".into());
    }
    names.sort();
    let mut text = String::new();
    let mut total = 0;
    for name in names {
        let mut entry = zip.by_name(&name).map_err(|_| "Cannot read DOCX text")?;
        total += entry.size();
        if total > 16 * 1024 * 1024 {
            return Err("DOCX text exceeds the extraction limit".into());
        }
        let mut xml = String::new();
        entry
            .by_ref()
            .take(16 * 1024 * 1024 + 1)
            .read_to_string(&mut xml)
            .map_err(|_| "Cannot decode DOCX XML")?;
        if xml.len() > 16 * 1024 * 1024 {
            return Err("DOCX text exceeds the extraction limit".into());
        }
        let mut reader = quick_xml::Reader::from_str(&xml);
        let mut in_text = false;
        loop {
            use quick_xml::events::Event;
            match reader.read_event().map_err(|_| "Invalid DOCX XML")? {
                Event::Start(e) => {
                    if e.local_name().as_ref() == "t" {
                        in_text = true;
                    }
                }
                Event::End(e) => match e.local_name().as_ref() {
                    "t" => in_text = false,
                    "p" | "tr" => text.push('\n'),
                    "tc" => text.push('\t'),
                    _ => {}
                },
                Event::Empty(e) => match e.local_name().as_ref() {
                    "tab" => text.push('\t'),
                    "br" | "cr" => text.push('\n'),
                    _ => {}
                },
                Event::Text(e) if in_text => {
                    text.push_str(
                        &quick_xml::escape::unescape(
                            &e.xml_content(quick_xml::XmlVersion::Implicit1_0),
                        )
                        .map_err(|_| "Invalid DOCX text entity")?,
                    );
                }
                Event::GeneralRef(e) if in_text => {
                    let raw: &str = e.as_ref();
                    text.push_str(
                        &quick_xml::escape::unescape(&format!("&{raw};"))
                            .map_err(|_| "Unsupported DOCX entity")?,
                    );
                }
                Event::CData(e) if in_text => {
                    text.push_str(&e.xml_content(quick_xml::XmlVersion::Implicit1_0));
                }
                Event::DocType(_) => return Err("DOCX XML definitions are unsupported".into()),
                Event::Eof => break,
                _ => {}
            }
            if text.len() > MAX_DOCUMENT_TEXT {
                return Ok(bounded_text(&text, MAX_DOCUMENT_TEXT + 4));
            }
        }
        text.push('\n');
    }
    Ok(text)
}
impl Collection {
    pub fn import_document(
        &self,
        item_id: Option<&str>,
        filename: &str,
        bytes: &[u8],
        storage: Storage,
        source: Option<&Path>,
    ) -> Result<Item> {
        document_extension(Path::new(filename))?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("Document exceeds the 20 MB limit".into());
        }
        let mut item = if let Some(id) = item_id {
            self.load(id)?
        } else {
            let mut i = self.create_item()?;
            i.title = Path::new(filename)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into();
            i
        };
        self.add_document(&mut item, filename, bytes, storage, source)?;
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
    pub(crate) fn add_document(
        &self,
        item: &mut Item,
        filename: &str,
        bytes: &[u8],
        storage: Storage,
        source: Option<&Path>,
    ) -> Result<()> {
        let format = document_extension(Path::new(filename))?;
        if item.documents.len() >= 10 {
            return Err("Attach up to 10 documents per item".into());
        }
        let document_id = id();
        let path = if storage == Storage::Copy {
            let dir = self.item_dir(&item.id)?.join("assets");
            fs::create_dir_all(&dir).map_err(err)?;
            no_symlink(&dir)?;
            let relative = format!("assets/{document_id}.{format}");
            atomic_write(&self.item_dir(&item.id)?.join(&relative), bytes)?;
            relative
        } else {
            source
                .ok_or("Choose a file path to link a document")?
                .canonicalize()
                .map_err(err)?
                .to_string_lossy()
                .to_string()
        };
        item.documents.push(DocumentAttachment {
            id: document_id,
            filename: Path::new(filename)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            format,
            path,
            storage,
            title: String::new(),
            notes: String::new(),
            role: Role::Reference,
            content_hash: hash(bytes),
        });
        Ok(())
    }
    pub fn update_document(
        &self,
        item_id: &str,
        document_id: &str,
        expected_hash: &str,
        title: String,
        notes: String,
        role: Role,
    ) -> Result<Item> {
        let mut item = self.load(item_id)?;
        if item.source_hash != expected_hash {
            return Err("Item changed. Reopen it before editing the document.".into());
        }
        let d = item
            .documents
            .iter_mut()
            .find(|d| d.id == document_id)
            .ok_or("Document not found")?;
        d.title = title;
        d.notes = notes;
        d.role = role;
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
    pub fn refresh_document(
        &self,
        item_id: &str,
        document_id: &str,
        expected_hash: &str,
        replacement: Option<&Path>,
    ) -> Result<Item> {
        let mut item = self.load(item_id)?;
        if item.source_hash != expected_hash {
            return Err("Item changed. Reopen it before reconnecting or extracting.".into());
        }
        let index = item
            .documents
            .iter()
            .position(|d| d.id == document_id)
            .ok_or("Document not found")?;
        if let Some(path) = replacement {
            if item.documents[index].storage != Storage::Link {
                return Err("Only linked documents can be reconnected".into());
            }
            if document_extension(path)? != item.documents[index].format {
                return Err("Choose a replacement of the same document format".into());
            }
            item.documents[index].path = path
                .canonicalize()
                .map_err(err)?
                .to_string_lossy()
                .to_string();
        }
        let p = document_path(&self.root, &item, &item.documents[index])?;
        let actual = document_hash(&p)?;
        let changed = actual != item.documents[index].content_hash || replacement.is_some();
        item.documents[index].content_hash = actual;
        let cache = cache_path(&self.root, document_id)?;
        if cache.exists() {
            no_symlink(&cache)?;
            fs::remove_file(cache).map_err(err)?;
        }
        if changed {
            self.save_new_revision(&mut item)?;
        }
        Ok(item)
    }
    pub fn remove_document(
        &self,
        item_id: &str,
        document_id: &str,
        expected_hash: &str,
    ) -> Result<Item> {
        let mut item = self.load(item_id)?;
        if item.source_hash != expected_hash {
            return Err("Item changed. Reopen it before removing the document.".into());
        }
        let before = item.documents.len();
        item.documents.retain(|d| d.id != document_id);
        if before == item.documents.len() {
            return Err("Document not found".into());
        }
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
}
