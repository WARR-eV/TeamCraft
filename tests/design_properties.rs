//! Design properties of SolidCraft designs (issue #367, migration 0078):
//! read at check-in, listed with the file, found by the full-text search,
//! compared with the revision on its page (a hint that goes away when the
//! item or the file changes) and used for an empty name of a new item.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;
use teamcraft::domain::Role;
use teamcraft::scdesign::sample;

macro_rules! app_or_skip {
    () => {
        match common::setup().await {
            Some(app) => app,
            None => return,
        }
    };
}

const ORIGIN: &str = "http://localhost:8080";

/// A ZIP design with the given metadata object.
fn design(metadata: serde_json::Value) -> Vec<u8> {
    let document = json!({ "format": "solidcraft/46", "metadata": metadata, "features": [] });
    sample::design(document.to_string().as_bytes(), None)
}

/// The file `name` of a revision as the API lists it.
async fn file_of(
    app: &common::TestApp,
    token: &str,
    number: &str,
    name: &str,
) -> serde_json::Value {
    let r = app
        .api(
            Method::GET,
            &format!("/api/v1/items/{number}/revisions/A/files"),
            token,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.json()
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["filename"] == name)
        .unwrap_or_else(|| panic!("no {name}: {}", r.text()))
        .clone()
}

#[tokio::test]
async fn design_properties_are_listed_found_and_compared_with_the_item() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROCKET"]).await;
    let cookie = app.login("erika").await;
    let number = app
        .create_item(
            &erika,
            json!({ "item_type": "part", "number": "BR-7731", "name": "Holder", "project": "ROCKET" }),
        )
        .await;
    assert_eq!(number, "BR-7731");
    let v1 = design(json!({
        "part_number": "BR-7731",
        "revision": "A",
        "description": "Zylonwerk bracket",
        "organization": "Quarzhalle",
        "author": 42,
        "org.example.tool": { "part_number": "ignored" }
    }));
    let checkin = format!("/api/v1/items/{number}/revisions/A/checkin");
    let r = app
        .api_multipart(
            &checkin,
            &erika,
            &[
                ("bracket.sc", &v1),
                // a damaged design is stored like any file, without properties
                ("broken.sc", b"PK\x03\x04garbage"),
            ],
            &[("comment", "design")],
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    // listed with the file
    let file = file_of(&app, &erika, &number, "bracket.sc").await;
    assert_eq!(file["format"], "solidcraft-design");
    assert_eq!(file["properties"]["part_number"], "BR-7731");
    assert_eq!(file["properties"]["organization"], "Quarzhalle");
    assert!(
        file["properties"].get("author").is_none(),
        "not text: {file}"
    );
    assert!(file["properties"].get("org.example.tool").is_none());
    assert!(file["properties_line"]
        .as_str()
        .unwrap()
        .starts_with("Part number BR-7731 · Revision A · Description Zylonwerk bracket"));
    let broken = file_of(&app, &erika, &number, "broken.sc").await;
    assert!(broken.get("properties").is_none(), "{broken}");
    assert!(broken.get("properties_line").is_none());

    // found by the full-text search (only the design names "Quarzhalle")
    let r = app
        .api(Method::GET, "/api/v1/items?q=quarzhalle", &erika, None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["total"], 1, "{}", r.text());
    assert_eq!(r.json()["items"][0]["number"], "BR-7731");

    // the description differs from the name: a hint on the revision page
    let revision_id = file["revision_id"].as_str().unwrap().to_string();
    let page_url = format!("/revisions/{revision_id}");
    let page = app.get_page(&page_url, Some(&cookie)).await;
    assert_eq!(page.status, StatusCode::OK);
    let html = page.text();
    assert!(html.contains("Part number BR-7731"), "listed on the page");
    assert!(html.contains("Design properties differ from this revision"));
    assert!(html.contains("Zylonwerk bracket"));
    assert!(
        !html.contains("≠ item number"),
        "the part number agrees with the item number"
    );
    // the same hint in the REST API (the desktop revision view)
    let r = app
        .api(
            Method::GET,
            &format!("/api/v1/items/{number}/revisions/A"),
            &erika,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let found = r.json()["property_mismatches"].clone();
    assert_eq!(found.as_array().map(Vec::len), Some(1), "{found}");
    assert_eq!(found[0]["filename"], "bracket.sc");
    assert_eq!(found[0]["mismatches"][0]["key"], "description");
    assert_eq!(found[0]["mismatches"][0]["file"], "Zylonwerk bracket");

    // renaming the item makes the hint go away – nothing was changed for it
    let rev_url = format!("/api/v1/items/{number}/revisions/A");
    let r = app
        .api(Method::POST, &format!("{rev_url}/checkout"), &erika, None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let r = app
        .api(
            Method::PATCH,
            &rev_url,
            &erika,
            Some(json!({ "name": "Zylonwerk Bracket" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let html = app.get_page(&page_url, Some(&cookie)).await.text();
    assert!(
        !html.contains("Design properties differ from this revision"),
        "case does not count"
    );
    let r = app.api(Method::GET, &rev_url, &erika, None).await;
    assert_eq!(r.json()["property_mismatches"], json!([]), "{}", r.text());

    // a new version of the design with another revision: the hint is back
    let v2 = design(json!({
        "part_number": "BR-7731",
        "revision": "C",
        "description": "Zylonwerk bracket"
    }));
    let r = app
        .api_multipart(&checkin, &erika, &[("bracket.sc", &v2)], &[])
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let file = file_of(&app, &erika, &number, "bracket.sc").await;
    assert_eq!(file["version"], 2);
    assert_eq!(file["properties"]["revision"], "C");
    assert!(file["properties"].get("organization").is_none());
    let html = app.get_page(&page_url, Some(&cookie)).await.text();
    assert!(html.contains("Design properties differ from this revision"));
    assert!(html.contains("≠ revision"), "{html}");
    // the old organization is no longer found: only the current version counts
    let r = app
        .api(Method::GET, "/api/v1/items?q=quarzhalle", &erika, None)
        .await;
    assert_eq!(r.json()["total"], 0, "{}", r.text());
    app.teardown().await;
}

/// Multipart form POST like a browser (session cookie, same origin).
async fn post_multipart(
    app: &common::TestApp,
    uri: &str,
    cookie: &str,
    file: (&str, &[u8]),
    fields: &[(&str, &str)],
) -> common::Response {
    use axum::{body::Body, http::header, http::Request};
    let boundary = "teamcraft-design-boundary";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: application/octet-stream\r\n\r\n",
            file.0
        )
        .as_bytes(),
    );
    body.extend_from_slice(file.1);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::HOST, "localhost:8080")
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, cookie)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    app.send(request).await
}

#[tokio::test]
async fn a_new_item_without_a_name_takes_the_design_description() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROCKET"]).await;
    let cookie = app.login("erika").await;
    let project = app.project_id("ROCKET").await.to_string();
    let plate = design(json!({ "description": "Prefilled plate", "part_number": "PL-1" }));
    let r = post_multipart(
        &app,
        "/items",
        &cookie,
        ("plate.sc", &plate),
        &[
            ("item_type", "part"),
            ("project_id", &project),
            ("name", ""),
        ],
    )
    .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.text());
    let r = app
        .api(Method::GET, "/api/v1/items?q=prefilled", &erika, None)
        .await;
    assert_eq!(r.json()["total"], 1, "{}", r.text());
    assert_eq!(r.json()["items"][0]["name"], "Prefilled plate");

    // a typed name stays; the empty description is filled instead
    let r = post_multipart(
        &app,
        "/items",
        &cookie,
        ("plate2.sc", &plate),
        &[
            ("item_type", "part"),
            ("project_id", &project),
            ("name", "Typed name"),
        ],
    )
    .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.text());
    let r = app
        .api(Method::GET, "/api/v1/items?q=typed", &erika, None)
        .await;
    assert_eq!(r.json()["total"], 1, "{}", r.text());
    let number = r.json()["items"][0]["number"].as_str().unwrap().to_string();
    let r = app
        .api(
            Method::GET,
            &format!("/api/v1/items/{number}/revisions/A"),
            &erika,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let rev = r.json();
    let rev = rev.get("revision").cloned().unwrap_or(rev);
    assert_eq!(rev["name"], "Typed name");
    assert_eq!(rev["description"], "Prefilled plate");
    app.teardown().await;
}
