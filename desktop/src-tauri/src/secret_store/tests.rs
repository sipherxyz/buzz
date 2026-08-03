use super::*;

impl SecretStore {
    fn with_cache(service: &str, cache: Option<HashMap<String, String>>) -> Self {
        SecretStore {
            service: service.to_string(),
            cache: Mutex::new(cache),
            backend_io: Mutex::new(()),
            session_unavailable: AtomicBool::new(false),
        }
    }
}

#[test]
fn probe_returns_present_when_key_in_cache() {
    let mut map = HashMap::new();
    map.insert("identity".to_string(), "nsec1test".to_string());
    let store = SecretStore::with_cache("buzz-test-cache-hit", Some(map));
    assert_eq!(store.probe("identity"), KeyringProbe::Present);
}

#[test]
fn load_returns_value_when_key_in_cache() {
    let mut map = HashMap::new();
    map.insert("identity".to_string(), "nsec1test".to_string());
    let store = SecretStore::with_cache("buzz-test-load-cache-hit", Some(map));
    assert_eq!(
        store.load("identity").unwrap(),
        Some("nsec1test".to_string())
    );
}

#[test]
fn unavailable_session_fails_fast_without_reopening_keychain() {
    let store = SecretStore::with_cache("buzz-test-session-unavailable", None);
    store.session_unavailable.store(true, Ordering::Release);

    assert_eq!(
        store.load_blob().unwrap_err(),
        "keyring unavailable for this app session"
    );
}

#[ignore = "requires real OS keychain (run locally)"]
#[test]
fn test_stale_warm_cache_add_observes_prior_write() {
    let svc = "buzz-test-race-stale-cache";

    let setup = SecretStore::keyring(svc);
    let _ = setup.delete("k1");
    let _ = setup.delete("k2");
    let _ = setup.delete("k3");

    let store_a = SecretStore::keyring(svc);
    store_a.store("k1", "v1").unwrap();

    let store_b = SecretStore::keyring(svc);
    store_b.store("k2", "v2").unwrap();

    store_a.store("k3", "v3").unwrap();

    let reader = SecretStore::keyring(svc);
    assert_eq!(
        reader.load("k1").unwrap(),
        Some("v1".to_string()),
        "k1 must survive"
    );
    assert_eq!(
        reader.load("k2").unwrap(),
        Some("v2".to_string()),
        "k2 must not be dropped"
    );
    assert_eq!(
        reader.load("k3").unwrap(),
        Some("v3".to_string()),
        "k3 must be written"
    );

    let _ = reader.delete("k1");
    let _ = reader.delete("k2");
    let _ = reader.delete("k3");
}

#[ignore = "requires real OS keychain (run locally)"]
#[test]
fn test_concurrent_adds_neither_key_dropped() {
    let svc = "buzz-test-race-concurrent-add";

    let setup = SecretStore::keyring(svc);
    let _ = setup.delete("agent_a");
    let _ = setup.delete("agent_b");

    let store1 = SecretStore::keyring(svc);
    store1.store("agent_a", "nsec1aaa").unwrap();

    let store2 = SecretStore::keyring(svc);
    store2.store("agent_b", "nsec1bbb").unwrap();

    let reader = SecretStore::keyring(svc);
    assert_eq!(
        reader.load("agent_a").unwrap(),
        Some("nsec1aaa".to_string()),
        "agent_a must not be dropped"
    );
    assert_eq!(
        reader.load("agent_b").unwrap(),
        Some("nsec1bbb".to_string()),
        "agent_b must not be dropped"
    );

    let _ = reader.delete("agent_a");
    let _ = reader.delete("agent_b");
}

#[test]
fn test_blob_lockfile_path_is_in_tmp_with_uid() {
    let path = blob_lockfile_path("buzz-desktop");
    #[cfg(unix)]
    {
        let uid = unsafe { libc::getuid() };
        assert!(
            path.starts_with("/tmp"),
            "lockfile {path:?} must start with /tmp (not $TMPDIR)"
        );
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        assert!(
            name.contains(&uid.to_string()),
            "lockfile {path:?} must contain uid {uid}"
        );
        assert!(
            name.contains("buzz-keychain"),
            "lockfile name must contain 'buzz-keychain'"
        );
    }
    #[cfg(not(unix))]
    {
        assert!(
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("buzz-keychain")),
            "lockfile name must contain 'buzz-keychain'"
        );
    }
}

#[test]
fn test_blob_lock_acquire_and_release() {
    let guard = acquire_blob_lock("buzz-test-lock-smoke");
    assert!(
        guard.is_ok(),
        "advisory lock acquire must succeed: {:?}",
        guard.err()
    );
    drop(guard);
    let guard2 = acquire_blob_lock("buzz-test-lock-smoke");
    assert!(
        guard2.is_ok(),
        "advisory lock re-acquire after release must succeed: {:?}",
        guard2.err()
    );
}

