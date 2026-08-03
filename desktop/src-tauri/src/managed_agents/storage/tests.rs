use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write as _;

use tempfile::NamedTempFile;

use super::{
    agent_keyring_name, hydrate_keys_with, migrate_inline_key, persist_agent_keys_with,
    KeyMigration, KeyStore, KeyringProbe, ManagedAgentRecord,
};

/// In-memory [`KeyStore`] for testing the migrate decision without the OS
/// keyring. `reachable=false` simulates a backend outage; `fail_verify`
/// simulates a write whose read-back does not confirm.
struct FakeKeyStore {
    reachable: bool,
    fail_verify: bool,
    stored: RefCell<HashMap<String, String>>,
    write_count: RefCell<usize>,
    read_count: RefCell<usize>,
}

impl FakeKeyStore {
    fn reachable() -> Self {
        Self {
            reachable: true,
            fail_verify: false,
            stored: RefCell::new(HashMap::new()),
            write_count: RefCell::new(0),
            read_count: RefCell::new(0),
        }
    }
    fn unreachable() -> Self {
        Self {
            reachable: false,
            fail_verify: false,
            stored: RefCell::new(HashMap::new()),
            write_count: RefCell::new(0),
            read_count: RefCell::new(0),
        }
    }
    fn verify_fails() -> Self {
        Self {
            reachable: true,
            fail_verify: true,
            stored: RefCell::new(HashMap::new()),
            write_count: RefCell::new(0),
            read_count: RefCell::new(0),
        }
    }
    /// Seed a key as already present in the keyring.
    fn with_key(self, name: &str, value: &str) -> Self {
        self.stored
            .borrow_mut()
            .insert(name.to_string(), value.to_string());
        self
    }
}

impl KeyStore for FakeKeyStore {
    fn probe(&self, _name: &str) -> KeyringProbe {
        if self.reachable {
            KeyringProbe::ReachableButEmpty
        } else {
            KeyringProbe::Unreachable
        }
    }
    fn load(&self, name: &str) -> Result<Option<String>, String> {
        // An unreachable backend errors on read (outage), distinct from a
        // reachable backend returning `Ok(None)` for an absent entry.
        if !self.reachable {
            return Err("keyring backend unreachable".to_string());
        }
        *self.read_count.borrow_mut() += 1;
        Ok(self.stored.borrow().get(name).cloned())
    }
    fn load_all_readonly(&self) -> Result<Option<HashMap<String, String>>, String> {
        if !self.reachable {
            return Err("keyring backend unreachable".to_string());
        }
        *self.read_count.borrow_mut() += 1;
        let map = self.stored.borrow().clone();
        // Return None when completely empty (simulates no blob written yet).
        if map.is_empty() {
            Ok(None)
        } else {
            Ok(Some(map))
        }
    }
    fn write_and_verify(&self, name: &str, value: &str) -> Result<(), String> {
        if self.fail_verify {
            return Err("read-back verify failed".to_string());
        }
        *self.write_count.borrow_mut() += 1;
        self.stored
            .borrow_mut()
            .insert(name.to_string(), value.to_string());
        Ok(())
    }
    fn store_all_and_verify(&self, entries: &HashMap<String, String>) -> Result<(), String> {
        if !self.reachable {
            return Err("keyring backend unreachable".to_string());
        }
        if self.fail_verify {
            return Err("read-back verify failed".to_string());
        }
        *self.write_count.borrow_mut() += 1;
        let mut stored = self.stored.borrow_mut();
        for (k, v) in entries {
            stored.insert(k.clone(), v.clone());
        }
        Ok(())
    }
    fn store_all(&self, entries: &HashMap<String, String>) -> Result<(), String> {
        self.store_all_and_verify(entries)
    }
}

fn record_with_key(nsec: &str) -> ManagedAgentRecord {
    record_with_pubkey_and_key("agent-pubkey", nsec)
}

