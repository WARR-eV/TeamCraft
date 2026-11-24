//! The desktop app's version handshake (`desktop/src/compat.rs`, a file without
//! dependencies, compiled here because the desktop crate needs GUI libraries).

#[path = "../desktop/src/compat.rs"]
mod compat;

#[test]
fn parses_versions() {
    assert_eq!(compat::parse("0.2.0"), Some((0, 2)));
    assert_eq!(compat::parse("v1.4.2-rc.1"), Some((1, 4)));
    assert_eq!(compat::parse("1.10"), Some((1, 10)));
    assert_eq!(compat::parse("1.3-beta"), Some((1, 3)));
    assert_eq!(compat::parse(""), None);
    assert_eq!(compat::parse("abc"), None);
    assert_eq!(compat::parse("2"), None);
}

#[test]
fn before_1_0_major_and_minor_must_match() {
    assert!(compat::notice("0.2.0", "0.2.7").is_none());
    assert!(compat::notice("0.2.5", "0.2.0").is_none());
    let newer = compat::notice("0.2.0", "0.3.0").expect("notice");
    assert!(
        newer.contains("0.2.0")
            && newer.contains("0.3.0")
            && newer.contains("Install TeamCraft Desktop 0.3.x")
    );
    let older = compat::notice("0.3.0", "0.2.0").expect("notice");
    assert!(older.contains("upgrade the server"));
    assert!(older.contains("install TeamCraft Desktop 0.2.x"), "{older}");
}

#[test]
fn the_version_a_server_needs_is_named() {
    // #468: the update window and the notice name it
    assert_eq!(compat::needs("0.3.1").as_deref(), Some("0.3"));
    assert_eq!(compat::needs("v1.4.2-rc.1").as_deref(), Some("1.4"));
    assert_eq!(compat::needs("unknown"), None);
    // the named version always fits the server, whatever patch it gets
    for server in ["0.2.7", "0.3.0", "1.0.0", "1.5.3", "2.1.0"] {
        let needed = compat::needs(server).unwrap();
        assert!(compat::fits(&format!("{needed}.0"), server), "{server}");
        assert!(compat::fits(&format!("{needed}.9"), server), "{server}");
        let app = format!("{needed}.0");
        assert!(compat::notice(&app, server).is_none(), "{server}");
    }
    // from 1.0 an older minor fits too
    assert!(compat::fits("1.2.0", "1.5.0"));
    assert!(!compat::fits("1.6.0", "1.5.0"));
    // unreadable versions fit
    assert!(compat::fits("0.2.0", ""));
    assert!(compat::fits("", "0.2.0"));
}

#[test]
fn from_1_0_a_server_with_the_same_major_and_a_newer_minor_fits() {
    assert!(compat::notice("1.2.0", "1.2.0").is_none());
    assert!(compat::notice("1.2.0", "1.5.1").is_none());
    assert!(compat::notice("1.2.0", "1.1.9").is_some());
    assert!(compat::notice("1.2.0", "2.0.0").is_some());
    assert!(compat::notice("2.0.0", "1.9.0").is_some());
    assert!(compat::notice("1.0.0", "0.9.0").is_some());
}

#[test]
fn an_unreadable_server_version_is_not_blocked() {
    assert!(compat::notice("0.2.0", "").is_none());
    assert!(compat::notice("0.2.0", "unknown").is_none());
}

#[test]
fn local_agent_header_matches_the_server() {
    // a rename on one side would silently turn the server's hint off
    assert!(compat::LOCAL_AGENT_HEADER.eq_ignore_ascii_case(teamcraft::config::LOCAL_AGENT_HEADER));
    assert_eq!(
        compat::LOCAL_AGENT_HEADER_VALUE,
        teamcraft::config::LOCAL_AGENT_HEADER_VALUE
    );
    // a valid header name, as the HTTP stack of the agent needs it
    assert!(axum::http::HeaderName::from_bytes(compat::LOCAL_AGENT_HEADER.as_bytes()).is_ok());
}

#[test]
fn agent_backend_follows_the_policy_then_the_users_pick() {
    use compat::AgentBackend::{Company, Local, Off};
    // off + company AI: the company AI only, whatever was picked before
    for stored in [None, Some(Local), Some(Company)] {
        assert_eq!(
            compat::choose_backend(false, true, true, stored),
            (Company, vec![Company])
        );
    }
    // off + no company AI: no AI at all
    assert_eq!(
        compat::choose_backend(false, false, true, Some(Local)),
        (Off, vec![])
    );
    // both: the stored pick, else a set-up local agent, else the company AI
    assert_eq!(
        compat::choose_backend(true, true, true, Some(Company)),
        (Company, vec![Company, Local])
    );
    assert_eq!(
        compat::choose_backend(true, true, false, Some(Local)),
        (Local, vec![Company, Local])
    );
    assert_eq!(compat::choose_backend(true, true, true, None).0, Local);
    assert_eq!(compat::choose_backend(true, true, false, None).0, Company);
    // allowed, no company AI: the local agent (today's behaviour), even
    // with a stale pick of the company AI
    assert_eq!(
        compat::choose_backend(true, false, false, Some(Company)),
        (Local, vec![Local])
    );
}

