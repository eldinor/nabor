# Nabor

A desktop collection manager for images, captions, and references. Built with Tauri 2, React, TypeScript, Rust, and SQLite. Collection management and export work offline. Caption generation optionally connects to your ChatGPT plan.

## Appearance

Choose **Appearance** above **Help** in the sidebar: **System** (default), **Light**, or **Dark**. The preference is saved on this computer. System follows operating-system appearance changes immediately; a manual choice overrides them. Switching keeps current edits and unsent questions intact.

Themes cover the workspace, inspectors, agent panel, menus, editors, and status messages. Images keep their colors; transparent areas use a checkerboard. Rebuildable thumbnails use PNG for transparency and JPEG for opaque images. Existing previews regenerate as needed; original files and exports are unchanged. Native dialogs and window chrome follow the operating system.

## ChatGPT connection (experimental)

In **Agent settings**, enable **Send only attachments** to exclude the main original image from captions, revisions, batch generation, and Ask agent. It is off by default and saved on this computer. Reference images, caption sources, extracted document text, website link details, and item text remain included. The 20-image limit counts only included images. Caption generation requires reference material when the main image is excluded; Ask agent can still discuss the item's text. Pause an active batch before changing the setting.

Open **Agent settings → Continue with ChatGPT**. Nabor opens the system browser and uses OpenAI's documented dynamic public-client registration, PKCE, and a temporary loopback callback. Authorize Nabor and return to the app. The account's available models appear in the model picker; choose one before generating.

Open an item with an empty caption and choose **Generate caption**. This explicitly sends that item's title, instructions, original image, reference images, extracted document text, and website attachment URLs/titles/notes to OpenAI using the signed-in ChatGPT plan. Nabor prepares bounded JPEG inputs up to 1600 pixels on the longer edge; collection originals stay unchanged. This integration supports at most 20 images per item. Website links are not fetched. Link-only items can use their user-entered titles and notes; a URL by itself does not provide the page's contents.

Generation returns an editable draft. Enter **Revision instructions** and choose **Revise with agent** to improve the current draft, including manual corrections, using the item's references. Failed requests retain the current text and instructions. Choose **Save caption** to accept the result, or discard it.

For an existing caption, **Revise with agent** opens the saved text without sending a request. The saved caption is preserved until you explicitly accept its replacement. **Earlier versions → Restore** restores a draft version in the editor. The last 20 previous saved captions are retained in `item.json`; **History** opens them even after restarting Nabor. Intermediate unsaved draft versions remain available only while that editor is open. Export packages contain the final caption, not revision history. Existing manifests without history open without migration. If the item manifest changes while generation runs, saving the stale result is refused. Incomplete/error responses never produce a saved caption.

Credentials are stored in Windows Credential Manager or macOS Keychain, not in the collection, export, browser storage, or logs. A stable installation host identifier and each account's registration are retained across sign-ins. Credentials are stored in short staged records to fit Windows credential size limits, with an atomic index switch for rotating tokens. Only one Nabor process can use the shared agent session at a time. Sign out attempts server revocation, clears local tokens, and retains the registration for later sign-in; the UI reports unconfirmed remote revocation.

This uses **ChatGPT plan usage**, not an API key or separate API billing. It never falls back to another billing path. Availability, supported models, and usage limits are controlled by OpenAI for the selected account/workspace. Plus usage is shared across supported apps rather than allocated separately to Nabor. Successful tests of this implementation do not establish eligibility for a particular account; that requires completing sign-in and a real request.

