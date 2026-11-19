# Aibro maintenance

This is a Windows-first Rust + cef-rs agent browser. Read README.md for the protocol and build lifecycle.

- Keep CEF handles on the UI thread. Use the Work channel for requests from other threads.
- Keep browser control explicit and typed. Do not expose arbitrary page evaluation or shell execution through the agent API.
- Every new snapshot invalidates old element refs. Preserve isolated-world state and reject disconnected targets.
- Do not log command arguments, credentials, snapshot contents, or screenshot data.
- Keep the Chromium process sandbox enabled; use the official cef-rs bootstrap bundler.
- Stop the running bundle before replacing its DLLs. Build with scripts/build.ps1.
- Use the shared CEF_PATH from scripts/build.ps1 when compiling tests to avoid downloading another runtime.
- For browser-control changes run cargo fmt --check, cargo test --lib, and scripts/smoke.mjs against the actual bundled browser.
- Document unsupported behavior and keep MCP descriptions aligned with Command.
- Runtime assets, profiles, tokens, references, and screenshots remain ignored. Commit Cargo.lock.
