//! Functional tests of the item list: filters by type and state (of the
//! latest revision), paging with totals, LIKE wildcards in the query taken
//! literally, and refused unknown filter values.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;
use teamcraft::domain::Role;

macro_rules! app_or_skip {
    () => {
        match common::setup().await {
            Some(app) => app,
            None => return,
        }
    };
}

fn numbers(r: &common::Response) -> Vec<String> {
    let mut out: Vec<String> = r.json()["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{}", r.text()))
        .iter()
        .map(|i| i["number"].as_str().unwrap().to_string())
        .collect();
    out.sort();
    out
}

#[tokio::test]
async fn filters_paging_and_literal_wildcards() {
    let app = app_or_skip!();
    let (_, token) = app.user_in("erika", Role::Editor, &["SHOP"]).await;
    let (_, reviewer) = app.user("rolf", Role::Editor).await;
    let mut parts = Vec::new();
    for name in ["Clamp 100% steel", "Clamp 10x steel", "Clamp_a", "Clampa"] {
        parts.push(
            app.create_item(
                &token,
                json!({ "item_type": "part", "name": name, "project": "SHOP" }),
            )
            .await,
        );
    }
    let tool = app
        .create_item(
            &token,
            json!({ "item_type": "assembly", "name": "Clamp tool", "project": "SHOP" }),
        )
        .await;
    app.release(&token, ("rolf", &reviewer), &[&parts[0]]).await;

    // by type
    let r = app
        .api(Method::GET, "/api/v1/items?type=assembly", &token, None)
        .await;
    assert_eq!(numbers(&r), std::slice::from_ref(&tool));
    let r = app
        .api(Method::GET, "/api/v1/items?type=part&q=clamp", &token, None)
        .await;
    assert_eq!(r.json()["total"], 4);

    // by the state of the latest revision
    let r = app
        .api(Method::GET, "/api/v1/items?state=released", &token, None)
        .await;
    assert_eq!(numbers(&r), [parts[0].clone()]);
    let r = app
        .api(Method::GET, "/api/v1/items?state=in_work", &token, None)
        .await;
    assert_eq!(r.json()["total"], 4);
    // after a new revision the item is in work again
    let r = app
        .api(
            Method::POST,
            &format!("/api/v1/items/{}/revisions/A/revise", parts[0]),
            &token,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let r = app
        .api(Method::GET, "/api/v1/items?state=released", &token, None)
        .await;
    assert_eq!(r.json()["total"], 0, "{}", r.text());

    // % and _ are no wildcards in a search
    let r = app
        .api(Method::GET, "/api/v1/items?q=100%25", &token, None)
        .await;
    assert_eq!(numbers(&r), [parts[0].clone()], "{}", r.text());
    let r = app
        .api(Method::GET, "/api/v1/items?q=clamp_a", &token, None)
        .await;
    assert_eq!(numbers(&r), [parts[2].clone()], "{}", r.text());

    // paging keeps the total and never repeats an item
    let mut seen = Vec::new();
    for offset in [0, 2, 4] {
        let r = app
            .api(
                Method::GET,
                &format!("/api/v1/items?q=clamp&limit=2&offset={offset}"),
                &token,
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        assert_eq!(r.json()["total"], 5);
        assert_eq!(r.json()["limit"], 2);
        assert_eq!(r.json()["offset"], offset);
        seen.extend(numbers(&r));
    }
    seen.sort();
    let mut all = parts.clone();
    all.push(tool.clone());
    all.sort();
    assert_eq!(seen, all);
    let r = app
        .api(
            Method::GET,
            "/api/v1/items?limit=100000&offset=-5",
            &token,
            None,
        )
        .await;
    assert_eq!(r.json()["limit"], 500, "the limit is capped");
    assert_eq!(r.json()["offset"], 0, "negative offsets start at the top");

    // unknown filter values are refused, not ignored
    for query in ["type=gizmo", "state=frozen", "system=nonsense"] {
        let r = app
            .api(Method::GET, &format!("/api/v1/items?{query}"), &token, None)
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{query}: {}", r.text());
    }
    let r = app
        .api(Method::GET, "/api/v1/items?project=NOPE", &token, None)
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    app.teardown().await;
}

/// A query that names a file format ("step") matches the items whose latest
/// revision has a file of that format, besides the usual text matches. The
/// format is checked first for speed (#316); the rows, the total and the
/// visibility guard stay the same.
#[tokio::test]
async fn a_format_name_matches_files_of_that_format() {
    let app = app_or_skip!();
    let (_, token) = app.user_in("erika", Role::Editor, &["SHOP"]).await;
    let (_, other) = app.user_in("otto", Role::Editor, &["SHOP"]).await;
    let mut made = Vec::new();
    for name in ["Bracket", "Plate", "Step ladder"] {
        made.push(
            app.create_item(
                &token,
                json!({ "item_type": "part", "name": name, "project": "SHOP" }),
            )
            .await,
        );
    }
    let (bracket, plate, ladder) = (made[0].clone(), made[1].clone(), made[2].clone());
    for (number, filename, content) in [
        (
            &bracket,
            "bracket.step",
            &b"ISO-10303-21; HEADER; ENDSEC;"[..],
        ),
        (&plate, "plate.txt", &b"flat plate 2 mm"[..]),
    ] {
        let r = app
            .api_upload(
                &format!("/api/v1/items/{number}/revisions/A/files"),
                &token,
                filename,
                content,
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    }
    // a STEP file in a restricted group otto is not a member of
    let r = app
        .api(
            Method::POST,
            "/api/v1/projects",
            &token,
            Some(json!({ "code": "NTK", "name": "Need to know", "visibility": "restricted" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let secret = app
        .create_item(
            &token,
            json!({ "item_type": "part", "name": "Secret", "project": "NTK" }),
        )
        .await;
    let r = app
        .api_upload(
            &format!("/api/v1/items/{secret}/revisions/A/files"),
            &token,
            "secret.stp",
            b"ISO-10303-21; HEADER; /* secret */ ENDSEC;",
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());

    // the STEP file's item and the item named "Step …", nothing else
    let r = app
        .api(Method::GET, "/api/v1/items?q=step", &other, None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let mut expected = vec![bracket.clone(), ladder.clone()];
    expected.sort();
    assert_eq!(numbers(&r), expected, "{}", r.text());
    assert_eq!(r.json()["total"], 2);
    assert!(!numbers(&r).contains(&plate));
    // the member of the restricted group finds its item too
    let r = app
        .api(Method::GET, "/api/v1/items?q=STEP", &token, None)
        .await;
    assert_eq!(r.json()["total"], 3, "{}", r.text());
    assert!(numbers(&r).contains(&secret));
    // paging keeps the total
    let r = app
        .api(Method::GET, "/api/v1/items?q=step&limit=1", &other, None)
        .await;
    assert_eq!(r.json()["total"], 2, "{}", r.text());
    assert_eq!(r.json()["items"].as_array().map(Vec::len), Some(1));
    // a format word combined with a filter that excludes everything
    let r = app
        .api(
            Method::GET,
            "/api/v1/items?q=step&type=assembly",
            &other,
            None,
        )
        .await;
    assert_eq!(r.json()["total"], 0, "{}", r.text());
    app.teardown().await;
}