#[test]
fn agent_backend_names_round_trip() {
    use compat::AgentBackend;
    for b in [AgentBackend::Company, AgentBackend::Local] {
        assert_eq!(AgentBackend::parse(b.as_str()), Some(b));
    }
    // "off" is a state, never a pick
    assert_eq!(AgentBackend::Off.as_str(), "off");
    assert_eq!(AgentBackend::parse("off"), None);
    assert_eq!(AgentBackend::parse("Company"), None);
    assert_eq!(AgentBackend::parse(""), None);
}

#[test]
fn the_update_window_says_what_the_server_needs() {
    // #468: nothing to say when the update and this app both fit
    assert_eq!(compat::update_note("0.2.0", "0.2.5", "0.2.3"), None);
    // this app does not fit, the update does
    let fits = compat::update_note("0.2.0", "0.3.1", "0.3.0").unwrap();
    assert_eq!(
        fits,
        "Your server runs 0.3.0 and needs TeamCraft Desktop 0.3.x – this update fits it."
    );
    // the update does not fit the server, this app does: keep it – no
    // advice to install the version the user already has
    let newer = compat::update_note("0.2.0", "0.4.0", "0.2.3").unwrap();
    assert_eq!(
        newer,
        "Your server runs 0.2.3; version 0.4.0 may not work with it – keep this version until the administrator upgrades the server."
    );
    assert!(
        !newer.contains("Install") && !newer.contains("needs"),
        "{newer}"
    );
    // neither fits: it names the version the server needs
    let neither = compat::update_note("0.1.0", "0.3.0", "0.2.3").unwrap();
    assert!(
        neither.starts_with("Your server runs 0.2.3 and needs TeamCraft Desktop 0.2.x"),
        "{neither}"
    );
    assert!(neither.contains("version 0.3.0 may not work with it"));
    assert!(neither.contains("Install 0.2.x from the release page"));
    // a server that hides its version: nothing
    assert_eq!(compat::update_note("0.2.0", "0.4.0", "unknown"), None);
}

#[test]
fn the_update_window_follows_the_rules_from_1_0() {
    // an older minor of the same major fits: nothing to say
    assert_eq!(compat::update_note("1.2.0", "1.5.1", "1.5.0"), None);
    // the update is newer than the server, this app fits: no "needs 1.5.x"
    let newer = compat::update_note("1.2.0", "1.6.0", "1.5.0").unwrap();
    assert!(
        newer.contains("version 1.6.0 may not work with it"),
        "{newer}"
    );
    assert!(
        !newer.contains("needs 1.5.x") && !newer.contains("Install"),
        "{newer}"
    );
    // from a 0.x app to the first stable release of the server
    assert_eq!(
        compat::update_note("0.9.0", "1.0.0", "1.0.2").as_deref(),
        Some("Your server runs 1.0.2 and needs TeamCraft Desktop 1.0.x – this update fits it.")
    );
}

#[test]
fn pre_release_and_tag_versions_are_read() {
    // a release candidate on the server needs its major.minor
    assert_eq!(compat::needs("0.3.0-rc.1").as_deref(), Some("0.3"));
    assert_eq!(
        compat::update_note("0.2.9", "0.3.0", "0.3.0-rc.1").as_deref(),
        Some(
            "Your server runs 0.3.0-rc.1 and needs TeamCraft Desktop 0.3.x – this update fits it."
        )
    );
    // an offered tag with a leading v
    assert!(compat::fits("v0.3.0", "0.3.1"));
    assert_eq!(compat::update_note("0.3.0", "v0.3.0", "0.3.1"), None);
    assert!(compat::update_note("0.2.0", "v0.3.0", "0.3.1")
        .unwrap()
        .ends_with("this update fits it."));
}

#[test]
fn the_notice_for_unreadable_and_newer_apps() {
    // an unreadable app version (a development build) is not blocked
    assert_eq!(compat::notice("", "0.2.0"), None);
    assert_eq!(compat::notice("dev", "0.2.0"), None);
    // from 1.0, an app newer than the server: upgrade it, or the server's minor
    let newer = compat::notice("1.6.0", "1.5.0").expect("notice");
    assert!(newer.contains("upgrade the server"), "{newer}");
    assert!(newer.contains("install TeamCraft Desktop 1.5.x"), "{newer}");
}
