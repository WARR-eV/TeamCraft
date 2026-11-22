//! The component picker of the BOM (#419): `GET /items/suggest` (web, session
//! cookie) and `GET /api/v1/items/suggest` (REST) find items by part of the
//! number, name or file name, with the user's read rights – items of a
//! restricted group the user does not belong to never appear – and adding a
//! line still needs the user's own check-out, through the web form and the API.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use teamcraft::domain::Role;

macro_rules! app_or_skip {
    () => {
        match common::setup().await {
            Some(app) => app,
            None => return,
        }
    };
}

/// Numbers of the suggestions in a picker answer (`{"items": [...]}`).
fn numbers(body: &Value) -> Vec<String> {
    body["items"]
        .as_array()
        .unwrap_or_else(|| panic!("no items in {body}"))
        .iter()
        .map(|i| i["number"].as_str().unwrap().to_string())
        .collect()
}

async fn web_suggest(app: &common::TestApp, cookie: &str, q: &str) -> Value {
    let r = app
        .get_page(
            &format!("/items/suggest?q={}", teamcraft::util::pct_encode(q)),
            Some(cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "suggest {q}: {}", r.text());
    r.json()
}

async fn api_suggest(app: &common::TestApp, token: &str, q: &str) -> Value {
    let r = app
        .api(
            Method::GET,
            &format!("/api/v1/items/suggest?q={}", teamcraft::util::pct_encode(q)),
            token,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "api suggest {q}: {}", r.text());
    r.json()
}

#[tokio::test]
async fn suggestions_match_part_of_the_name_and_of_the_file_name() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["ROCKET"]).await;
    let bracket = app
        .create_item(
            &eng,
            json!({ "item_type": "part", "name": "Fin bracket", "project": "ROCKET" }),
        )
        .await;
    let cone = app
        .create_item(
            &eng,
            json!({ "item_type": "part", "name": "Tip", "project": "ROCKET" }),
        )
        .await;
    let r = app
        .api_upload(
            &format!("/api/v1/items/{cone}/revisions/A/files"),
            &eng,
            "nosecone-ogive.txt",
            b"ogive",
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let cookie = app.login("eng").await;

    // part of the name
    let found = web_suggest(&app, &cookie, "bRACk").await;
    assert_eq!(numbers(&found), vec![bracket.clone()], "{found}");
    let row = &found["items"][0];
    assert_eq!(row["name"], "Fin bracket");
    assert_eq!(row["latest_code"], "A");
    assert_eq!(row["latest_state"], "in_work");
    assert_eq!(row["project_code"], "ROCKET");
    assert!(row["thumbnail_url"].is_null(), "no preview image: {row}");

    // part of the file name, with the file that matched
    let found = web_suggest(&app, &cookie, "nosecone-og").await;
    assert_eq!(numbers(&found), vec![cone.clone()], "{found}");
    assert_eq!(found["items"][0]["matched_file"], "nosecone-ogive.txt");

    // part of the number
    let digits = bracket.rsplit('-').next().unwrap();
    assert!(numbers(&web_suggest(&app, &cookie, digits).await).contains(&bracket));

    // the REST twin answers the same
    let found = api_suggest(&app, &eng, "nosecone").await;
    assert_eq!(numbers(&found), vec![cone.clone()], "{found}");
    assert!(found["items"][0]["thumbnail_file_id"].is_null());

    // fewer than 2 characters: nothing; the limit is capped at 10
    assert!(numbers(&web_suggest(&app, &cookie, "b").await).is_empty());
    assert!(numbers(&api_suggest(&app, &eng, " ").await).is_empty());
    for i in 0..12 {
        app.create_item(
            &eng,
            json!({ "item_type": "part", "name": format!("Washer {i}"), "project": "ROCKET" }),
        )
        .await;
    }
    let r = app
        .get_page("/items/suggest?q=washer&limit=500", Some(&cookie))
        .await;
    assert_eq!(numbers(&r.json()).len(), 10, "{}", r.text());
    let r = app
        .api(
            Method::GET,
            "/api/v1/items/suggest?q=washer&limit=3",
            &eng,
            None,
        )
        .await;
    assert_eq!(numbers(&r.json()).len(), 3, "{}", r.text());
    app.teardown().await;
}

