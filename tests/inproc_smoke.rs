//! The in-process octos backend, against a REAL provider config.
//!
//! Ignored by default and gated on `agent-octos`, because it needs an octos
//! setup this machine actually has — there is no offline substitute. The rest
//! of the suite injects `fake_acp` through `HOST_LAUNCHER_AGENT_CMD`, and that
//! override deliberately wins over the in-process path (see `start_backend`),
//! so nothing else exercises this code at all.
//!
//! Run it by hand after touching the backend or bumping the octos pin:
//!
//! ```bash
//! cargo test --features agent-octos --test inproc_smoke -- --ignored --nocapture
//! ```
//!
//! No API call: reaching `SessionReady` means octos resolved a provider and
//! constructed the agent, not that it talked to anyone. That is the thing worth
//! pinning — a hand-rolled resolver silently missed keys in the auth store and
//! behind `keychain:` markers, and reported "no provider" where the child
//! process ran fine.

#![cfg(feature = "agent-octos")]

use host_launcher::generate::octos_inproc::InProcessOctos;
use host_launcher::generate::AgentTransport;

#[test]
#[ignore = "needs a real octos provider config on this machine"]
fn in_process_backend_builds_an_agent() {
    let workspace = std::env::temp_dir().join("hl_inproc_smoke");
    let mut client = InProcessOctos::start(&workspace).expect("backend should start");

    // Generous: building the provider chain can touch the keychain, which may
    // prompt. It does not make a network call.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut seen: Vec<String> = Vec::new();
    while std::time::Instant::now() < deadline {
        for event in client.drain_events() {
            seen.push(format!("{event:?}"));
        }
        if seen.iter().any(|e| e.contains("SessionReady")) {
            return;
        }
        assert!(
            !seen.iter().any(|e| e.contains("ProcessGone")),
            "the factory failed to build an agent: {seen:?}",
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("no SessionReady within 60s, saw: {seen:?}");
}
