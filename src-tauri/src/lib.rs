use nabor_core::{Collection, Item, Role, Storage};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{mpsc, Mutex},
};
use tauri::{Emitter, Manager};
mod agent;
mod batch;
mod talk;
mod web_import;
fn batch_active(app: &tauri::AppHandle) -> bool {
    app.state::<batch::Runtime>()
        .active
        .load(std::sync::atomic::Ordering::SeqCst)
}
use base64::Engine;

struct ThumbnailJob {
    root: PathBuf,
    item: Item,
    asset: nabor_core::Asset,
}
struct State {
    collection: Mutex<Option<Collection>>,
    thumbnails: mpsc::Sender<ThumbnailJob>,
}

#[derive(Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Request {
    Open {
        path: String,
        create: bool,
    },
    Refresh,
    Info,
    List {
        query: String,
        filter: String,
        offset: usize,
        limit: usize,
    },
    Get {
        id: String,
    },
    Create,
    Save {
        item: Item,
    },
    Import {
        item_id: Option<String>,
        paths: Vec<String>,
        storage: Storage,
    },
    ImportImage {
        item_id: Option<String>,
        filename: String,
        bytes_base64: String,
        source_url: Option<String>,
    },
    ImportWebImage {
        item_id: Option<String>,
        url: String,
    },
    OpenSource {
        item_id: String,
        asset_id: String,
    },
    Clipboard {
        item_id: Option<String>,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    Role {
        item_id: String,
        asset_id: String,
        role: Role,
    },
    Detach {
        item_id: String,
        asset_id: String,
    },
    Move {
        from: String,
        to: String,
        asset_id: String,
    },
    Reconnect {
        item_id: String,
        asset_id: String,
        path: String,
    },
    Settings {
        storage: Storage,
    },
    CaptionInstructions {
        prompt: String,
    },
    Check {
        ids: Vec<String>,
    },
    Remove {
        ids: Vec<String>,
    },
    Export {
        ids: Vec<String>,
        parent: String,
    },
    Previews {
        ids: Vec<String>,
    },
    Original {
        item_id: String,
        asset_id: String,
    },
}