#[ignore = "requires real OS keychain (run locally)"]
#[test]
fn mutate_blob_does_not_advance_cache_on_write_failure() {
    let mut map = HashMap::new();
    map.insert("existing".to_string(), "durable_val".to_string());
    let store = SecretStore::with_cache("buzz-test-cow-write-fail", Some(map));

    let result = store.store("new_key", "new_val");

    if result.is_err() {
        assert_eq!(
            store.load("existing").unwrap(),
            Some("durable_val".to_string()),
            "cache must remain at last durable state after write failure"
        );
        let after = store.load("new_key");
        assert!(
            matches!(after, Ok(None) | Err(_)),
            "a key whose write failed must not be visible via load: {after:?}"
        );
    }
}

#[test]
fn availability_error_discriminator() {
    assert!(is_keyring_availability_error("dbus connection failed"));
    assert!(is_keyring_availability_error(
        "org.freedesktop.secrets not provided"
    ));
    assert!(is_keyring_availability_error("No Secret Service"));
    assert!(is_keyring_availability_error(
        "Platform secure storage failure"
    ));
    assert!(is_keyring_availability_error("User canceled the operation"));
    assert!(is_keyring_availability_error("User denied access"));
    assert!(is_keyring_availability_error(
        "Interaction with the Security Server is not allowed"
    ));
    assert!(!is_keyring_availability_error("entry not found"));
}

#[cfg(target_os = "macos")]
#[test]
fn dpk_error_discriminators() {
    let e = SFError::from_code(-34018);
    assert!(is_dpk_unavailable(&e));
    assert!(!is_not_found(&e));
    let e = SFError::from_code(-25300);
    assert!(is_not_found(&e));
    assert!(!is_dpk_unavailable(&e));
}

#[ignore = "requires real OS keychain (run locally)"]
#[test]
fn blob_stores_and_retrieves_multiple_keys() {
    let store = SecretStore::keyring("buzz-test-blob-multi");
    store.store("key_a", "val_a").unwrap();
    store.store("key_b", "val_b").unwrap();
    assert_eq!(store.load("key_a").unwrap(), Some("val_a".to_string()));
    assert_eq!(store.load("key_b").unwrap(), Some("val_b".to_string()));
    assert_eq!(store.load("key_c").unwrap(), None);
    let _ = store.delete("key_a");
    let _ = store.delete("key_b");
}

#[ignore = "requires real OS keychain (run locally)"]
#[test]
fn blob_probe_present_absent_unreachable() {
    let store = SecretStore::keyring("buzz-test-blob-probe");
    assert_eq!(store.probe("identity"), KeyringProbe::ReachableButEmpty);
    store.store("identity", "nsec1test").unwrap();
    assert_eq!(store.probe("identity"), KeyringProbe::Present);
    assert_eq!(store.probe("other"), KeyringProbe::ReachableButEmpty);
    let _ = store.delete("identity");
}

#[ignore = "requires real OS keychain (run locally)"]
#[test]
fn blob_delete_removes_key_not_others() {
    let store = SecretStore::keyring("buzz-test-blob-delete");
    store.store("keep", "keep_val").unwrap();
    store.store("remove", "remove_val").unwrap();
    store.delete("remove").unwrap();
    assert_eq!(store.load("keep").unwrap(), Some("keep_val".to_string()));
    assert_eq!(store.load("remove").unwrap(), None);
    let _ = store.delete("keep");
}

#[ignore = "requires real OS keychain (run locally)"]
#[test]
fn blob_migration_from_per_key_entry() {
    let svc = "buzz-test-blob-migration";
    let key = "identity";
    let value = "nsec1migrationtest";

    let entry = keyring_entry(svc, key).unwrap();
    entry.set_password(value).unwrap();

    let store = SecretStore::keyring(svc);
    assert_eq!(store.probe(key), KeyringProbe::Present);
    assert_eq!(store.load(key).unwrap(), Some(value.to_string()));

    let entry = keyring_entry(svc, key).unwrap();
    assert!(matches!(entry.get_password(), Err(keyring::Error::NoEntry)));

    let store2 = SecretStore::keyring(svc);
    assert_eq!(store2.probe(key), KeyringProbe::Present);
    assert_eq!(store2.load(key).unwrap(), Some(value.to_string()));
    let _ = store2.delete(key);
}

#[ignore = "requires real OS keychain (run locally)"]
#[test]
fn delete_all_with_legacy_cleanup_removes_per_key_identity() {
    let svc = "buzz-test-delete-all-legacy";
    let key = "identity";
    let value = "nsec1legacytest";

    let entry = keyring_entry(svc, key).unwrap();
    entry.set_password(value).unwrap();

    let store = SecretStore::keyring(svc);
    store.store("agent:abc123", "nsec1agent").unwrap();

    let store2 = SecretStore::keyring(svc);
    assert_eq!(store2.probe(key), KeyringProbe::Present);

    store2.delete_all_with_legacy_cleanup().unwrap();

    let store3 = SecretStore::keyring(svc);
    assert_eq!(
        store3.probe(key),
        KeyringProbe::ReachableButEmpty,
        "per-key identity must not survive delete_all_with_legacy_cleanup"
    );
    assert_eq!(
        store3.load(key).unwrap(),
        None,
        "load must not resurrect the legacy per-key identity"
    );
    assert_eq!(store3.load("agent:abc123").unwrap(), None);
}
