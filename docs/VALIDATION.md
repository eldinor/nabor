# Validation and release gates

## Local environment

Measured on Windows 10 build 19044, x64, Intel Core i7-11800H @ 2.30 GHz, 16 GB RAM, local collection directory. Node.js 22.20.0; Rust stable 1.99.1. Date: 2026-11-11.

The 10,000-item fixture used small real manifests and short captions, with cached SQLite queries. Seeding and rebuilding took 7.01 seconds, first-page query 3.63 ms, and text search 6.28 ms. These measurements cover metadata queries, not HD image decoding, filesystem import, initial collection scan, or a complete rendering frame. Thumbnails are generated lazily by a worker; the grid virtualizes the current 100-item page.

## Automated checks

- Rust: 11 real-filesystem integration tests, including 10,000 manifests.
- UI: six Playwright scenarios with development-only mocked IPC; production excludes the fixture bridge.
- TypeScript and production web build.
- Native Rust shell compilation.
- Hidden Windows WebView2 smoke test with real Rust IPC: collection creation, copied and linked imports, caption editing through React, scoped asset-protocol image loading, byte-exact export, broken-link detection/reconnection, and linked-source preservation after removal. No filesystem or IPC fixture was substituted in this test. Native file dialogs and system clipboard capture were not exercised.
- Optimized unsigned Windows NSIS installer built successfully (approximately 3.35 MiB).
- npm dependency audit: no reported vulnerabilities.

The Windows smoke harness requires a dedicated debug build; its configuration hides the window and enables a loopback debugging endpoint. It is never used for release builds:

```sh
npm run tauri -- build --debug --no-bundle --config scripts/tauri.smoke.conf.json
node scripts/native-smoke.mjs
```

Test artifacts and screenshots are written under `test-results/`. The harness leaves fixtures for inspection and terminates its app process afterward. Do not run another Playwright suite concurrently, since Playwright clears its output folder.

## Native manual acceptance checklist

### Attachment-only agent input (0.8.2)

On Windows, all 61 Rust tests and 35 UI scenarios have passed. New filesystem checks prove the main image's bytes are excluded while references are retained, a missing excluded original does not block generation, missing included references still block it, image limits count only included files, caption-source role changes remain effective, and batch preview eligibility matches the request policy. Account-store tests cover legacy defaults, preference persistence after reopening, and rollback on failed saves. The UI check covers changing, reopening, resetting, and reporting failed preference saves; a cold development-server navigation needed a longer test timeout. The appearance keyboard check now waits for the menu's initial focus before sending navigation keys.

The shared input selector serves individual captions, revisions, batch generation, and item questions. The setting defaults off, is saved locally with agent preferences, and cannot change during an active batch. Existing unaccepted drafts are retained. No live inference is needed for these checks.

The native Windows smoke test passed with real IPC, image loading, document extraction, batch previews, broken-link repair, and byte-exact exports. It did not change the user's agent preferences or request inference. The unsigned 0.8.2 Windows NSIS installer was built successfully; macOS runtime checks remain pending.

### Appearance (0.8.0)

On Windows, 34 UI scenarios have passed, and all 56 Rust tests pass: 40 core/filesystem, 9 agent, and 7 desktop backend. Appearance checks cover the System default, live operating-system changes, saved manual overrides after reload, keyboard-accessible radio menu choices, narrow-window access, retained caption edits and unsent questions, readable error states, and unfiltered image previews. Thumbnail tests verify transparency, automatic rebuilding of legacy JPEG caches, opaque JPEG output, removal of obsolete cache formats, retained previews for missing originals, and unchanged original bytes.

The hidden native Windows WebView2 smoke test passed with actual Rust IPC and filesystem operations. It switched to Dark, checked the workspace palette, exercised conversation and caption review, displayed extracted document text, and passed existing copy/link, missing-file repair, image import, and byte-exact export checks. No agent inference was requested. Dark inspector, conversation, and document views were inspected visually. macOS runtime appearance, native dialogs, installer upgrade, and signed-release checks still require their target environments.

### Batch captions (0.3.0)

On Windows, 17 Playwright scenarios and 36 Rust tests pass (22 collection/core, 8 agent, 6 desktop backend). Coverage includes collection-wide queries beyond the visible page, selected IDs across pages, common prompt precedence, skipping saved captions and drafts, persistent draft revisions, interrupted-run recovery, pause/resume/cancel, retrying item failures, pausing on usage/authentication errors, and refusing stale draft acceptance. Exports exclude unaccepted drafts.

The hidden native Windows WebView smoke harness additionally verified the collection prompt and batch previews through real IPC, indexed a durable test draft, kept a manual edit, and accepted it through Save all drafts. It also passed its existing image-drop, linked-file, revision-history, and byte-exact export checks. This run skipped agent settings and made no inference requests. Live batch generation against the user's plan, macOS behavior, and signed installer acceptance remain unverified.

