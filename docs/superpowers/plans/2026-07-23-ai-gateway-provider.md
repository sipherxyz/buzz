# AI Gateway Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a first-class `ai-gateway` provider to the Buzz desktop app and bundled `buzz-agent`, using the employee's locally installed AI Gateway for credentials and live model discovery.

**Architecture:** The desktop UI stores only the provider and selected model. The Tauri backend resolves the AI Gateway profile, base URL, and credential at runtime, injects them into the managed `buzz-agent` process, and queries the gateway's OpenAI-compatible `/v1/models` endpoint with the detailed-model header. `buzz-agent` reuses its OpenAI-compatible chat transport.

**Tech Stack:** Rust, Tauri 2, React 19, TypeScript, Vitest, reqwest, macOS Keychain.

---

### Task 1: Define and test the provider contract

- Add failing Rust tests in `crates/buzz-agent/src/config.rs` proving `ai-gateway` selects the OpenAI-compatible transport.
- Add failing desktop tests proving `ai-gateway` appears in the provider list, requires an explicit model, and does not request a plaintext API-key field.
- Implement the minimal provider aliases and UI metadata required to pass.

### Task 2: Resolve the local AI Gateway profile securely

- Add unit tests for resolving profile/base URL/key from explicit environment overrides and the AI Gateway macOS Keychain entry.
- Implement a small Tauri-side resolver with redacted errors and no persistence of the credential in Buzz records.
- Inject the resolved OpenAI-compatible environment only for managed `buzz-agent` launches using provider `ai-gateway`.
- Add readiness tests proving a working local profile satisfies credential checks and an unavailable profile produces an actionable requirement.

### Task 3: Discover and display live gateway models

- Add HTTP fixture tests for `GET /v1/models`, Bearer authentication, and `X-AI-Gateway-Models-Detail: full`.
- Parse the gateway's optional `display_name` while remaining compatible with plain OpenAI model responses; keep provider-specific capability metadata out of Buzz until the shared model contract can represent it.
- Make AI Gateway discovery fail closed: no built-in fallback list when its live endpoint is unavailable.
- Add React tests for loading, success, empty, authentication failure, and unreachable gateway states.

### Task 4: Verify the provider end to end

- Run focused Rust and desktop tests during each red-green cycle.
- Run `cargo test -p buzz-agent`, `just desktop-tauri-test`, and the relevant desktop Vitest suite.
- Run formatting and lint checks for every touched workspace.