fn perform(app: tauri::AppHandle, payload: Request) -> Result<Value, String> {
    let state = app.state::<State>();
    let mut guard = state
        .collection
        .lock()
        .map_err(|_| "Collection lock failed")?;
    if let Request::Open { path, create } = payload {
        if app
            .state::<batch::Runtime>()
            .active
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(
                "Pause generation and wait for the current item before switching collections"
                    .into(),
            );
        }
        if let Some(c) = guard.as_mut() {
            if std::fs::canonicalize(&path).ok().as_ref() == Some(&c.root) {
                let report = c.refresh()?;
                return Ok(
                    json!({"root":c.root.to_string_lossy(),"settings":c.settings,"report":report}),
                );
            }
        }
        let (collection, report) = Collection::open(&PathBuf::from(path), create)?;
        let info = json!({"root":collection.root.to_string_lossy(),"settings":collection.settings,"report":report});
        *guard = Some(collection);
        return Ok(info);
    }
    let c = guard.as_mut().ok_or("Open a collection first")?;
    let value = match payload {
        Request::Open { .. } => unreachable!(),
        Request::Info => json!({"root":c.root.to_string_lossy(),"settings":c.settings}),
        Request::Refresh => json!(c.refresh()?),
        Request::List {
            query,
            filter,
            offset,
            limit,
        } => json!(c.list(&query, &filter, offset, limit)?),
        Request::Get { id } => {
            c.check(&[id.clone()])?;
            json!(c.load(&id)?)
        }
        Request::Create => json!(c.create_item()?),
        Request::Save { mut item } => {
            c.update(&mut item)?;
            json!(item)
        }
        Request::Import {
            item_id,
            paths,
            storage,
        } => json!(c.import(item_id.as_deref(), &paths, storage)?),
        Request::ImportImage {
            item_id,
            filename,
            bytes_base64,
            source_url,
        } => {
            if bytes_base64.len() > ((nabor_core::MAX_IMAGE_BYTES + 2) / 3) * 4 {
                return Err("Image exceeds the 50 MB import limit".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(bytes_base64)
                .map_err(|_| "Invalid dropped image data")?;
            json!(c.import_image(item_id.as_deref(), &filename, &bytes, source_url)?)
        }
        Request::ImportWebImage { item_id, url } => {
            if let Some(id) = &item_id {
                c.load(id)?;
            }
            let source = web_import::validate_url(&url)?.to_string();
            let (filename, bytes) = web_import::download(&source)?;
            json!(c.import_image(item_id.as_deref(), &filename, &bytes, Some(source))?)
        }
        Request::OpenSource { item_id, asset_id } => {
            let item = c.load(&item_id)?;
            let asset = item
                .assets
                .iter()
                .find(|a| a.id == asset_id)
                .ok_or("Attachment not found")?;
            let source = web_import::validate_url(
                asset
                    .source_url
                    .as_deref()
                    .ok_or("No source URL is available")?,
            )?;
            open::that(source.as_str()).map_err(|_| "Cannot open the source website")?;
            Value::Null
        }
        Request::Clipboard {
            item_id,
            rgba,
            width,
            height,
        } => {
            if width == 0
                || height == 0
                || u64::from(width) * u64::from(height) * 4 != rgba.len() as u64
                || rgba.len() > 256 * 1024 * 1024
            {
                return Err("Clipboard image is invalid or larger than 256 MB".into());
            }
            let image = tauri::image::Image::new_owned(rgba, width, height);
            // Encode raw clipboard pixels through the core's image encoder.
            let png = nabor_core::encode_clipboard(image.rgba(), width, height)?;
            json!(c.clipboard(item_id.as_deref(), &png)?)
        }
        Request::Role {
            item_id,
            asset_id,
            role,
        } => json!(c.set_role(&item_id, &asset_id, role)?),
        Request::Detach { item_id, asset_id } => json!(c.detach(&item_id, &asset_id)?),
        Request::Move { from, to, asset_id } => {
            c.move_asset(&from, &to, &asset_id)?;
            Value::Null
        }
        Request::Reconnect {
            item_id,
            asset_id,
            path,
        } => json!(c.reconnect(&item_id, &asset_id, &PathBuf::from(path))?),
        Request::Settings { storage } => {
            c.set_default(storage)?;
            json!(c.settings)
        }
        Request::CaptionInstructions { prompt } => {
            c.set_caption_instructions(prompt)?;
            json!(c.settings)
        }
        Request::Check { ids } => json!(c.check(&ids)?),
        Request::Remove { ids } => {
            c.remove(&ids)?;
            Value::Null
        }
        Request::Export { ids, parent } => {
            json!(c.export(&ids, &PathBuf::from(parent))?.to_string_lossy())
        }
        Request::Previews { ids } => {
            let mut previews = serde_json::Map::new();
            for item_id in ids.into_iter().take(200) {
                let item = c.load(&item_id)?;
                for asset in &item.assets {
                    let path = c
                        .root
                        .join(".nabor/thumbnails")
                        .join(format!("{}.jpg", asset.id));
                    if path.exists() {
                        app.asset_protocol_scope()
                            .allow_file(&path)
                            .map_err(|e| e.to_string())?;
                        previews.insert(asset.id.clone(), json!(path.to_string_lossy()));
                    }
                    let _ = state.thumbnails.send(ThumbnailJob {
                        root: c.root.clone(),
                        item: item.clone(),
                        asset: asset.clone(),
                    });
                }
            }
            Value::Object(previews)
        }
        Request::Original { item_id, asset_id } => {
            let item = c.load(&item_id)?;
            let asset = item
                .assets
                .iter()
                .find(|a| a.id == asset_id)
                .ok_or("Attachment not found")?;
            let path = c
                .resolve(&item, asset)?
                .canonicalize()
                .map_err(|e| e.to_string())?;
            app.asset_protocol_scope()
                .allow_file(&path)
                .map_err(|e| e.to_string())?;
            json!(path.to_string_lossy())
        }
    };
    Ok(value)
}

#[tauri::command]
async fn request(app: tauri::AppHandle, payload: Request) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = perform(app.clone(), payload);
        if result.is_err() {
            if let Ok(guard) = app.state::<State>().collection.lock() {
                if let Some(c) = guard.as_ref() {
                    c.log("operation-failed");
                }
            }
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            app.manage(agent::AgentState::default());
            app.manage(batch::Runtime::default());
            let (sender, receiver) = mpsc::channel::<ThumbnailJob>();
            app.manage(State {
                collection: Mutex::new(None),
                thumbnails: sender,
            });
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                for job in receiver {
                    if let Ok(Some(path)) = nabor_core::thumbnail(&job.root, &job.item, &job.asset)
                    {
                        if handle.asset_protocol_scope().allow_file(&path).is_ok() {
                            let _ = handle.emit(
                                "thumbnail-ready",
                                json!({"id":job.asset.id,"path":path.to_string_lossy()}),
                            );
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            request,
            agent::agent_request,
            batch::batch_request,
            talk::talk_request
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Nabor");
}
