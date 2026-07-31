# fs-mcp-rs desktop

Native Tauri desktop application for configuring and running the embedded fs-mcp-rs HTTP server. It uses the same MCP handler, validated `Settings` model, security policy, and client snippet formatting as the CLI.

## Development

```console
npm install
npm run dev:tauri
```

`dev:tauri` disables Tauri's Rust file watcher so frontend changes stay in Vite HMR
instead of restarting the native application. Use `npm run dev` for the frontend
only. The frontend-only Vite view cannot use persistence or server commands because those APIs intentionally require the native backend.

Validate and build the frontend with `npm run build`. Build the native debug bundle with `npm run tauri -- build --debug`, or use `npm run tauri -- build` for a production bundle. Tauri writes platform installers under the Cargo target bundle directory.

## Configuration

Profiles are stored as schema-versioned JSON in Tauri's platform app-data directory. The active profile id is stored separately. Writes are explicit through **Save profile** and are performed atomically; profile settings are not kept in browser localStorage. A first launch creates one default profile awaiting at least one allowed folder. Corrupt or unsupported persistence data is reported in the UI and can be replaced with defaults.

Each profile supports multiple filesystem roots, including roots on different Windows drives, along with server, filesystem, search, terminal, OAuth, logging, and optional tunnel settings. The Rust backend validates all roots and limits again before starting. If saved settings change while the server runs, the old runtime remains active until **Restart** is used; restart disconnects an active tunnel first.

**Export TOML** writes an absolute-root configuration compatible with `Settings::load`. Existing files require confirmation. **Client snippets** generates a Claude Desktop STDIO entry referencing the profile's generated TOML and a Cursor/general MCP HTTP URL through the shared CLI formatter. Snippets never embed roots or OAuth secrets and can be copied or saved as JSON.

## Runtime limitations

The desktop-managed embedded server is HTTP-only. The generated Claude Desktop STDIO snippet launches the separately installed `fs-mcp-rs` CLI with the saved profile TOML; the desktop process itself does not expose STDIO transport.

Remote access requires the selected provider binary (`cloudflared`, `ngrok`, or `zrok`) to be installed or configured with an explicit executable path. ngrok may require an authtoken and zrok requires an enabled environment. Child tunnel processes are dropped on stop, restart, and application exit; shutdown signaling is best-effort during forced OS termination.

Terminal access executes commands with the desktop user's permissions and is not a sandbox. Binding to a non-loopback address or opening a public tunnel exposes every enabled tool and allowed root; use narrow roots, read-only mode, authentication, and a trusted TLS boundary.