### Item conversations (0.4.0)

On Windows, 20 Playwright scenarios and 42 Rust tests pass (26 collection/core, 9 agent, 7 desktop backend). New checks cover per-item conversation persistence and portability, explicit retry after interruption or failure, cross-item isolation, conversation revision conflicts, schema preservation, bounded chronological history, and Responses input roles. UI tests verify follow-ups, copy, note appending, caption review before replacement, clearing only conversation content, and retry without duplicate turns. Discussing an item leaves its manifest unchanged. Export excludes conversations.

The hidden Windows WebView smoke test also passed real conversation IPC: reopening a saved fixture reply, appending it to existing instructions, opening it for caption review and discarding without replacing the final caption, and exporting without conversation content. It repeated existing filesystem, queue/draft, and byte-exact export checks. Screenshot: `test-results/native-conversation.png`.

These checks use fixture responses and make no inference requests. Live conversation requests, native clipboard copying, macOS behavior, and signed installer acceptance remain unverified.

### Inbox masonry (0.4.1)

All 23 UI scenarios pass. New coverage verifies portrait/landscape proportions, resize when opening the item tray, view preference after reload, switching back to the fixed grid, missing/broken previews, and virtualized scrolling, paging, selection, and search in a 10,000-item fixture. Rendered cards remain bounded and do not overlap. The frontend production build passes; the Rust implementation is unchanged.

The Windows native WebView smoke check also passes with real cached square and landscape images. It checks their displayed aspect ratios in masonry, switches back to grid, and repeats the filesystem, caption, draft, conversation, and export checks. Screenshot: `test-results/native-masonry.png`. The harness now waits for import controls to become enabled before dispatching synthetic drops, avoiding a race with the previous action. macOS native verification remains outstanding.

### Website attachments (0.5.0)

All 26 UI scenarios pass, including adding/reopening/editing/removing links, opening the correct stored link through IPC, clipboard copying, searching URL/title/notes, retaining a failed save, and moving a website reference between items. All 48 Rust tests pass across the existing suites and six new website tests. Coverage includes legacy-item promotion with an exact backup, failure to create a backup, stale edit/removal rejection, unsafe URL rejection, text bounds, collection transfer, index rebuilding, link-only batch eligibility, reference context, moves, and versioned exports preserving original image bytes.

The Windows native WebView smoke test also passes: adding a link through the UI, reopening the item, retaining its prior caption, creating the version 1 backup, and exporting the link and notes with schema version 2. Screenshot: `test-results/native-links.png`. The harness uses its own WebView profile so it can run alongside an installed app. It does not fetch attached websites, launch the external browser, or consume ChatGPT usage. Browser-launch behavior, macOS verification, and signing remain manual release gates.

### Document attachments (0.6.0)

All 29 UI scenarios and 54 Rust tests pass. New document checks cover PDF text, DOCX paragraph/entity/table text, Markdown, UTF-8/UTF-16 text, copy/link import, mixed image/document imports, cached/missing/changed sources, explicit reconnection, moves, details/roles, removal without deleting linked originals, schema 1/2 backups before version 3 promotion, cache rebuilding, source conflicts at caption acceptance/export, and byte-exact document exports. Empty/scanned-style and damaged PDF fixtures report extraction status; large text is explicitly truncated. The UI regression also verifies pending manual caption edits survive document detail changes.

The Windows WebView smoke harness passed real document IPC and helper-process extraction for PDF, DOCX, MD, and TXT. It verified original-byte exports, schema version 3, visible extracted-text preview, and empty/damaged PDF status. A separate check, `node scripts/document-worker-smoke.mjs`, passed all four formats against the release GUI-subsystem executable through its CLI helper entry point with redirected output and no visible window. Tests use a separate collection and do not consume ChatGPT usage. The helper has a 20-second process timeout and hides its Windows console.

Live document-backed ChatGPT inference, opening originals in external applications, macOS native behavior, and signed installer acceptance remain unverified. OCR, embedded image analysis, legacy DOC/RTF, and Word layout fidelity are outside this release. The agent receives extracted text rather than raw document uploads; previews are capped at 128,000 UTF-8 bytes and request body text at 100,000 bytes across document references, with truncation markers.

### Desktop interface (0.7.0)

All 32 UI scenarios pass, including existing capture, captions, revisions, batch queues, links, documents, and export fixtures. New checks assert 15 px caption/conversation text and 14 px navigation, persistent inspector resizing by pointer and keyboard, narrow-window layout without horizontal overflow, menu keyboard navigation/Escape/outside dismissal, collection-search shortcut, non-modal agent layout, included-image preview, and retention of unsent questions when switching items. A menu issue caused by scrolling near the inspector footer was corrected and its workflow checks pass.

