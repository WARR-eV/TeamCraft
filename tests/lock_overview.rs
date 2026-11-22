//! Check-out overview of a project group (#428): who may see it
//! (administrators and the group's managers; 403 for members and viewers,
//! 404 for a group the actor cannot see) and what it lists – the locks
//! grouped per holder (REST), drawn as a graph on the web page.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;
use sqlx::PgPool;
use teamcraft::domain::Role;
use uuid::Uuid;

macro_rules! app_or_skip {
    () => {
        match common::setup().await {
            Some(app) => app,
            None => return,
        }
    };
}

/// A new part in ROVER – new items are checked out to their author.
async fn new_part(app: &common::TestApp, token: &str, name: &str) {
    let r = app
        .api(
            Method::POST,
            "/api/v1/items",
            token,
            Some(json!({ "item_type": "part", "name": name, "project": "ROVER" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
}

async fn make_manager(pool: &PgPool, project: Uuid, user: Uuid) {
    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, manager) VALUES ($1, $2, TRUE)
         ON CONFLICT (project_id, user_id) DO UPDATE SET manager = TRUE",
    )
    .bind(project)
    .bind(user)
    .execute(pool)
    .await
    .unwrap();
}

const PATH: &str = "/api/v1/projects/ROVER/checkouts";

#[tokio::test]
async fn admins_and_managers_see_who_holds_which_lock() {
    let app = app_or_skip!();
    let (_, root) = app.user("root", Role::Admin).await;
    let (_, leo) = app.user_in("leo", Role::Editor, &["ROVER"]).await;
    let (erika_actor, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let (_, vera) = app.user_in("vera", Role::Viewer, &["ROVER"]).await;
    new_part(&app, &leo, "Bracket").await;
    new_part(&app, &leo, "Plate").await;
    new_part(&app, &erika, "Mast").await;

    let r = app.api(Method::GET, PATH, &root, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let j = r.json();
    assert_eq!(j["project_code"], "ROVER");
    assert_eq!(j["total"], 3);
    let holders = j["holders"].as_array().unwrap();
    // one entry per holder, by name: erika before leo
    let names: Vec<&str> = holders
        .iter()
        .map(|h| h["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["erika (test)", "leo (test)"]);
    let counts: Vec<usize> = holders
        .iter()
        .map(|h| h["locks"].as_array().unwrap().len())
        .collect();
    assert_eq!(counts, vec![1, 2]);
    let lock = &holders[1]["locks"][0];
    assert_eq!(lock["code"], "A");
    assert_eq!(lock["item_type"], "part");
    assert_eq!(lock["holder_active"], true);
    assert!(lock["item_number"].as_str().unwrap().starts_with("PRT-"));

    // members and viewers do not see it; a manager does
    assert_eq!(
        app.api(Method::GET, PATH, &leo, None).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.api(Method::GET, PATH, &vera, None).await.status,
        StatusCode::FORBIDDEN
    );
    let rover = app.project_id("ROVER").await;
    make_manager(&app.pool, rover, erika_actor.id).await;
    // GET /me names the managed groups, so the desktop app offers the view
    let me = app
        .api(Method::GET, "/api/v1/me", &erika, None)
        .await
        .json();
    assert_eq!(me["managed_projects"], json!([rover]));
    let me = app.api(Method::GET, "/api/v1/me", &leo, None).await.json();
    assert_eq!(me["managed_projects"], json!([]));
    let r = app.api(Method::GET, PATH, &erika, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["total"], 3);

    // the web page: the graph for the admin, refused for a plain member
    let page = format!("/projects/{rover}/checkouts");
    let root_cookie = app.login("root").await;
    let r = app.get_page(&page, Some(&root_cookie)).await;
    assert_eq!(r.status, StatusCode::OK);
    let html = r.text();
    assert!(html.contains("class=\"lock-graph\""), "{html}");
    assert!(html.contains("3 revisions held by 2 people"), "{html}");
    let leo_cookie = app.login("leo").await;
    assert_eq!(
        app.get_page(&page, Some(&leo_cookie)).await.status,
        StatusCode::FORBIDDEN
    );
    // the group page links the overview for those who may see it
    let html = app
        .get_page(&format!("/projects/{rover}"), Some(&root_cookie))
        .await
        .text();
    assert!(html.contains(&format!("/projects/{rover}/checkouts")));
}

#[tokio::test]
async fn an_empty_group_and_a_hidden_group() {
    let app = app_or_skip!();
    let (_, root) = app.user("root", Role::Admin).await;
    let (_, olli) = app.user("olli", Role::Editor).await;

    let r = app.api(Method::GET, PATH, &root, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["total"], 0);
    assert_eq!(r.json()["holders"], json!([]));

    // a restricted group answers like a missing one to an outsider
    sqlx::query("UPDATE projects SET visibility = 'restricted' WHERE code = 'ROVER'")
        .execute(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        app.api(Method::GET, PATH, &olli, None).await.status,
        StatusCode::NOT_FOUND
    );
}