The integration follows the documentation for locally hosted clients and is not a declaration of an open-source license or a published commercial integration. See [registration](https://developers.openai.com/siwc/token-sharing-open-source/sign-in), [accounts and usage](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions), and [preview restrictions](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations). Commercial/remote distribution requires separately checking OpenAI's integration requirements.

## Batch captions

Set the shared prompt in **Collection settings → Caption instructions**. It applies to individual generation and batch runs; each item's Instructions take priority.

Use **Generate selected** for selected cards, including selections across pages, or **Collection actions (•••) → Generate entire collection** for every indexed item regardless of the current search, filter, or page. Preview shows eligible items and skips existing final captions, saved drafts, and unavailable inputs. Starting a run snapshots its common prompt, model, account, and item IDs.

The queue processes one item at a time. **Pause** and **Cancel** finish the current request and retain its draft; cancel stops pending items. Authentication, usage, and connectivity failures pause processing. Resume explicitly with the same account; no billing fallback is used. **Retry failed** retries item failures after pending work finishes. Keep Nabor open while processing; an interrupted run reopens paused and requires Resume. Completed persisted drafts are not generated again.

Open **Caption drafts** to review, manually edit, revise with another prompt, keep, discard, or save each result. **Save all drafts** accepts eligible drafts and reports conflicts. Final captions remain empty until accepted; exports include only accepted captions. Source changes or a manually filled final caption block stale draft acceptance. Resolve the conflict by retaining the desired text manually or discarding the draft and generating again.

Drafts are durable files under `captions/drafts/`; `captions/queue.json` stores the run. Both belong in collection backups. SQLite's draft index is rebuildable. Manually kept draft text survives restart; intermediate editor history is session-only. A request interrupted before its completed draft is written may need to run again and consume plan usage again.

## Ask agent

Open an item and choose **Ask agent**. The panel shows which item and how many images are included. Sending uses the selected ChatGPT account/model, the current item's images, title, saved caption, instructions, source URLs, and recent completed conversation. It supports follow-up questions as well as text-only blank items. Website contents are not fetched, and replies do not automatically modify captions or instructions.

**Use as caption** opens the reply in the caption review editor; accepting it explicitly saves/replaces the final caption. **Add to notes** appends the reply to the existing Instructions field. **Copy** copies the full reply. Saved conversation text is displayed as plain text. Up to 20 recent completed exchanges, bounded to 60,000 text bytes, accompany each request; older messages stay saved locally. Current item text is bounded to 200,000 bytes and images use the same 20-image/bounded JPEG preparation as captions.

Questions are saved before inference. Failed or interrupted questions retain their text, and the latest unanswered question offers **Retry question**. Nothing retries automatically; repeating an interrupted request can consume plan usage again. Close/reopen the panel to reload external changes. The conversation file is `items/<item-id>/conversation.json`, separate from the item manifest, backed up and moved with its item. Conversations are excluded from agent exports. **Clear conversation** requires confirmation and leaves captions, instructions, and files intact. Invalid/future conversation schemas are preserved and reported when opened. Conversations are limited to 100 turns and a 2 MB file; clear a full conversation before continuing.

## Run

For macOS setup, build commands, and native checks, see [MAC_NOTES.md](MAC_NOTES.md).

Install Node.js 22.20+ and stable Rust. On Windows, install the Microsoft C++ build tools and WebView2; on macOS, install Xcode Command Line Tools. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
npm ci
npm run tauri dev
```

`npm run dev` opens the web interface for preview; native file dialogs, clipboard integration, and filesystem operations require the desktop app.

This workspace also has a local Rust toolchain in `.tools` (ignored). To use it from PowerShell:

```powershell
$env:CARGO_HOME = Join-Path (Get-Location) '.tools/cargo'
$env:RUSTUP_HOME = Join-Path (Get-Location) '.tools/rustup'
$env:Path = "$env:CARGO_HOME/bin;$env:Path"
npm run tauri dev
```

## Workflow

The interface uses the system font (SF on Mac, Segoe UI on Windows), 15 px content text, 14 px controls, and 12–13 px metadata/labels. The item inspector and Ask agent panel can be resized with their divider; arrow keys adjust a focused divider and double-click resets its width. Widths are remembered on this computer. **Cmd/Ctrl+F** focuses collection search outside modal dialogs.

Attachment primary actions remain visible; their **•••** menus contain edit, move, locate, copy, re-extract, and removal actions as applicable. Ask agent is a docked panel beside the inbox. Selecting another item switches the panel's scope; unsent questions are retained per item for the current session. Saved conversation history remains durable in the collection. **View image** opens the included image without closing the conversation.

Use **Add files** or an item's **Attach** action for images and PDF, DOCX, Markdown (`.md`), and text (`.txt`) references. The current Copy files / Link to files choice applies to both images and documents. Dropped document bytes are copied; use the file picker to keep a document in its original location. Inbox imports create one item per file; dropping on an item adds a reference. Documents cannot replace the original image.

Document attachments offer **Open in default app**, **View text**, **Edit details** (title, notes, and reference/caption-source role), **Re-extract text**, **Move to item**, **Locate file** for linked files, and **Remove document**. Removal drops the relationship while retaining managed and linked originals. Document filenames, titles, and notes are searchable; extracted bodies are available in View text and agent context.

PDF text and DOCX paragraphs/table-cell text are extracted locally; embedded document images, OCR, Word formatting, and external resources are not processed. Markdown stays plain text and no embedded links are followed. UTF-8 and BOM-marked UTF-16 text are supported. Scanned/empty, damaged, protected, or unsupported documents show an extraction status instead of silently supplying missing text to the agent. Older `.doc`, `.docm`, RTF, and other document types remain unsupported.

Extraction runs through a serialized background helper process, with a 20-second timeout. Originals are limited to 20 MB each and ten documents per item. Extracted previews are bounded to 128,000 UTF-8 bytes; truncation is shown. Agent requests include at most 100,000 bytes of document body text across the attached documents, with explicit truncation markers, plus their titles, roles, and notes. Raw original files are never uploaded by this implementation. Missing/error/empty/changed document text blocks an agent request until repaired, re-extracted, or removed.

The source SHA-256 is stored with each document. Linked files are checked before using cached text. **Re-extract text** acknowledges a changed source, updates its fingerprint, and invalidates older caption drafts through the item manifest hash. Missing files retain cached previews; Locate file requires an explicit same-format replacement. Generated-caption acceptance and export also refuse unacknowledged document changes. Cached extraction is stored in `.nabor/document-text/` and can be rebuilt from originals. Exports contain byte-exact document files and metadata, not extraction caches.

Use **View → Masonry** in the inbox toolbar to show mixed portrait, landscape, and square previews without cropping. Cards retain equal column widths with variable heights; exceptionally tall previews fit within a 720 px image area. **Grid** retains fixed-size cropped previews. The view preference is saved on this computer. Both views use cached thumbnails, virtualization, and pagination; full-resolution images load only when explicitly opened.

1. Create a collection in an empty folder, or open an existing collection.
2. Drop images into the inbox or use **Add files**. Each image becomes a separate item.
   Browser image drops download the image into the collection and retain its image URL. Drop on a card or the active item panel to attach it: the first image becomes the original when none exists, and subsequent images become references. The attachment's **Open in browser** action opens its saved image URL in the default browser.
3. Open an item. Type/paste its caption, attach screenshots, and assign each attachment as original, caption source, or reference. Dropping on the active item panel attaches files there; dropping on the inbox creates separate items. Paste outside a text field into the active item, or use **Paste into this item**. Without an active item, pasted content creates one.
4. Choose **Copy files** or **Link to files** before importing. The check button saves that choice as the collection default. Clipboard images are always copied.
5. Search titles, typed captions, notes, and filenames. Use filters for missing originals, captions, or files. Refresh reconciles external edits and checks all files.
6. Select cards, then export. Choose a destination outside the collection. Linked files are copied into the package. Missing files pause export; locate them in their item or deselect their items before trying again.

Only one Nabor process may open a collection at a time. Text edits save after a short pause; failures retain the draft with retry/discard controls. External edits are detected with a content hash and revision to avoid silent overwrites.

Browser imports support direct public HTTP(S) image URLs, Chrome-style dragged image markup, and dropped image files. They preserve downloaded/file bytes, validate that the content is an image, and limit each image to 50 MB (20 images per drop). Downloads use no browser cookies or credentials; a site requiring authentication or blocking downloads may need **Save image as** in the browser followed by file import. Redirect destinations are checked individually. AVIF/SVG and browser-private blob URLs without transferable image bytes are not supported. A dragged preview may be lower resolution than the website's original; Nabor downloads the actual URL supplied by the browser.

Filesystem image drops still copy file bytes. If a drop includes a local file URI, the selected copy/link mode is used; otherwise use **Add files** or **Attach** to link a local file by its real path. The app reports when a byte-only file drop is copied while link mode is selected. Web images are always managed copies, regardless of that setting. Normal text drops in text fields remain text.

PNG, JPEG, WebP, GIF, BMP, and TIFF are supported. Thumbnails show the first frame/page of animated or multipage images; exports preserve all original bytes. HEIC, SVG, OCR, cloud sync, global shortcuts, and in-app AI transformations are outside this release.

## Files and portability

Website references are added through **Attachments → Add link** in an open item. Enter an HTTP(S) URL and optional title and notes. Each link offers **Open in browser**, **Copy link**, **Edit link**, **Move to item**, and **Remove link**. Titles, URLs, and notes are searchable and included in individual/batch caption generation, Ask agent, and exports. Saving, refreshing, and exporting do not fetch websites or check their availability. Removal deletes only the item's link record. Website attachments remain separate from image sources and linked local files.

Each item supports up to 50 website links, bounded to 64,000 bytes of combined link details; each title supports 512 bytes and each notes field 8,000 bytes. URLs must use HTTP(S), have a host, and contain no embedded username/password or control characters. Local/intranet URLs can be saved and explicitly opened in the user's browser; they are never requested by the app.

```text
collection/
  collection.json                 # schema version and default import mode
  items/<item-id>/
    item.json                     # authoritative relationships and text
    caption.txt                   # generated convenience copy
    assets/<asset-id>.<extension>  # copied files; relative paths
  captions/
    drafts/<item-id>.json          # unaccepted generated captions
    queue.json                    # resumable batch state
  .nabor/
    index.sqlite                  # rebuildable SQLite index
    thumbnails/                   # rebuildable JPEG/PNG previews and fingerprints
    document-text/                # rebuildable extracted document text
    collection.lock
    diagnostics.log               # timestamps + operation codes, no user contents
    trash/                        # removed item folders
```

Item manifests support schema versions 1, 2, and 3. They include stable item/asset UUIDs, title, caption, instruction notes, timestamps, revision, and image asset roles (`original`, `caption-source`, `reference`). Version 2 adds a `links` array of website reference records (`id`, `url`, `title`, `notes`). Version 3 adds `documents` records with ID, filename, format, storage mode, relative/absolute path, title, notes, role, and content hash. Adding the first new attachment type promotes only its item after atomically backing up the previous manifest as `item.v<version>.<hash>.backup.json`. If the backup cannot be written, the edit is refused. Removing attachments retains the newer version. Older Nabor builds report newer schemas as unsupported instead of editing them and losing attachments. Unchanged legacy items remain version 1; the collection marker remains version 1.

Each image asset records its original filename and storage mode (`copy` or `link`). A copied path is relative to the item folder; a linked local path is absolute. There is at most one original image per item. Website links cannot be originals. `sourceHash` returned through IPC is a concurrency token, not part of saved manifests; link changes invalidate earlier caption drafts like other item changes.

Manifests and originals are durable; caches are not. Opening or refreshing rebuilds the index from valid manifests. Invalid/future manifests are reported without being rewritten. Migrations back up original manifests before writing a new schema. Edit `item.json` rather than `caption.txt` when changing text outside Nabor. SQLite's search is case-insensitive for ASCII; non-ASCII text is matched as entered.

Moving a collection preserves copied assets. External links may need reconnection on another computer. Missing linked files retain cached thumbnails. **Locate file** reconnects a file the user explicitly selects; Nabor does not guess.

**Detach** removes the relationship but retains any managed file in its item folder for recovery. **Remove** moves the whole item folder into `.nabor/trash`; linked sources are never moved or deleted. To recover a removed item, close Nabor, move the folder back under `items/`, rename it to the ID inside its `item.json`, then reopen. A move between items saves the destination first; an interrupted move can leave a duplicate relationship rather than lose an attachment. Unreferenced managed files are not exported. Partial imports may leave unreferenced files; manifests remain authoritative.

Back up the entire collection folder. File durability assumes a local filesystem with working atomic rename; cloud-synced/network folders are not validated for concurrent access.

## Agent export contract

An export is created under a temporary directory, then renamed to `Nabor-export-<timestamp>-<id>` only after completion. It contains:

- `package.json`: `{ "schemaVersion": 1, "format": "nabor-agent-package", "items": ["items/<id>/item.json"] }`; the package version is the highest exported item schema (1, 2, or 3).
- `items/<id>/item.json`, `caption.txt`, and all referenced assets, including copies of linked files.
- `README.md`: instructions explaining item boundaries, attachment roles, and missing context.

Each asset path is relative to its item folder. The agent reads caption and notes from that item's manifest and never borrows context from another item. Original bytes and resolution remain unchanged. No provider-specific API is involved. Interrupted exports are never exposed as finished packages; an abrupt process termination may leave a hidden `.nabor-export-*` staging folder for manual removal.

Website references remain in each item's `links` array with their IDs, URLs, titles, and user notes. They do not create downloaded files. The package guide explicitly explains that website contents have not been fetched and must not be treated as included evidence. Migration backup files are not exported.

Collection operations use typed, tagged IPC requests, `request({ payload: { action, ...args } })`; agent and caption queue operations use `agent_request` and `batch_request`. Filesystem mutations are serialized on a blocking worker. Thumbnail decoding runs on a separate worker and publishes `thumbnail-ready` events. File previews are granted individually to Tauri's asset protocol; broad filesystem access is not granted to the webview.

## Checks

```sh
npm run build
npm run test:core
npm run test:agent
npx playwright install chromium
npm test
```

Rust integration tests exercise real files, SQLite, copy/link imports, 20 image/caption pairs, attachment moves and roles, clipboard PNG encoding, missing-link recovery, collection transfer, cache rebuilding/corruption, external edits, invalid schema handling, atomic write failure, traversal rejection, and byte-exact exports. A 10,000-manifest test asserts page queries and search each finish within one second.

Playwright tests use an explicit development-only IPC fixture. They cover editing and reopen, roles, failed-save recovery, missing-file export blocking, selection/removal, moves, pagination, and virtualization. A separate Windows smoke harness also verifies real native IPC, scoped image loading, and exports. Neither substitutes for native file dialog/clipboard testing.

See [validation and release checklist](docs/VALIDATION.md) for measured results and remaining platform gates.

## Build and release

```sh
npm run tauri build
```

Installers are written to `src-tauri/target/release/bundle`. The desktop workflow tests and builds separately on Windows x64, Apple Silicon, and Intel macOS. It uploads CI artifacts without publishing a public release. These native runners are required; a Windows build cannot verify macOS behavior.

Unsigned builds run by default. To sign, manually dispatch the workflow with `sign=true` and configure these GitHub secrets:

- Windows: `WINDOWS_CERTIFICATE` (base64 PFX), `WINDOWS_CERTIFICATE_PASSWORD`.
- Mac: `APPLE_CERTIFICATE` (base64 Developer ID Application P12), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`.

Signing must be tested against the certificate provider's requirements; hardware-backed Windows certificates may need a custom signing command. See the official [Windows signing](https://v2.tauri.app/distribute/sign/windows/) and [Mac signing and notarization](https://v2.tauri.app/distribute/sign/macos/) documentation. Credentials are not included in this repository.