Windows native WebView smoke checks pass through real IPC with the redesigned controls, including save states, revision history, draft acceptance, website attachments, document helper extraction, masonry ratios, and byte-exact exports. Screenshots are generated as `test-results/native-links.png` and `test-results/native-conversation.png`. Rust collection/agent behavior and file formats are unchanged in this release; the Rust suites were not rerun for styling changes. SF font rendering and native macOS acceptance still require a Mac.

Record results separately on Windows, macOS Apple Silicon, and macOS Intel. An unchecked gate remains unverified.

- [ ] Install and launch the packaged app, then uninstall without removing user collections.
- [ ] Create/open a collection using the native folder dialog.
- [ ] Import copied and linked images using the native file dialog.
- [ ] Drop 20 originals into the inbox; attach their matching caption screenshots to the correct trays.
- [ ] Paste a native screenshot and plain text; verify normal paste still works in text fields.
- [ ] Restart and verify captions, roles, and references.
- [ ] Move an attachment to another item; verify source and destination.
- [ ] Move a linked file, verify the retained preview, locate it, and export.
- [ ] Export selected copied and linked items; inspect original bytes and relative paths.
- [ ] Cancel dialogs, deny folder access, and simulate a read-only save; verify draft recovery.
- [ ] Exercise Unicode paths, external drives, drive disconnection, and moved collections.
- [ ] Edit an item manifest externally; verify conflict handling and refresh.
- [ ] Confirm missing files block export until located or their items are deselected.
- [ ] Test 10,000 items interactively, including thumbnail generation, search, and pagination.
- [ ] Validate signed installer signatures; verify Mac notarization/stapling and launch on a clean machine.

## Experimental ChatGPT connection

Version 0.2.0 builds as a Windows NSIS installer. All 13 core tests, 7 agent tests, and 8 UI scenarios passed. Agent tests cover PKCE/host reuse, callback state and account binding, signed ID-token validation (including signature tampering, issuer, audience, nonce, timestamps), token rotation, model catalog handling, and completed-stream handling. UI fixtures exercise connecting, selecting an account model, reviewing/editing a generated draft, preserving nonempty captions, discarding drafts, and displaying eligibility errors. Core tests cover stale-draft rejection and bounded image preparation without changing originals.

The native Windows smoke test opened Agent settings, read account status from the OS credential store, confirmed that IPC does not expose access tokens, and completed the existing real-filesystem/export checks. This automated test did not authorize an account or consume its plan. During the interactive check on 2026-11-12, the user reported completing sign-in and seeing the account's model list in Nabor. This confirms live authorization and model-catalog retrieval for that account; actual caption inference and credentials surviving a restart remain to be checked. The implementation follows the official preview and uses no API-key billing fallback.

The user subsequently reported that caption generation with GPT-6.1 Sol works. Version 0.2.1 adds manual/agent revision of the current draft, explicit replacement of saved captions, session draft restoration, and 20 previous saved caption versions. All 15 core tests, the backend revision-context test, and 10 UI scenarios passed, including failed-revision recovery, forwarding manual edits with the latest instructions, saved-history restoration after reopening, stale-save rejection, and excluding history from exports. Revision requests reuse the same stateless Responses flow and send the current caption and source context explicitly.

The version 0.2.1 native Windows smoke test also replaced a saved caption through the real editor, verified its persisted history through Rust IPC, restored the previous caption from History, and saved it successfully. This check used only a test collection and did not call model inference.

## Browser image drops (0.2.2)

The app now uses HTML5 drop events on Windows and macOS, with Tauri's native file-drop interceptor disabled. Chrome-style `text/html` image sources, direct URL payloads, and image File bytes are accepted. Web downloads preserve source URL metadata, validate decoded image content, check public destinations and redirects, and enforce a 50 MB limit. Byte imports retain the input bytes. Local-file linking is preserved through file dialogs or local-file URI drops.

All 16 core tests, 3 backend tests, and 13 UI scenarios passed. The native Windows smoke test injected browser-style HTML/URL payloads into the real WebView, downloaded a W3C image through its live redirect, checked the retained source URL in the item and export, and verified original/reference assignment from real local image bytes. The remaining existing filesystem, caption history, and export checks passed. This automated test did not perform a physical mouse drag between Chrome and Nabor; that interaction and Mac runtime behavior remain manual checks. The Windows NSIS installer was built successfully.

## Release boundary

Stage 5 is gated on successful native tests on both Mac architectures and Windows, plus production signing credentials. CI configuration alone is not proof that these gates have passed. No macOS artifact or signed release was produced on the Windows development host.
