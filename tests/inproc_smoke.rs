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
//! Reaching `SessionReady` means octos resolved a provider and constructed the
//! agent — worth pinning on its own, since the hand-rolled resolver this
//! replaced silently missed keys in the auth store and behind `keychain:`
//! markers, and reported "no provider" where the child process ran fine. The
//! second test goes further and completes a real turn.
//!
//! Running BOTH matters, and running them close together matters more: the
//! episode store takes an exclusive redb lock, so the second backend builds
//! while the first is still letting go. That is the same window a user hits by
//! pressing Stop and then Send, and it failed outright until `build_agent`
//! learned to wait the previous holder out.

#![cfg(feature = "agent-octos")]

use host_launcher::generate::acp_client::AcpEvent;
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

/// A real prompt turn, end to end: prompt in, streamed text out, `TurnDone`.
///
/// This one DOES call the provider — a handful of tokens, deliberately the
/// smallest prompt that still proves the round trip. Reaching `SessionReady`
/// only shows the agent was constructed; it says nothing about whether the
/// reporter, the streaming dedupe or the history rules survived the move to
/// octos's factory, which is where the real risk was.
#[test]
#[ignore = "makes a real provider call; needs an octos provider config"]
fn in_process_backend_completes_a_prompt_turn() {
    let workspace = std::env::temp_dir().join("hl_inproc_turn");
    let mut client = InProcessOctos::start(&workspace).expect("backend should start");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut ready = false;
    let mut streamed = String::new();
    let mut done: Option<(String, String)> = None;

    while std::time::Instant::now() < deadline {
        for event in client.drain_events() {
            match event {
                AcpEvent::SessionReady => {
                    ready = true;
                    client.send_prompt("Reply with exactly: OK");
                }
                AcpEvent::Chunk(text) => streamed.push_str(&text),
                AcpEvent::TurnDone { stop_reason, text } => done = Some((stop_reason, text)),
                AcpEvent::Error(e) | AcpEvent::ProcessGone(e) => panic!("agent failed: {e}"),
                _ => {}
            }
        }
        if done.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    assert!(ready, "never reached SessionReady");
    let (stop_reason, text) = done.expect("no TurnDone within 120s");
    assert_eq!(stop_reason, "end_turn", "turn did not end cleanly");
    assert!(!text.trim().is_empty(), "TurnDone carried no reply text");
    // Streaming is what the console renders; a turn that only produced a final
    // Response would still pass the assertions above while the bar sat empty.
    assert!(
        !streamed.trim().is_empty(),
        "nothing streamed — the reporter is not forwarding chunks",
    );
    println!("streamed {} bytes, reply: {:?}", streamed.len(), text.trim());
}