fn record_with_pubkey_and_key(pubkey: &str, nsec: &str) -> ManagedAgentRecord {
    serde_json::from_str(&format!(
        r#"{{
            "pubkey": "{pubkey}",
            "name": "test-agent",
            "private_key_nsec": "{nsec}",
            "relay_url": "wss://localhost:3000",
            "acp_command": "buzz-acp",
            "agent_command": "goose",
            "agent_args": [],
            "mcp_command": "",
            "turn_timeout_seconds": 320,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }}"#
    ))
    .expect("sample record")
}

#[test]
fn migrate_persists_and_signals_stripping_when_keyring_reachable() {
    let store = FakeKeyStore::reachable();
    let record = record_with_key("nsec1realkey");

    let outcome = migrate_inline_key(&store, &record);

    assert_eq!(outcome, KeyMigration::Persisted);
    assert_eq!(
        store
            .stored
            .borrow()
            .get(&agent_keyring_name("agent-pubkey"))
            .map(String::as_str),
        Some("nsec1realkey")
    );
}

#[test]
fn migrate_keeps_inline_when_keyring_unreachable() {
    let store = FakeKeyStore::unreachable();
    let record = record_with_key("nsec1realkey");

    let outcome = migrate_inline_key(&store, &record);

    assert_eq!(outcome, KeyMigration::KeptInline);
    assert!(store.stored.borrow().is_empty());
}

#[test]
fn migrate_keeps_inline_when_verify_fails() {
    let store = FakeKeyStore::verify_fails();
    let record = record_with_key("nsec1realkey");

    assert_eq!(
        migrate_inline_key(&store, &record),
        KeyMigration::KeptInline
    );
}

#[test]
fn migrate_reports_nothing_for_empty_key() {
    let store = FakeKeyStore::reachable();
    let record = record_with_key("");

    assert_eq!(migrate_inline_key(&store, &record), KeyMigration::Nothing);
    assert!(store.stored.borrow().is_empty());
}

#[test]
fn hydrate_fills_key_from_keyring_when_reachable() {
    let store =
        FakeKeyStore::reachable().with_key(&agent_keyring_name("agent-pubkey"), "nsec1stored");
    let mut records = vec![record_with_key("")];

    hydrate_keys_with(&store, &mut records);

    assert_eq!(records[0].private_key_nsec, "nsec1stored");
}

#[test]
fn hydrate_leaves_key_empty_on_keyring_outage() {
    let store = FakeKeyStore::unreachable();
    let mut records = vec![record_with_key("")];

    hydrate_keys_with(&store, &mut records);

    assert!(
        records[0].private_key_nsec.is_empty(),
        "an unreadable key must stay empty, not be fabricated"
    );
}

#[test]
fn spawn_refused_when_private_key_empty() {
    let record = record_with_key("");
    assert!(
        super::spawn_key_refusal(&record).is_some(),
        "an agent with no private key must be refused"
    );
}

#[test]
fn spawn_allowed_when_private_key_present() {
    let record = record_with_key("nsec1realkey");
    assert!(super::spawn_key_refusal(&record).is_none());
}

#[test]
fn persist_agent_keys_issues_zero_writes_when_inline_keys_already_cleared() {
    let store = FakeKeyStore::reachable();
    let mut records = vec![record_with_key(""), record_with_key("")];

    persist_agent_keys_with(&store, &mut records);

    assert_eq!(
        *store.write_count.borrow(),
        0,
        "a save with no inline keys must issue zero keychain writes"
    );
}

#[test]
fn persist_agent_keys_batches_inline_keys_into_one_write() {
    let store = FakeKeyStore::reachable();
    let mut records = vec![
        record_with_pubkey_and_key("pubkey-agent-alpha", "nsec1key_a"),
        record_with_pubkey_and_key("pubkey-agent-beta", "nsec1key_b"),
    ];

    persist_agent_keys_with(&store, &mut records);

    assert_eq!(
        *store.write_count.borrow(),
        1,
        "all inline keys must be persisted in one blob write"
    );
    assert_eq!(
        store
            .stored
            .borrow()
            .get(&agent_keyring_name("pubkey-agent-alpha"))
            .map(String::as_str),
        Some("nsec1key_a"),
    );
    assert_eq!(
        store
            .stored
            .borrow()
            .get(&agent_keyring_name("pubkey-agent-beta"))
            .map(String::as_str),
        Some("nsec1key_b"),
    );
    assert!(records[0].private_key_nsec.is_empty());
    assert!(records[1].private_key_nsec.is_empty());
}