#[tokio::test]
async fn items_of_a_restricted_group_never_appear_for_outsiders() {
    let app = app_or_skip!();
    let (_, erika) = app.user("erika", Role::Editor).await;
    let (_, vera) = app.user_in("vera", Role::Editor, &["ROCKET"]).await;
    let r = app
        .api(
            Method::POST,
            "/api/v1/projects",
            &erika,
            Some(json!({ "code": "NTK", "name": "Need to know", "visibility": "restricted" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let secret = app
        .create_item(
            &erika,
            json!({ "item_type": "part", "name": "Secret bracket", "project": "NTK" }),
        )
        .await;
    let r = app
        .api_upload(
            &format!("/api/v1/items/{secret}/revisions/A/files"),
            &erika,
            "secret-bracket.txt",
            b"secret",
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let open = app
        .create_item(
            &vera,
            json!({ "item_type": "part", "name": "Open bracket", "project": "ROCKET" }),
        )
        .await;

    // the member finds it
    let erika_cookie = app.login("erika").await;
    assert!(numbers(&web_suggest(&app, &erika_cookie, "secret").await).contains(&secret));

    // the outsider does not – neither by name, file name nor number
    let cookie = app.login("vera").await;
    for q in ["bracket", "secret", "secret-bracket.txt", secret.as_str()] {
        let web = web_suggest(&app, &cookie, q).await;
        assert!(!numbers(&web).contains(&secret), "web {q}: {web}");
        assert!(!web.to_string().contains("Secret"), "web {q}: {web}");
        let api = api_suggest(&app, &vera, q).await;
        assert!(!numbers(&api).contains(&secret), "api {q}: {api}");
    }
    assert_eq!(
        numbers(&web_suggest(&app, &cookie, "bracket").await),
        vec![open]
    );
    app.teardown().await;
}

#[tokio::test]
async fn suggestions_need_a_signed_in_user() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["ROCKET"]).await;
    let number = app
        .create_item(
            &eng,
            json!({ "item_type": "part", "name": "Fin bracket", "project": "ROCKET" }),
        )
        .await;
    // no session: refused (redirect to the sign-in or 401), nothing listed
    let r = app.get_page("/items/suggest?q=bracket", None).await;
    assert_ne!(r.status, StatusCode::OK, "{}", r.text());
    assert!(!r.text().contains(&number), "{}", r.text());
    // the web route takes the session cookie only, never a bearer token
    let r = app
        .send(
            axum::http::Request::builder()
                .uri("/items/suggest?q=bracket")
                .header(axum::http::header::AUTHORIZATION, format!("Bearer {eng}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await;
    assert_ne!(r.status, StatusCode::OK, "{}", r.text());
    assert!(!r.text().contains(&number));
    // the REST route needs a token
    let r = app
        .api(
            Method::GET,
            "/api/v1/items/suggest?q=bracket",
            "tc_pat_invalid",
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{}", r.text());
    app.teardown().await;
}

#[tokio::test]
async fn a_line_needs_my_check_out_through_the_web_form_and_the_api() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROCKET"]).await;
    let (_, vera) = app.user_in("vera", Role::Editor, &["ROCKET"]).await;
    let parent = app
        .create_item(
            &erika,
            json!({ "item_type": "assembly", "name": "Fin can", "project": "ROCKET" }),
        )
        .await;
    let child = app
        .create_item(
            &erika,
            json!({ "item_type": "part", "name": "Fin bracket", "project": "ROCKET" }),
        )
        .await;
    let rev_uri = format!("/api/v1/items/{parent}/revisions/A");
    let rev_id = app.api(Method::GET, &rev_uri, &erika, None).await.json()["revision"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let bom_len = |token: String| {
        let app = &app;
        let rev_uri = rev_uri.clone();
        async move {
            app.api(Method::GET, &rev_uri, &token, None).await.json()["bom"]
                .as_array()
                .map_or(0, Vec::len)
        }
    };
    let api_add = |token: String| {
        let app = &app;
        let uri = format!("{rev_uri}/bom");
        let child = child.clone();
        async move {
            app.api(
                Method::POST,
                &uri,
                &token,
                Some(json!({ "child": child, "quantity": 2 })),
            )
            .await
        }
    };
    let web_add = |user: &'static str| {
        let app = &app;
        let uri = format!("/revisions/{rev_id}/bom/lines");
        let child = child.clone();
        async move {
            let cookie = app.login(user).await;
            app.post_form(
                &uri,
                Some(&cookie),
                Some("http://localhost:8080"),
                &[
                    ("child", child.as_str()),
                    ("quantity", "2"),
                    ("find_number", ""),
                    ("notes", ""),
                ],
            )
            .await
        }
    };

    // A new item is checked out to its author: release it first.
    let r = app
        .api(
            Method::POST,
            &format!("{rev_uri}/cancel-checkout"),
            &erika,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    // not checked out by anyone: refused
    let r = api_add(erika.clone()).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    let r = web_add("erika").await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.text());
    assert_eq!(
        bom_len(erika.clone()).await,
        0,
        "the web form added a line without a check-out"
    );

    // checked out by erika: vera is refused through both
    let r = app
        .api(Method::POST, &format!("{rev_uri}/checkout"), &erika, None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let r = api_add(vera.clone()).await;
    assert!(r.status.is_client_error(), "{} {}", r.status, r.text());
    let r = web_add("vera").await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.text());
    assert_eq!(
        bom_len(erika.clone()).await,
        0,
        "vera added a line to erika's check-out"
    );

    // erika's own check-out: the web form adds the line
    let r = web_add("erika").await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.text());
    assert_eq!(bom_len(erika.clone()).await, 1);
    app.teardown().await;
}
