//! Items without a project group (#310). Visibility is a deny list, so an
//! item without a group is readable by everyone: new items and moves
//! therefore always need a group, administrators see how many such items
//! are left from older versions (with a link to them), and moving one into a
//! group closes it like any other item.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;
use teamcraft::{domain::Role, services::projects};

macro_rules! app_or_skip {
    () => {
        match common::setup().await {
            Some(app) => app,
            None => return,
        }
    };
}

#[tokio::test]
async fn new_items_and_moves_need_a_project_group() {
    let app = app_or_skip!();
    let (_, token) = app.user_in("erika", Role::Editor, &["SHOP"]).await;

    // a new item without a group is refused, and nothing is created
    for body in [
        json!({ "item_type": "part", "name": "Loose bracket" }),
        json!({ "item_type": "part", "name": "Loose bracket", "project": "" }),
    ] {
        let r = app
            .api(Method::POST, "/api/v1/items", &token, Some(body.clone()))
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}: {}", r.text());
        assert!(r.text().contains("project group"), "{}", r.text());
    }
    let r = app
        .api(Method::GET, "/api/v1/items?q=loose", &token, None)
        .await;
    assert_eq!(r.json()["total"], 0, "{}", r.text());
    assert_eq!(projects::count_ungrouped_items(&app.pool).await.unwrap(), 0);

    // an item from an older version that has no group
    let number = app
        .create_item(
            &token,
            json!({ "item_type": "part", "name": "Old bracket", "project": "SHOP" }),
        )
        .await;
    sqlx::query("UPDATE items SET project_id = NULL WHERE number = $1")
        .bind(&number)
        .execute(&app.pool)
        .await
        .unwrap();
    assert_eq!(projects::count_ungrouped_items(&app.pool).await.unwrap(), 1);

    // a change of its master data that does not name a group is refused:
    // it cannot stay without one
    let r = app
        .api(
            Method::PATCH,
            &format!("/api/v1/items/{number}"),
            &token,
            Some(json!({ "classification": "purchased" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert!(r.text().contains("project group"), "{}", r.text());
    assert_eq!(projects::count_ungrouped_items(&app.pool).await.unwrap(), 1);

    // moved into a group, it is counted no more
    let r = app
        .api(
            Method::PATCH,
            &format!("/api/v1/items/{number}"),
            &token,
            Some(json!({ "project": "SHOP" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(projects::count_ungrouped_items(&app.pool).await.unwrap(), 0);
    app.teardown().await;
}

#[tokio::test]
async fn administrators_are_shown_the_items_without_a_group() {
    let app = app_or_skip!();
    let (_, token) = app.user_in("erika", Role::Editor, &["SHOP"]).await;
    app.user("ada", Role::Admin).await;
    let admin = app.login("ada").await;
    let editor = app.login("erika").await;

    // nothing to point out while every item has a group
    let page = app.get_page("/projects", Some(&admin)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.text());
    assert!(!page.text().contains("id=\"ungrouped-items\""));

    let number = app
        .create_item(
            &token,
            json!({ "item_type": "part", "name": "Old bracket", "project": "SHOP" }),
        )
        .await;
    sqlx::query("UPDATE items SET project_id = NULL WHERE number = $1")
        .bind(&number)
        .execute(&app.pool)
        .await
        .unwrap();

    // administrators see the count and the link to the list …
    let page = app.get_page("/projects", Some(&admin)).await;
    let text = page.text();
    assert!(text.contains("id=\"ungrouped-items\""), "{text}");
    assert!(text.contains("1 item has no project group"), "{text}");
    assert!(text.contains("href=\"/vault?folder=none\""), "{text}");
    // … other users do not
    let page = app.get_page("/projects", Some(&editor)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.text());
    assert!(!page.text().contains("id=\"ungrouped-items\""));

    // as the permissions guide warns, such an item is open to everyone: a
    // viewer who is a member of no group reads it
    let (_, viewer) = app.user("vera", Role::Viewer).await;
    let r = app
        .api(
            Method::GET,
            &format!("/api/v1/items/{number}"),
            &viewer,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    app.teardown().await;
}
