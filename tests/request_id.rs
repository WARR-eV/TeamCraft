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
    let page = app.get_page("/branding/banner", None).await;
    assert_eq!(page.status, StatusCode::NOT_FOUND);
    let id = page.header("x-request-id").expect("x-request-id header");
    assert!(uuid::Uuid::parse_str(&id).is_ok(), "{id}");
    assert!(
        page.text().contains(&format!("Request id: {id}")),
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
    assert!(kept.text().contains("Request id: edge-7f3a"));
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
