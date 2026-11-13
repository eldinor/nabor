# Building Nabor on macOS

These notes match Nabor 0.8.2. macOS builds must be compiled and checked on a Mac; this workspace has been validated on Windows so far.

## 1. Install prerequisites

### Xcode Command Line Tools

In Terminal:

```bash
xcode-select --install
```

Finish the installation dialog before continuing. Verify:

```bash
xcode-select -p
```

Full Xcode is optional for this desktop app. If you use full Xcode, launch it once to finish its setup. See the [Tauri macOS prerequisites](https://v2.tauri.app/start/prerequisites/#macos).

### Node.js and npm

Install **Node.js 22.20 or a newer 22.x version**, including npm, from the [official Node.js downloads](https://nodejs.org/en/download). The repository's CI uses Node 22.20.0.

On Apple Silicon, choose the ARM64 installer and use a native Terminal session. On Intel, choose x64. Check:

```bash
node --version
npm --version
uname -m
```

`uname -m` should report `arm64` for a native Apple Silicon session, or `x86_64` on Intel.

### Rust

Install stable Rust with [rustup](https://www.rust-lang.org/tools/install):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
```

Check the compiler and build tool:

```bash
rustc --version
cargo --version
rustc -vV
```

The host shown by `rustc -vV` should be `aarch64-apple-darwin` on native Apple Silicon, or `x86_64-apple-darwin` on Intel. Use a current stable compiler; update an older installation with `rustup update stable`.

## 2. Get the repository

Clone the latest repository or download and extract its source ZIP. Open Terminal in the repository root: the folder containing `package.json`, `package-lock.json`, and `src-tauri/`.

Keep both lockfiles (`package-lock.json` and `src-tauri/Cargo.lock`). Install dependencies on the Mac:

```bash
npm ci
```

Do not reuse Windows copies of `node_modules`, `src-tauri/target`, or `.tools`. The `.tools` Rust installation and PowerShell commands in the Windows setup notes are specific to that machine. The first build needs internet access to download dependencies.

The npm install supplies the Tauri CLI; a separate global Tauri installation is unnecessary. Document extraction uses Rust libraries and Nabor's own helper process; it does not require Word, LibreOffice, Python, Pandoc, or a separate PDF extraction program.

## 3. Run during development

From the repository root:

```bash
npm run tauri -- dev
```

This launches the native desktop app and the frontend development server. Stop it with **Ctrl+C** in Terminal.

`npm run dev` alone is a browser preview. Use the Tauri command for native dialogs, collection files, clipboard access, and document extraction.

## 4. Build the app and installer

```bash
npm run tauri -- build
```

This builds the frontend, compiles Rust, and packages the application for the Mac's configured Rust host architecture. By default, an Apple Silicon build and an Intel build are separate artifacts.

With the default native build, outputs are under:

```text
src-tauri/target/release/bundle/macos/Nabor.app
src-tauri/target/release/bundle/dmg/*.dmg
```

Launch the built application:

```bash
open "src-tauri/target/release/bundle/macos/Nabor.app"
```

For an app bundle without building a DMG:

```bash
npm run tauri -- build --bundles app
```

To install a DMG build, open the generated DMG and copy Nabor into Applications. Keep the `.app` bundle intact: document extraction invokes its own executable as a background helper.

The configuration currently sets `minimumSystemVersion` to macOS 11.0. That is the configured application deployment target, not proof that current Node, Rust, and Xcode build tools run on every older macOS release. Actual compatibility still needs native testing. See [Tauri application bundles](https://v2.tauri.app/distribute/macos-application-bundle/).

## 5. Run automated checks

```bash
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --workspace
npx playwright install chromium
npm test
```

The UI tests use a development fixture in Chromium. They complement native testing; they do not verify macOS WebKit, system dialogs, or the clipboard by themselves.

The following scripts currently target Windows and should not be used as Mac test commands:

- `scripts/native-smoke.mjs` — Windows WebView2 automation.
- `scripts/document-worker-smoke.mjs` — uses the Windows `nabor.exe` release path.
- `scripts/tauri.smoke.conf.json` — Windows smoke-test window configuration.

## 6. Check the Mac app manually

Use a temporary collection for the first check:

1. Create/open a collection with the native folder dialog.
2. Import copied and linked images; test Finder drag-and-drop and **Cmd+V** paste.
3. Switch between Grid and Masonry, open an item, edit its caption, and restart.
4. Add a website attachment and use **Open in browser** and **Copy link**.
5. Attach PDF, DOCX, MD, and TXT references. Confirm extracted text appears, **View text** works, and **Open in default app** uses the expected application. A scanned PDF should report that no text is available.
6. Move a linked file outside Nabor, then repair its attachment with **Locate file**.
7. Test a Unicode filename and a collection folder on an external drive.
8. Export an item with images, a document, and a website link. Check that original files and their item relationships are preserved.
9. If using the agent, connect through Agent settings on this Mac and test a request using a disposable item. This uses the selected account's allowance.
10. In **Appearance**, test **System**, **Light**, and **Dark**. Change macOS appearance while System is selected, restart with a manual choice, and verify that theme changes keep edits and unsent questions. Check menus, document text, error states, and transparent image previews in both themes.

See [docs/VALIDATION.md](docs/VALIDATION.md) for the full release checklist and recorded Windows results. Apple Silicon and Intel both need native acceptance checks.

## Collections transferred from Windows

Copy the entire collection folder. Managed attachment paths are relative and remain usable after moving the folder. Linked files use absolute paths and may need **Locate file** on the Mac. Account credentials are separate from collection data; set up the agent connection again on the Mac.

## Signing and distribution

The basic local build does not require signing credentials. Signing and notarization for distribution are separate release steps. Follow the [README release instructions](README.md#build-and-release) and [Tauri macOS signing documentation](https://v2.tauri.app/distribute/sign/macos/). Signing credentials are not included in the repository.

The existing [desktop CI workflow](.github/workflows/desktop.yml) defines separate Apple Silicon and Intel macOS jobs. Its configuration alone does not confirm that a particular commit has passed those jobs.

## Common build problems

- **`cargo` or `rustc` not found:** open a new Terminal or run `source "$HOME/.cargo/env"`.
- **Unsupported Node version:** switch to Node 22.20+ in the 22.x line, then run `npm ci` again.
- **Compiler/SDK tools missing:** finish Command Line Tools installation and check `xcode-select -p`. If using full Xcode, complete its first-launch setup.
- **Port 1420 already in use:** stop the other Nabor/Vite development server before starting another development run.
- **DMG packaging fails after the app was built:** try the app-only build above to check the application separately, and keep the DMG error output for troubleshooting.
- **Document extraction fails:** check the attachment's status and use **Re-extract text**. Documents are limited to 20 MB; extraction has a 20-second timeout. OCR and legacy `.doc` files are not supported.
