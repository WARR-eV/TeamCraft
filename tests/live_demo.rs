//! A hosted live demo (`TEAMCRAFT_DEMO_USER`, `TEAMCRAFT_DEMO_PASSWORD`):
//! the public account signs in and reads everything it may read, but every
//! request that could change something is refused – in the web UI and in the
//! REST API – while signing out still works and other accounts are not
//! affected. The login page shows the account, every page the banner, and a
//! stranger's wrong passwords cannot lock the public account.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;
use teamcraft::domain::Role;

const ORIGIN: &str = "http://localhost:8080";

async fn demo_app() -> Option<common::TestApp> {
    common::setup_with(&[
        ("TEAMCRAFT_DEMO_USER", "demo"),
        ("TEAMCRAFT_DEMO_PASSWORD", common::PASSWORD),
    ])
    .await
}

#[tokio::test]
async fn the_demo_account_reads_but_changes_nothing() {
    let Some(app) = demo_app().await else {
        return;
    };
    let (_, demo_token) = app.user_in("demo", Role::Editor, &["ROCKET"]).await;
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROCKET"]).await;
    let part = app
        .create_item(
            &erika,
            json!({ "item_type": "part", "name": "Fin", "project": "ROCKET" }),
        )
        .await;

    // the login page names the account; every page carries the banner
    let login = app.get_page("/login", None).await;
    assert_eq!(login.status, StatusCode::OK);
    let text = login.text();
    assert!(text.contains("Sign in to the live demo as"), "{text}");
    assert!(text.contains(common::PASSWORD) && text.contains("demo-banner"));

    // reading works, in the browser and over the API
    let cookie = app.login("demo").await;
    let page = app.get_page("/items", Some(&cookie)).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.text().contains("demo-banner"));
    let uri = format!("/api/v1/items/{part}");
    let r = app.api(Method::GET, &uri, &demo_token, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    // every change is refused: a form post, an API call, the own password
    let r = app
        .post_form(
            "/account/tokens",
            Some(&cookie),
            Some(ORIGIN),
            &[("name", "mine"), ("expires_days", "7")],
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());
    assert!(r.text().contains("read-only live demo"), "{}", r.text());
    let body = json!({ "item_type": "part", "name": "Mine", "project": "ROCKET" });
    let r = app
        .api(Method::POST, "/api/v1/items", &demo_token, Some(body))
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());
    assert!(r.text().contains("read-only live demo"), "{}", r.text());
    // even a harmless write of the own account: watching an item
    let watch = format!("{uri}/watch");
    let r = app.api(Method::POST, &watch, &demo_token, None).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());
    let r = app
        .post_form(
            "/account/password",
            Some(&cookie),
            Some(ORIGIN),
            &[
                ("current_password", common::PASSWORD),
                ("new_password", "taken-over-demo"),
                ("confirm_password", "taken-over-demo"),
            ],
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());
    let unchanged =
        teamcraft::services::users::authenticate_local(&app.pool, "demo", common::PASSWORD)
            .await
            .unwrap();
    assert!(unchanged.is_some(), "the public password still works");
    let tokens: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM api_tokens t JOIN users u ON u.id = t.user_id WHERE u.username = 'demo'",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(tokens, 1, "only the test's own token");

    // other accounts are not affected
    let body = json!({ "item_type": "part", "name": "Nozzle", "project": "ROCKET" });
    let r = app
        .api(Method::POST, "/api/v1/items", &erika, Some(body))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());

    // signing out still works
    let r = app
        .post_form("/logout", Some(&cookie), Some(ORIGIN), &[])
        .await;
    assert!(r.status.is_redirection(), "{} {}", r.status, r.text());
    let page = app.get_page("/items", Some(&cookie)).await;
    assert!(page.status.is_redirection(), "signed out: {}", page.status);
    app.teardown().await;
}

/// The per-username sign-in throttle does not apply to the public account –
/// otherwise anybody could lock every visitor out with wrong passwords – but
/// it still applies to every other account.
#[tokio::test]
async fn wrong_passwords_do_not_lock_the_demo_account() {
    let Some(app) = demo_app().await else {
        return;
    };
    app.user_in("demo", Role::Editor, &["ROCKET"]).await;
    app.user_in("erika", Role::Editor, &["ROCKET"]).await;
    let attempt = |username: &'static str, password: &'static str| {
        let app = &app;
        async move {
            app.post_form(
                "/login",
                None,
                Some(ORIGIN),
                &[
                    ("username", username),
                    ("password", password),
                    ("next", "/"),
                ],
            )
            .await
            .status
        }
    };
    for _ in 0..12 {
        assert_eq!(
            attempt("demo", "wrong-password").await,
            StatusCode::UNAUTHORIZED
        );
        attempt("erika", "wrong-password").await;
    }
    assert_eq!(
        attempt("demo", common::PASSWORD).await,
        StatusCode::SEE_OTHER,
        "the demo account still signs in"
    );
    assert_eq!(
        attempt("erika", common::PASSWORD).await,
        StatusCode::TOO_MANY_REQUESTS,
        "other accounts are throttled as before"
    );
    app.teardown().await;
}
