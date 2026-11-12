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

### Batch captions (0.3.0)

On Windows, 17 Playwright scenarios and 36 Rust tests pass (22 collection/core, 8 agent, 6 desktop backend). Coverage includes collection-wide queries beyond the visible page, selected IDs across pages, common prompt precedence, skipping saved captions and drafts, persistent draft revisions, interrupted-run recovery, pause/resume/cancel, retrying item failures, pausing on usage/authentication errors, and refusing stale draft acceptance. Exports exclude unaccepted drafts.

The hidden native Windows WebView smoke harness additionally verified the collection prompt and batch previews through real IPC, indexed a durable test draft, kept a manual edit, and accepted it through Save all drafts. It also passed its existing image-drop, linked-file, revision-history, and byte-exact export checks. This run skipped agent settings and made no inference requests. Live batch generation against the user's plan, macOS behavior, and signed installer acceptance remain unverified.

### Item conversations (0.4.0)

On Windows, 20 Playwright scenarios and 42 Rust tests pass (26 collection/core, 9 agent, 7 desktop backend). New checks cover per-item conversation persistence and portability, explicit retry after interruption or failure, cross-item isolation, conversation revision conflicts, schema preservation, bounded chronological history, and Responses input roles. UI tests verify follow-ups, copy, note appending, caption review before replacement, clearing only conversation content, and retry without duplicate turns. Discussing an item leaves its manifest unchanged. Export excludes conversations.

The hidden Windows WebView smoke test also passed real conversation IPC: reopening a saved fixture reply, appending it to existing instructions, opening it for caption review and discarding without replacing the final caption, and exporting without conversation content. It repeated existing filesystem, queue/draft, and byte-exact export checks. Screenshot: `test-results/native-conversation.png`.

These checks use fixture responses and make no inference requests. Live conversation requests, native clipboard copying, macOS behavior, and signed installer acceptance remain unverified.

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
