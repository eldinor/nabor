use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;
mod captions;
pub use captions::*;
mod conversation;
pub use conversation::*;
mod links;
pub use links::*;
mod documents;
pub use documents::*;

pub type Result<T> = std::result::Result<T, String>;
pub const VERSION: u32 = 1;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
fn valid_id(s: &str) -> Result<()> {
    Uuid::parse_str(s)
        .map(|_| ())
        .map_err(|_| "Invalid item or asset ID".into())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Original,
    CaptionSource,
    Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Storage {
    Copy,
    Link,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub role: Role,
    pub storage: Storage,
    pub path: String,
    pub filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionRevision {
    pub caption: String,
    pub saved_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub caption: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caption_history: Vec<CaptionRevision>,
    pub notes: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub revision: u64,
    pub assets: Vec<Asset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<WebsiteLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub documents: Vec<DocumentAttachment>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_hash: String,
}
impl Item {
    pub fn empty() -> Self {
        Self {
            schema_version: VERSION,
            id: id(),
            title: "Untitled item".into(),
            caption: String::new(),
            caption_history: vec![],
            notes: String::new(),
            created_at: now(),
            updated_at: now(),
            revision: 0,
            assets: vec![],
            links: vec![],
            documents: vec![],
            source_hash: String::new(),
        }
    }
    fn validate(&self) -> Result<()> {
        if ![1, 2, 3].contains(&self.schema_version) {
            return Err(format!("Unsupported item schema {}. This app supports versions 1, 2, and 3; the manifest was left untouched.", self.schema_version));
        }
        if self.schema_version == 1 && !self.links.is_empty() {
            return Err("Website attachments require item schema 2".into());
        }
        validate_links(&self.links)?;
        validate_documents(&self.documents)?;
        if self.schema_version < 3 && !self.documents.is_empty() {
            return Err("Document attachments require item schema 3".into());
        }
        valid_id(&self.id)?;
        if self
            .assets
            .iter()
            .filter(|a| a.role == Role::Original)
            .count()
            > 1
        {
            return Err("An item can have only one original".into());
        }
        let mut ids = std::collections::HashSet::new();
        for a in &self.assets {
            valid_id(&a.id)?;
            if !ids.insert(&a.id) {
                return Err("Duplicate asset ID".into());
            }
            if a.storage == Storage::Copy {
                safe_relative(&a.path)?;
            } else if !Path::new(&a.path).is_absolute() {
                return Err("Linked paths must be absolute".into());
            }
        }
        for link in &self.links {
            if !ids.insert(&link.id) {
                return Err("Duplicate attachment ID".into());
            }
        }
        for d in &self.documents {
            if !ids.insert(&d.id) {
                return Err("Duplicate attachment ID".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub caption: String,
    pub asset_count: usize,
    pub missing_original: bool,
    pub missing_files: bool,
    pub empty_caption: bool,
    pub thumbnail_asset: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub items: Vec<Summary>,
    pub total: usize,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub indexed: usize,
    pub warnings: Vec<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Missing {
    pub item_id: String,
    pub title: String,
    pub asset_id: String,
    pub filename: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub schema_version: u32,
    pub default_storage: Storage,
    #[serde(default)]
    pub caption_instructions: String,
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("No parent directory")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(err)?;
    temp.write_all(bytes).map_err(err)?;
    temp.as_file().sync_all().map_err(err)?;
    temp.persist(path).map_err(err)?;
    Ok(())
}
fn safe_relative(s: &str) -> Result<()> {
    let p = Path::new(s);
    if p.is_absolute()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
        || s.contains('\\')
        || s.contains(':')
    {
        return Err("Unsafe managed path".into());
    }
    let parts: Vec<_> = p.components().collect();
    if parts.len() != 2 || parts[0].as_os_str() != "assets" {
        return Err("Managed files must be inside assets/".into());
    }
    Ok(())
}
fn no_symlink(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path)
        .map_err(err)?
        .file_type()
        .is_symlink()
    {
        return Err("Collection directories cannot be symbolic links".into());
    }
    Ok(())
}
fn image_extension(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff"].contains(&ext.as_str()) {
        return Err("Supported images: PNG, JPEG, WebP, GIF, BMP, TIFF".into());
    }
    Ok(ext)
}

pub struct Collection {
    pub root: PathBuf,
    db: Connection,
    pub settings: Settings,
    _lock: fs::File,
}
impl Collection {
    pub fn apply_generated_caption(
        &self,
        item_id: &str,
        expected_hash: &str,
        caption: String,
    ) -> Result<Item> {
        if caption.trim().is_empty() || caption.len() > 32_000 {
            return Err("Caption is empty or too long".into());
        }
        let mut item = self.load(item_id)?;
        if item.source_hash != expected_hash || !item.caption.trim().is_empty() {
            return Err("This item changed or already has a caption. Generate a new draft after reviewing the item.".into());
        }
        validate_document_sources(&self.root, &item)?;
        item.caption = caption;
        self.update(&mut item)?;
        Ok(item)
    }
    pub fn apply_caption_revision(
        &self,
        item_id: &str,
        expected_hash: &str,
        caption: String,
    ) -> Result<Item> {
        if caption.trim().is_empty() || caption.len() > 32_000 {
            return Err("Caption is empty or too long".into());
        }
        let mut item = self.load(item_id)?;
        if item.source_hash != expected_hash {
            return Err(
                "This item changed. Reopen its caption before saving a replacement.".into(),
            );
        }
        validate_document_sources(&self.root, &item)?;
        item.caption = caption;
        self.update(&mut item)?;
        Ok(item)
    }
    pub fn open(root: &Path, create: bool) -> Result<(Self, Report)> {
        if create {
            fs::create_dir_all(root).map_err(err)?;
        }
        let root = root.canonicalize().map_err(err)?;
        let marker = root.join("collection.json");
        if !marker.exists() && !create {
            return Err(
                "This folder is not a Nabor collection. Use Create collection to initialize it."
                    .into(),
            );
        }
        if !marker.exists() {
            if fs::read_dir(&root).map_err(err)?.next().is_some() {
                return Err("Create a collection in an empty folder".into());
            }
            atomic_write(
                &marker,
                &serde_json::to_vec_pretty(&Settings {
                    schema_version: VERSION,
                    default_storage: Storage::Copy,
                    caption_instructions: String::new(),
                })
                .map_err(err)?,
            )?;
        }
        let settings: Settings =
            serde_json::from_slice(&fs::read(marker).map_err(err)?).map_err(err)?;
        if settings.schema_version != VERSION {
            return Err("Unsupported collection version; no files were changed".into());
        }
        for dir in ["items", ".nabor", ".nabor/thumbnails", ".nabor/trash"] {
            let p = root.join(dir);
            fs::create_dir_all(&p).map_err(err)?;
            no_symlink(&p)?;
        }
        use fs2::FileExt;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(".nabor/collection.lock"))
            .map_err(err)?;
        lock.try_lock_exclusive()
            .map_err(|_| "This collection is already open in another Nabor process")?;
        let db_path = root.join(".nabor/index.sqlite");
        let mut db = Connection::open(&db_path).map_err(err)?;
        if db.execute_batch("PRAGMA quick_check;").is_err() {
            drop(db);
            fs::rename(
                &db_path,
                root.join(".nabor")
                    .join(format!("index-corrupt-{}.sqlite", id())),
            )
            .map_err(err)?;
            for suffix in ["-wal", "-shm"] {
                let p = PathBuf::from(format!("{}{suffix}", db_path.display()));
                if p.exists() {
                    fs::rename(
                        &p,
                        root.join(".nabor").join(format!("cache-backup-{}", id())),
                    )
                    .map_err(err)?;
                }
            }
            db = Connection::open(&db_path).map_err(err)?;
        }
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS items (id TEXT PRIMARY KEY, title TEXT NOT NULL, caption TEXT NOT NULL, notes TEXT NOT NULL, filenames TEXT NOT NULL, updated INTEGER, asset_count INTEGER, missing_original INTEGER, missing_files INTEGER, empty_caption INTEGER, thumbnail_asset TEXT);").map_err(err)?;
        let mut c = Self {
            root,
            db,
            settings,
            _lock: lock,
        };
        c.init_captions()?;
        let mut report = c.refresh()?;
        if let Err(e) = c.recover_queue() {
            report.warnings.push(e);
        }
        Ok((c, report))
    }
    fn item_dir(&self, item_id: &str) -> Result<PathBuf> {
        valid_id(item_id)?;
        let p = self.root.join("items").join(item_id);
        if p.exists() {
            no_symlink(&p)?;
        }
        Ok(p)
    }
    pub fn load(&self, item_id: &str) -> Result<Item> {
        let p = self.item_dir(item_id)?.join("item.json");
        no_symlink(&p)?;
        let bytes = fs::read(&p).map_err(err)?;
        let mut item: Item = serde_json::from_slice(&bytes).map_err(err)?;
        item.source_hash = hash(&bytes);
        item.validate()?;
        if item.id != item_id {
            return Err("Manifest ID differs from folder name".into());
        }
        Ok(item)
    }
    pub fn resolve(&self, item: &Item, a: &Asset) -> Result<PathBuf> {
        if a.storage == Storage::Link {
            return Ok(PathBuf::from(&a.path));
        }
        safe_relative(&a.path)?;
        let dir = self.item_dir(&item.id)?;
        let p = dir.join(&a.path);
        if p.exists() {
            let canonical = p.canonicalize().map_err(err)?;
            if !canonical.starts_with(dir.canonicalize().map_err(err)?) {
                return Err("Managed asset escapes its item folder".into());
            }
        }
        Ok(p)
    }
    fn save_new_revision(&self, item: &mut Item) -> Result<()> {
        let target_version = if !item.documents.is_empty() {
            3
        } else if !item.links.is_empty() {
            2
        } else {
            item.schema_version
        };
        if item.schema_version < target_version {
            let disk = self.load(&item.id)?;
            if disk.revision != item.revision || disk.source_hash != item.source_hash {
                return Err("This item changed externally. Refresh before saving.".into());
            }
            let dir = self.item_dir(&item.id)?;
            let backup = dir.join(format!(
                "item.v{}.{}.backup.json",
                disk.schema_version,
                &disk.source_hash[..16]
            ));
            if backup.exists() {
                no_symlink(&backup)?;
                if !backup.is_file()
                    || fs::read(&backup).map_err(err)?
                        != fs::read(dir.join("item.json")).map_err(err)?
                {
                    return Err(
                        "The migration backup already exists and does not match this item".into(),
                    );
                }
            } else {
                atomic_write(&backup, &fs::read(dir.join("item.json")).map_err(err)?)?;
            }
            item.schema_version = target_version;
        }
        item.validate()?;
        let dir = self.item_dir(&item.id)?;
        if dir.join("item.json").exists() {
            let disk = self.load(&item.id)?;
            if disk.revision != item.revision || disk.source_hash != item.source_hash {
                return Err("This item changed externally. Refresh before saving to avoid overwriting changes.".into());
            }
            item.caption_history = disk.caption_history;
            if disk.caption != item.caption && !disk.caption.trim().is_empty() {
                item.caption_history.push(CaptionRevision {
                    caption: disk.caption,
                    saved_at: disk.updated_at,
                });
            }
            if item.caption_history.len() > 20 {
                item.caption_history
                    .drain(..item.caption_history.len() - 20);
            }
        }
        fs::create_dir_all(dir.join("assets")).map_err(err)?;
        no_symlink(&dir.join("assets"))?;
        item.updated_at = now();
        item.revision += 1;
        // Caption text is a convenience derivative. The atomic manifest is authoritative.
        let mut stored = item.clone();
        stored.source_hash.clear();
        let bytes = serde_json::to_vec_pretty(&stored).map_err(err)?;
        if let Err(e) = atomic_write(&dir.join("item.json"), &bytes) {
            item.revision -= 1;
            return Err(e);
        }
        item.source_hash = hash(&bytes);
        if atomic_write(&dir.join("caption.txt"), item.caption.as_bytes()).is_err() {
            self.log("caption-derivative-write-failed");
        }
        self.index(item)
    }
    pub fn update(&self, item: &mut Item) -> Result<()> {
        let existing = self.load(&item.id)?;
        // Metadata edits cannot smuggle arbitrary new paths into a collection.
        if serde_json::to_value(&existing.assets).map_err(err)?
            != serde_json::to_value(&item.assets).map_err(err)?
        {
            return Err("Use attachment commands to change assets".into());
        }
        if existing.links != item.links
            || existing.documents != item.documents
            || existing.schema_version != item.schema_version
        {
            return Err("Use link attachment commands to change website links".into());
        }
        self.save_new_revision(item)
    }
    pub fn create_item(&self) -> Result<Item> {
        let mut item = Item::empty();
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
    pub fn import(
        &self,
        item_id: Option<&str>,
        paths: &[String],
        storage: Storage,
    ) -> Result<Vec<Item>> {
        // Validate every input before changing anything.
        for s in paths {
            let p = Path::new(s);
            if image_extension(p).is_err() {
                document_extension(p)?;
                document_bytes(p)?;
            }
            if !p.is_file() {
                return Err(format!("File unavailable: {}", p.display()));
            }
        }
        if let Some(item_id) = item_id {
            let mut item = self.load(item_id)?;
            for s in paths {
                if document_extension(Path::new(s)).is_ok() {
                    self.add_document(
                        &mut item,
                        Path::new(s)
                            .file_name()
                            .unwrap_or_default()
                            .to_str()
                            .ok_or("Invalid document filename")?,
                        &document_bytes(Path::new(s))?,
                        storage.clone(),
                        Some(Path::new(s)),
                    )?;
                    continue;
                }
                let role = if item.assets.iter().any(|a| a.role == Role::Original) {
                    Role::Reference
                } else {
                    Role::Original
                };
                self.add_file(&mut item, Path::new(s), storage.clone(), role)?;
            }
            self.save_new_revision(&mut item)?;
            Ok(vec![item])
        } else {
            let mut items = vec![];
            for s in paths {
                let p = Path::new(s);
                if document_extension(p).is_ok() {
                    items.push(
                        self.import_document(
                            None,
                            p.file_name()
                                .unwrap_or_default()
                                .to_str()
                                .ok_or("Invalid document filename")?,
                            &document_bytes(p)?,
                            storage.clone(),
                            Some(p),
                        )?,
                    );
                    continue;
                }
                let mut item = Item::empty();
                item.title = p.file_stem().unwrap_or_default().to_string_lossy().into();
                self.add_file(&mut item, p, storage.clone(), Role::Original)?;
                self.save_new_revision(&mut item)?;
                items.push(item);
            }
            Ok(items)
        }
    }
    fn add_file(&self, item: &mut Item, p: &Path, storage: Storage, role: Role) -> Result<()> {
        let ext = image_extension(p)?;
        let asset_id = id();
        let filename = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let path = if storage == Storage::Copy {
            let dir = self.item_dir(&item.id)?.join("assets");
            fs::create_dir_all(&dir).map_err(err)?;
            no_symlink(&dir)?;
            let relative = format!("assets/{asset_id}.{ext}");
            let target = self.item_dir(&item.id)?.join(&relative);
            let mut source = fs::File::open(p).map_err(err)?;
            let mut temp = tempfile::NamedTempFile::new_in(&dir).map_err(err)?;
            std::io::copy(&mut source, &mut temp).map_err(err)?;
            temp.as_file().sync_all().map_err(err)?;
            temp.persist(&target).map_err(err)?;
            relative
        } else {
            p.canonicalize().map_err(err)?.to_string_lossy().to_string()
        };
        item.assets.push(Asset {
            id: asset_id,
            role,
            storage,
            path,
            filename,
            source_url: None,
        });
        Ok(())
    }
    pub fn clipboard(&self, item_id: Option<&str>, png: &[u8]) -> Result<Item> {
        let mut item = match item_id {
            Some(i) => self.load(i)?,
            None => Item::empty(),
        };
        let mut temp = tempfile::Builder::new()
            .suffix(".png")
            .tempfile()
            .map_err(err)?;
        temp.write_all(png).map_err(err)?;
        let role = if item.assets.iter().any(|a| a.role == Role::Original) {
            Role::Reference
        } else {
            Role::Original
        };
        self.add_file(&mut item, temp.path(), Storage::Copy, role)?;
        item.assets.last_mut().unwrap().filename = "Clipboard image.png".into();
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
    pub fn import_image(
        &self,
        item_id: Option<&str>,
        filename: &str,
        bytes: &[u8],
        source_url: Option<String>,
    ) -> Result<Item> {
        let extension = validated_image_extension(bytes)?;
        if let Some(url) = &source_url {
            if url.len() > 16_384 {
                return Err("Source URL is too long".into());
            }
            let url = url::Url::parse(url).map_err(|_| "Invalid source URL")?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err("Source URL must be an HTTP(S) image URL without credentials".into());
            }
        }
        let mut item = match item_id {
            Some(id) => self.load(id)?,
            None => Item::empty(),
        };
        let name = filename
            .rsplit(['/', '\\'])
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("Web image")
            .chars()
            .take(240)
            .collect::<String>();
        if item_id.is_none() {
            item.title = std::path::Path::new(&name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
        }
        let mut temp = tempfile::Builder::new()
            .suffix(&format!(".{extension}"))
            .tempfile()
            .map_err(err)?;
        temp.write_all(bytes).map_err(err)?;
        let role = if item.assets.iter().any(|a| a.role == Role::Original) {
            Role::Reference
        } else {
            Role::Original
        };
        self.add_file(&mut item, temp.path(), Storage::Copy, role)?;
        let asset = item.assets.last_mut().unwrap();
        asset.filename = if Path::new(&name).extension().is_some() {
            name
        } else {
            format!("{name}.{extension}")
        };
        asset.source_url = source_url;
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
    pub fn set_role(&self, item_id: &str, asset_id: &str, role: Role) -> Result<Item> {
        let mut item = self.load(item_id)?;
        if !item.assets.iter().any(|a| a.id == asset_id) {
            return Err("Attachment not found".into());
        }
        if role == Role::Original {
            for a in &mut item.assets {
                if a.role == Role::Original {
                    a.role = Role::Reference;
                }
            }
        }
        item.assets
            .iter_mut()
            .find(|a| a.id == asset_id)
            .unwrap()
            .role = role;
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
    pub fn detach(&self, item_id: &str, asset_id: &str) -> Result<Item> {
        let mut item = self.load(item_id)?;
        let before = item.assets.len();
        item.assets.retain(|a| a.id != asset_id);
        if before == item.assets.len() {
            return Err("Attachment not found".into());
        }
        // Orphaned managed files stay recoverable; export uses the manifest only.
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
    pub fn move_asset(&self, from: &str, to: &str, asset_id: &str) -> Result<()> {
        if from == to {
            return Err("Choose a different destination".into());
        }
        let mut source = self.load(from)?;
        let mut target = self.load(to)?;
        if let Some(mut d) = source.documents.iter().find(|d| d.id == asset_id).cloned() {
            let original = document_path(&self.root, &source, &d)?;
            let bytes = document_bytes(&original)?;
            d.id = id();
            if d.storage == Storage::Copy {
                let path = format!("assets/{}.{}", d.id, d.format);
                atomic_write(&self.item_dir(&target.id)?.join(&path), &bytes)?;
                d.path = path;
            }
            d.role = Role::Reference;
            d.content_hash = hash(&bytes);
            target.documents.push(d);
            self.save_new_revision(&mut target)?;
            source.documents.retain(|d| d.id != asset_id);
            self.save_new_revision(&mut source)?;
            return Ok(());
        }
        if let Some(mut link) = source.links.iter().find(|l| l.id == asset_id).cloned() {
            link.id = id();
            target.links.push(link);
            // As with files, save the destination before removing the source.
            self.save_new_revision(&mut target)?;
            source.links.retain(|l| l.id != asset_id);
            self.save_new_revision(&mut source)?;
            return Ok(());
        }
        let a = source
            .assets
            .iter()
            .find(|a| a.id == asset_id)
            .ok_or("Attachment not found")?
            .clone();
        let p = self.resolve(&source, &a)?;
        self.add_file(&mut target, &p, a.storage.clone(), Role::Reference)?;
        target.assets.last_mut().unwrap().filename = a.filename.clone();
        target.assets.last_mut().unwrap().source_url = a.source_url.clone();
        // Save destination first: a failed move can duplicate a reference but cannot lose it.
        self.save_new_revision(&mut target)?;
        source.assets.retain(|a| a.id != asset_id);
        self.save_new_revision(&mut source)?;
        Ok(())
    }
    pub fn reconnect(&self, item_id: &str, asset_id: &str, path: &Path) -> Result<Item> {
        image_extension(path)?;
        if !path.is_file() {
            return Err("Choose an existing image".into());
        }
        let mut item = self.load(item_id)?;
        let a = item
            .assets
            .iter_mut()
            .find(|a| a.id == asset_id)
            .ok_or("Attachment not found")?;
        if a.storage != Storage::Link {
            return Err("Only linked attachments can be reconnected".into());
        }
        a.path = path
            .canonicalize()
            .map_err(err)?
            .to_string_lossy()
            .to_string();
        let _ = fs::remove_file(
            self.root
                .join(".nabor/thumbnails")
                .join(format!("{}.stamp", a.id)),
        );
        self.save_new_revision(&mut item)?;
        Ok(item)
    }
    pub fn set_default(&mut self, storage: Storage) -> Result<()> {
        self.settings.default_storage = storage;
        atomic_write(
            &self.root.join("collection.json"),
            &serde_json::to_vec_pretty(&self.settings).map_err(err)?,
        )
    }
    fn index(&self, item: &Item) -> Result<()> {
        let missing = item
            .assets
            .iter()
            .any(|a| self.resolve(item, a).map(|p| !p.is_file()).unwrap_or(true))
            || item.documents.iter().any(|d| {
                document_path(&self.root, item, d)
                    .map(|p| !p.is_file())
                    .unwrap_or(true)
            });
        let original = item.assets.iter().find(|a| a.role == Role::Original);
        let missing_original =
            original.is_none_or(|a| self.resolve(item, a).map(|p| !p.is_file()).unwrap_or(true));
        let thumb = original
            .or_else(|| item.assets.first())
            .map(|a| a.id.clone());
        self.db
            .execute(
                "INSERT OR REPLACE INTO items VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    item.id,
                    item.title,
                    item.caption,
                    item.notes,
                    item.assets
                        .iter()
                        .map(|a| a.filename.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                        + " "
                        + &item
                            .links
                            .iter()
                            .flat_map(|l| [&*l.title, &*l.url, &*l.notes])
                            .collect::<Vec<_>>()
                            .join(" ")
                        + " "
                        + &item
                            .documents
                            .iter()
                            .flat_map(|d| [&*d.filename, &*d.title, &*d.notes])
                            .collect::<Vec<_>>()
                            .join(" "),
                    item.updated_at,
                    item.assets.len() + item.links.len() + item.documents.len(),
                    missing_original,
                    missing,
                    item.caption.trim().is_empty(),
                    thumb
                ],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn refresh(&mut self) -> Result<Report> {
        let mut report = Report {
            indexed: 0,
            warnings: vec![],
        };
        let mut loaded = vec![];
        for entry in fs::read_dir(self.root.join("items")).map_err(err)? {
            let entry = entry.map_err(err)?;
            if !entry.file_type().map_err(err)?.is_dir() {
                continue;
            }
            let item_id = entry.file_name().to_string_lossy().to_string();
            match self.load(&item_id) {
                Ok(item) => loaded.push(item),
                Err(e) => report.warnings.push(format!("{item_id}: {e}")),
            }
        }
        self.db
            .execute_batch("BEGIN IMMEDIATE; DELETE FROM items;")
            .map_err(err)?;
        for item in &loaded {
            if let Err(e) = self.index(item) {
                let _ = self.db.execute_batch("ROLLBACK;");
                return Err(e);
            }
            report.indexed += 1;
        }
        self.db.execute_batch("COMMIT;").map_err(err)?;
        report.warnings.extend(self.rebuild_caption_index()?);
        Ok(report)
    }
    pub fn list(&self, query: &str, filter: &str, offset: usize, limit: usize) -> Result<Page> {
        let condition = match filter {
            "missing-original" => "missing_original=1",
            "missing-files" => "missing_files=1",
            "empty-caption" => "empty_caption=1",
            _ => "1=1",
        };
        let search = format!(
            "%{}%",
            query
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let clause=format!("{condition} AND (title LIKE ?1 ESCAPE '\\' OR caption LIKE ?1 ESCAPE '\\' OR notes LIKE ?1 ESCAPE '\\' OR filenames LIKE ?1 ESCAPE '\\')");
        let total: usize = self
            .db
            .query_row(
                &format!("SELECT COUNT(*) FROM items WHERE {clause}"),
                params![search],
                |r| r.get(0),
            )
            .map_err(err)?;
        let mut stmt=self.db.prepare(&format!("SELECT id,title,substr(caption,1,240),asset_count,missing_original,missing_files,empty_caption,thumbnail_asset FROM items WHERE {clause} ORDER BY updated DESC,id LIMIT ?2 OFFSET ?3")).map_err(err)?;
        let items = stmt
            .query_map(params![search, limit.min(200), offset], |r| {
                Ok(Summary {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    caption: r.get(2)?,
                    asset_count: r.get(3)?,
                    missing_original: r.get(4)?,
                    missing_files: r.get(5)?,
                    empty_caption: r.get(6)?,
                    thumbnail_asset: r.get(7)?,
                })
            })
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(Page { items, total })
    }
    /// Select and validate exactly the material included in an agent request.
    pub fn agent_assets<'a>(
        &self,
        item: &'a Item,
        only_attachments: bool,
        require_material: bool,
    ) -> Result<Vec<&'a Asset>> {
        let assets: Vec<_> = item
            .assets
            .iter()
            .filter(|a| !only_attachments || a.role != Role::Original)
            .collect();
        if require_material
            && assets.is_empty()
            && item.links.is_empty()
            && item.documents.is_empty()
        {
            return Err(if only_attachments { "Add reference attachments or turn off Send only attachments before generating a caption" } else { "Attach an image or reference before generating a caption" }.into());
        }
        if assets.len() > 20 {
            return Err("Agent requests support up to 20 included images".into());
        }
        for asset in &assets {
            if !self.resolve(item, asset)?.is_file() {
                return Err(format!("Locate missing file: {}", asset.filename));
            }
        }
        for doc in &item.documents {
            if !document_path(&self.root, item, doc)?.is_file() {
                return Err(format!("Locate missing file: {}", doc.filename));
            }
        }
        Ok(assets)
    }
    pub fn agent_images(
        &self,
        item: &Item,
        only_attachments: bool,
        require_material: bool,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        self.agent_assets(item, only_attachments, require_material)?
            .into_iter()
            .enumerate()
            .map(|(n, asset)| {
                Ok((
                    format!("Image {}: {:?}", n + 1, asset.role),
                    agent_image(&self.resolve(item, asset)?)?,
                ))
            })
            .collect()
    }
    pub fn check(&self, ids: &[String]) -> Result<Vec<Missing>> {
        let mut missing = vec![];
        for item_id in ids {
            let item = self.load(item_id)?;
            self.index(&item)?;
            for a in &item.assets {
                if !self.resolve(&item, a)?.is_file() {
                    missing.push(Missing {
                        item_id: item.id.clone(),
                        title: item.title.clone(),
                        asset_id: a.id.clone(),
                        filename: a.filename.clone(),
                    });
                }
            }
            for d in &item.documents {
                if !document_path(&self.root, &item, d)?.is_file() {
                    missing.push(Missing {
                        item_id: item.id.clone(),
                        title: item.title.clone(),
                        asset_id: d.id.clone(),
                        filename: d.filename.clone(),
                    });
                }
            }
        }
        Ok(missing)
    }
    pub fn remove(&self, ids: &[String]) -> Result<()> {
        for item_id in ids {
            self.load(item_id)?;
        }
        for item_id in ids {
            let source = self.item_dir(item_id)?;
            let dest = self
                .root
                .join(".nabor/trash")
                .join(format!("{item_id}-{}", id()));
            fs::rename(source, dest).map_err(err)?;
            self.db
                .execute("DELETE FROM items WHERE id=?1", [item_id])
                .map_err(err)?;
        }
        Ok(())
    }
    pub fn export(&self, ids: &[String], parent: &Path) -> Result<PathBuf> {
        if ids.is_empty() {
            return Err("Select at least one item".into());
        }
        let missing = self.check(ids)?;
        if !missing.is_empty() {
            return Err(format!(
                "{} files are unavailable. Locate them or exclude their items before exporting.",
                missing.len()
            ));
        }
        let parent = parent.canonicalize().map_err(err)?;
        if parent.starts_with(&self.root) {
            return Err("Choose an export folder outside the collection".into());
        }
        let staging = tempfile::Builder::new()
            .prefix(".nabor-export-")
            .tempdir_in(&parent)
            .map_err(err)?;
        let mut exported = vec![];
        let mut package_version = VERSION;
        for item_id in ids {
            let mut item = self.load(item_id)?;
            package_version = package_version.max(item.schema_version);
            let original = item.clone();
            item.source_hash.clear();
            item.caption_history.clear();
            let dir = staging.path().join("items").join(item_id);
            fs::create_dir_all(dir.join("assets")).map_err(err)?;
            for a in &mut item.assets {
                let source = self.resolve(&original, a)?;
                let extension = image_extension(&source)?;
                let relative = format!("assets/{}.{}", a.id, extension);
                fs::copy(&source, dir.join(&relative)).map_err(err)?;
                a.path = relative;
                a.storage = Storage::Copy;
            }
            for d in &mut item.documents {
                let source = document_path(&self.root, &original, d)?;
                let bytes = document_bytes(&source)?;
                if hash(&bytes) != d.content_hash {
                    return Err(format!(
                        "{} changed. Re-extract its text before exporting.",
                        d.filename
                    ));
                }
                let relative = format!("assets/{}.{}", d.id, d.format);
                atomic_write(&dir.join(&relative), &bytes)?;
                d.path = relative;
                d.storage = Storage::Copy;
                d.content_hash = hash(&bytes);
            }
            atomic_write(&dir.join("caption.txt"), item.caption.as_bytes())?;
            atomic_write(
                &dir.join("item.json"),
                &serde_json::to_vec_pretty(&item).map_err(err)?,
            )?;
            exported.push(format!("items/{item_id}/item.json"));
        }
        let manifest = serde_json::json!({"schemaVersion":package_version,"format":"nabor-agent-package","items":exported});
        atomic_write(
            &staging.path().join("package.json"),
            &serde_json::to_vec_pretty(&manifest).map_err(err)?,
        )?;
        atomic_write(&staging.path().join("README.md"),b"# Nabor agent package\n\nRead package.json for the list of independent item manifests. Each item.json is authoritative and uses schemaVersion 1, 2, or 3. Schema 3 adds documents with id, filename, format, title, notes, role, storage, path, and contentHash; document paths are relative. Original PDF, DOCX, MD and TXT files are included unchanged. Extracted text is a rebuildable cache and is not included in this export. Read originals with appropriate tools; scanned PDFs may need OCR. Schema 2 adds website reference attachments in links: each has id, url, title, and notes. Website contents have not been fetched or included; do not claim to have read the URLs. Asset paths are relative to its item folder. original is the full-resolution image to use; caption-source is evidence containing caption text; reference provides context. caption is the final caption, entered by the user or generated by an agent. notes contains user instructions. Never apply one item's caption or references to another item. If caption is empty, generate it from this item's images and references using the instructions. Preserve existing captions. Do not invent unsupported details; explain missing source material when needed. All image assets are copied with original bytes intact. Website links retain their URLs and user-entered text. No outside paths or agent provider are required.\n")?;
        let destination = parent.join(format!("Nabor-export-{}-{}", now(), &id()[..8]));
        fs::rename(staging.path(), &destination).map_err(err)?;
        Ok(destination)
    }
    pub fn log(&self, code: &str) {
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join(".nabor/diagnostics.log"))
        {
            let _ = writeln!(f, "{} {}", now(), code);
        }
    }
}

pub fn cached_thumbnail(root: &Path, asset_id: &str) -> Option<PathBuf> {
    valid_id(asset_id).ok()?;
    ["png", "jpg"]
        .into_iter()
        .map(|ext| {
            root.join(".nabor/thumbnails")
                .join(format!("{asset_id}.{ext}"))
        })
        .find(|p| p.is_file() && no_symlink(p).is_ok())
}

// Called from the desktop's thumbnail worker, never from the UI thread.
pub fn thumbnail(root: &Path, item: &Item, asset: &Asset) -> Result<Option<PathBuf>> {
    use sha2::{Digest, Sha256};
    valid_id(&item.id)?;
    valid_id(&asset.id)?;
    let cache = root.join(".nabor/thumbnails");
    let stamp = cache.join(format!("{}.stamp", asset.id));
    let source = if asset.storage == Storage::Copy {
        safe_relative(&asset.path)?;
        let dir = root
            .join("items")
            .join(&item.id)
            .canonicalize()
            .map_err(err)?;
        let candidate = dir.join(&asset.path);
        if candidate.exists() && !candidate.canonicalize().map_err(err)?.starts_with(&dir) {
            return Err("Unsafe thumbnail path".into());
        }
        candidate
    } else {
        PathBuf::from(&asset.path)
    };
    if !source.is_file() {
        return Ok(cached_thumbnail(root, &asset.id));
    }
    let meta = fs::metadata(&source).map_err(err)?;
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "alpha-v2:{}:{}:{:?}",
                source.display(),
                meta.len(),
                meta.modified().ok()
            )
            .as_bytes()
        )
    );
    if fs::read_to_string(&stamp).unwrap_or_default() == fingerprint {
        if let Some(output) = cached_thumbnail(root, &asset.id) {
            return Ok(Some(output));
        }
    }
    let mut reader = image::ImageReader::open(&source)
        .map_err(err)?
        .with_guessed_format()
        .map_err(err)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().map_err(err)?;
    let image = decoded
        .thumbnail(decoded.width().min(480), decoded.height().min(480))
        .to_rgba8();
    let transparent = image.pixels().any(|p| p.0[3] < 255);
    let output = cache.join(format!(
        "{}.{}",
        asset.id,
        if transparent { "png" } else { "jpg" }
    ));
    let mut data = std::io::Cursor::new(vec![]);
    if transparent {
        image
            .write_to(&mut data, image::ImageFormat::Png)
            .map_err(err)?;
    } else {
        image::DynamicImage::ImageRgba8(image)
            .to_rgb8()
            .write_to(&mut data, image::ImageFormat::Jpeg)
            .map_err(err)?;
    }
    atomic_write(&output, &data.into_inner())?;
    atomic_write(&stamp, fingerprint.as_bytes())?;
    let _ = fs::remove_file(cache.join(format!(
        "{}.{}",
        asset.id,
        if transparent { "jpg" } else { "png" }
    )));
    Ok(Some(output))
}

pub fn encode_clipboard(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let image = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or("Invalid clipboard pixels")?;
    let mut cursor = std::io::Cursor::new(vec![]);
    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(err)?;
    Ok(cursor.into_inner())
}

pub const MAX_IMAGE_BYTES: usize = 50 * 1024 * 1024;
pub fn validated_image_extension(bytes: &[u8]) -> Result<&'static str> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("Image exceeds the 50 MB import limit".into());
    }
    let format =
        image::guess_format(bytes).map_err(|_| "The dropped content is not a supported image")?;
    let ext = match format {
        image::ImageFormat::Png => "png",
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::WebP => "webp",
        image::ImageFormat::Gif => "gif",
        image::ImageFormat::Bmp => "bmp",
        image::ImageFormat::Tiff => "tiff",
        _ => return Err("Supported image formats: PNG, JPEG, WebP, GIF, BMP, TIFF".into()),
    };
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|_| "Image data is incomplete, invalid, or too large to decode")?;
    Ok(ext)
}

/// Bounded image input for caption generation. Collection originals are untouched.
pub fn agent_image(path: &Path) -> Result<Vec<u8>> {
    let mut reader = image::ImageReader::open(path)
        .map_err(err)?
        .with_guessed_format()
        .map_err(err)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let source = reader
        .decode()
        .map_err(|_| "An image could not be decoded for caption generation")?
        .thumbnail(1600, 1600)
        .to_rgba8();
    // Composite transparent images on white rather than silently turning alpha black.
    let mut rgb = image::RgbImage::new(source.width(), source.height());
    for (x, y, pixel) in source.enumerate_pixels() {
        let a = u16::from(pixel[3]);
        rgb.put_pixel(
            x,
            y,
            image::Rgb(
                [0, 1, 2].map(|c| ((u16::from(pixel[c]) * a + 255 * (255 - a)) / 255) as u8),
            ),
        );
    }
    let mut bytes = std::io::Cursor::new(vec![]);
    rgb.write_to(&mut bytes, image::ImageFormat::Jpeg)
        .map_err(err)?;
    Ok(bytes.into_inner())
}