fn write_log(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp log");
    file.write_all(content.as_bytes()).expect("write log");
    file
}

#[cfg(unix)]
#[test]
fn restricted_write_lands_owner_only_without_post_write_chmod() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("managed-agents.json");

    super::atomic_write_json_restricted(&path, br#"[{"private_key_nsec":"nsec1secret"}]"#)
        .expect("restricted write");

    let mode = std::fs::metadata(&path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "secret-bearing write must be owner-only");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        r#"[{"private_key_nsec":"nsec1secret"}]"#
    );
}

#[test]
fn meaningful_agent_error_from_log_promotes_wrapped_llm_auth() {
    let file =
        write_log("noise\nAgent reported error (code -32001): llm auth: 401 unauthorized: ...\n");
    let result = super::meaningful_agent_error_from_log(file.path()).unwrap();
    assert!(result.message.contains("llm auth"));
    assert_eq!(result.code, Some(-32001));
}

#[test]
fn meaningful_agent_error_from_log_promotes_unwrapped_llm_auth() {
    let file = write_log("noise\nllm auth: denied\n");
    let result = super::meaningful_agent_error_from_log(file.path()).unwrap();
    assert_eq!(result.message, "Agent reported error: llm auth: denied");
    assert_eq!(result.code, Some(-32001));
}

#[test]
fn meaningful_agent_error_from_log_promotes_bare_model_not_found() {
    let file = write_log("noise\nllm model not found: (some-model) 404\n");
    let result = super::meaningful_agent_error_from_log(file.path()).unwrap();
    assert_eq!(
        result.message,
        "Agent reported error: llm model not found: (some-model) 404"
    );
    assert_eq!(result.code, Some(-32002));
}

#[test]
fn meaningful_agent_error_from_log_promotes_legacy_format() {
    let file = write_log("noise\nAgent reported error: llm: 500 internal\n");
    let result = super::meaningful_agent_error_from_log(file.path()).unwrap();
    assert_eq!(result.message, "Agent reported error: llm: 500 internal");
    assert_eq!(result.code, None);
}

#[test]
fn meaningful_agent_error_from_log_does_not_promote_midline_auth_text() {
    let file = write_log("noise before llm auth: denied\n");
    assert!(super::meaningful_agent_error_from_log(file.path()).is_none());
}

#[test]
fn strips_ansi_from_typical_tracing_line() {
    let input = "\x1b[2m2026-05-27T15:16:32\x1b[0m \x1b[32m INFO\x1b[0m \x1b[2mbuzz_acp\x1b[0m\x1b[2m:\x1b[0m starting";
    assert_eq!(
        strip_ansi_escapes::strip_str(input),
        "2026-05-27T15:16:32  INFO buzz_acp: starting"
    );
}

#[test]
fn copy_agent_keys_copies_keys_present_in_src_to_dst() {
    let src = FakeKeyStore::reachable()
        .with_key(&agent_keyring_name("agent-alpha"), "nsec1alpha")
        .with_key(&agent_keyring_name("agent-beta"), "nsec1beta");
    let dst = FakeKeyStore::reachable();

    super::copy_agent_keys_between_stores(
        &["agent-alpha".to_string(), "agent-beta".to_string()],
        &src,
        &dst,
    );

    assert_eq!(
        dst.stored
            .borrow()
            .get(&agent_keyring_name("agent-alpha"))
            .map(String::as_str),
        Some("nsec1alpha"),
        "agent-alpha must be copied from src to dst"
    );
    assert_eq!(
        dst.stored
            .borrow()
            .get(&agent_keyring_name("agent-beta"))
            .map(String::as_str),
        Some("nsec1beta"),
        "agent-beta must be copied from src to dst"
    );
    assert_eq!(
        dst.stored
            .borrow()
            .get(super::DEV_MIGRATION_MARKER)
            .map(String::as_str),
        Some("done"),
        "migration-complete marker must be set after first migration"
    );
    assert_eq!(
        *dst.write_count.borrow(),
        1,
        "must perform exactly one bulk write"
    );
    assert_eq!(
        *src.read_count.borrow(),
        1,
        "src must be read exactly once (bulk)"
    );
}

