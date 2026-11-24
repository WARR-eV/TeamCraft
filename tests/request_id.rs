//! Request ids (#381): every answer carries `x-request-id`, and a web error
//! page shows the same id in its *Copy details* text.

mod common;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};

#[tokio::test]
async fn an_error_page_shows_the_id_of_its_response_header() {
    let Some(app) = common::setup().await else {
        return;
    };
    // a 404 carries the header but offers no bug report (#524)
    let page = app.get_page("/branding/banner", None).await;
    assert_eq!(page.status, StatusCode::NOT_FOUND);
    let id = page.header("x-request-id").expect("x-request-id header");
    assert!(uuid::Uuid::parse_str(&id).is_ok(), "{id}");
    assert!(
        !page.text().contains("Details for a bug report") && !page.text().contains("Request id:"),
        "{}",
        page.text()
    );

    // a reverse proxy's id is kept; a forged log line is not
    let with = |value: &str| {
        Request::builder()
            .method(Method::GET)
            .uri("/branding/banner")
            .header("x-request-id", value)
            .body(Body::empty())
            .unwrap()
    };
    let kept = app.send(with("edge-7f3a")).await;
    assert_eq!(kept.header("x-request-id").as_deref(), Some("edge-7f3a"));
    let replaced = app.send(with("x y\tz")).await;
    let id = replaced.header("x-request-id").unwrap();
    assert!(uuid::Uuid::parse_str(&id).is_ok(), "{id}");

    // the JSON API keeps its error shape and gets the header too
    let api = app.get_page("/api/v1/items", None).await;
    assert_eq!(api.status, StatusCode::UNAUTHORIZED);
    assert!(api.header("x-request-id").is_some());
    assert!(app
        .get_page("/healthz", None)
        .await
        .header("x-request-id")
        .is_some());
    app.teardown().await;
}

/// #524: a signed-in user's 404 is a page of the app (top bar, a way back)
/// without *Details for a bug report*; a server error keeps the details with
/// the request id of its response header.
#[tokio::test]
async fn only_server_errors_offer_a_bug_report() {
    let Some(app) = common::setup().await else {
        return;
    };
    app.user("ada", teamcraft::domain::Role::Editor).await;
    let cookie = app.login("ada").await;

    let missing = format!("/items/{}", uuid::Uuid::new_v4());
    let page = app.get_page(&missing, Some(&cookie)).await;
    assert_eq!(page.status, StatusCode::NOT_FOUND);
    let html = page.text();
    assert!(html.contains("class=\"topbar\""), "app shell: {html}");
    assert!(html.contains("class=\"error-page\"") && html.contains("data-back"));
    assert!(!html.contains("Details for a bug report"), "{html}");
    // signed out: the sign-in page first, as before
    let page = app.get_page(&missing, None).await;
    assert_eq!(page.status, StatusCode::SEE_OTHER, "sign in first");

    // break the database under the running app: the item list fails inside
    sqlx::query("DROP TABLE bom_lines, items CASCADE")
        .execute(&app.pool)
        .await
        .unwrap();
    let page = app.get_page("/items", Some(&cookie)).await;
    assert_eq!(page.status, StatusCode::INTERNAL_SERVER_ERROR);
    let id = page.header("x-request-id").expect("x-request-id header");
    let html = page.text();
    assert!(html.contains("Details for a bug report"), "{html}");
    assert!(html.contains(&format!("Request id: {id}")), "{html}");
    assert!(
        !html.contains("class=\"topbar\""),
        "server errors stay plain"
    );
    app.teardown().await;
}