#[test]
fn copy_agent_keys_skips_keys_already_in_dst() {
    let src = FakeKeyStore::reachable().with_key(&agent_keyring_name("agent-alpha"), "nsec1old");
    let dst = FakeKeyStore::reachable().with_key(&agent_keyring_name("agent-alpha"), "nsec1new");

    super::copy_agent_keys_between_stores(&["agent-alpha".to_string()], &src, &dst);

    assert_eq!(
        dst.stored
            .borrow()
            .get(&agent_keyring_name("agent-alpha"))
            .map(String::as_str),
        Some("nsec1new"),
        "key already in dst must not be overwritten by migration"
    );
    assert_eq!(
        dst.stored
            .borrow()
            .get(super::DEV_MIGRATION_MARKER)
            .map(String::as_str),
        Some("done"),
        "marker must be set even when all keys are already present"
    );
    assert_eq!(*src.read_count.borrow(), 0);
}

#[test]
fn copy_agent_keys_skips_keys_absent_from_src() {
    let src = FakeKeyStore::reachable();
    let dst = FakeKeyStore::reachable();

    super::copy_agent_keys_between_stores(&["new-agent".to_string()], &src, &dst);

    assert!(
        dst.stored
            .borrow()
            .get(&agent_keyring_name("new-agent"))
            .is_none(),
        "absent src key must produce no agent key write to dst"
    );
    assert_eq!(
        dst.stored
            .borrow()
            .get(super::DEV_MIGRATION_MARKER)
            .map(String::as_str),
        Some("done"),
        "marker must be set even when no keys were present in src"
    );
}

#[test]
fn copy_agent_keys_skips_all_when_dst_unreachable() {
    let src = FakeKeyStore::reachable().with_key(&agent_keyring_name("agent-alpha"), "nsec1alpha");
    let dst = FakeKeyStore::unreachable();

    super::copy_agent_keys_between_stores(&["agent-alpha".to_string()], &src, &dst);

    assert_eq!(*dst.write_count.borrow(), 0);
    assert_eq!(
        *src.read_count.borrow(),
        0,
        "src must not be accessed when dst is unreachable"
    );
}

#[test]
fn copy_agent_keys_skips_entirely_when_marker_present() {
    let src = FakeKeyStore::reachable().with_key(&agent_keyring_name("agent-alpha"), "nsec1alpha");
    let dst = FakeKeyStore::reachable()
        .with_key(super::DEV_MIGRATION_MARKER, "done")
        .with_key(&agent_keyring_name("agent-alpha"), "nsec1dev");

    super::copy_agent_keys_between_stores(&["agent-alpha".to_string()], &src, &dst);

    assert_eq!(
        *src.read_count.borrow(),
        0,
        "src must not be read when migration-complete marker is present"
    );
    assert_eq!(
        *dst.write_count.borrow(),
        0,
        "dst must not be written when migration-complete marker is present"
    );
    assert_eq!(
        dst.stored
            .borrow()
            .get(&agent_keyring_name("agent-alpha"))
            .map(String::as_str),
        Some("nsec1dev"),
        "dev key must not be overwritten on subsequent boots"
    );
}

#[test]
fn copy_agent_keys_writes_marker_even_with_empty_agent_list() {
    let src = FakeKeyStore::reachable();
    let dst = FakeKeyStore::reachable();

    super::copy_agent_keys_between_stores(&[], &src, &dst);

    assert_eq!(
        dst.stored
            .borrow()
            .get(super::DEV_MIGRATION_MARKER)
            .map(String::as_str),
        Some("done"),
        "marker must be set even when pubkey list is empty"
    );
    assert_eq!(*src.read_count.borrow(), 0);
}

#[test]
fn try_delete_agent_key_returns_result() {
    let _: fn(&str) -> Result<(), String> = super::try_delete_agent_key;
}
