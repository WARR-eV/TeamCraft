//! Default CAD program of a project group: managers and administrators set
//! it (REST API and project page), it is validated, audited and part of the
//! group's JSON – TeamCraft Desktop opens the group's CAD files with it
//! (*Check out and open*). The formats list names each format's family.

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

const ORIGIN: Option<&str> = Some("http://localhost:8080");

async fn make_manager(app: &common::TestApp, username: &str, code: &str) {
    sqlx::query(
        "UPDATE project_members SET manager = true
         WHERE project_id = (SELECT id FROM projects WHERE code = $1)
           AND user_id = (SELECT id FROM users WHERE username = $2)",
    )
    .bind(code)
    .bind(username)
    .execute(&app.pool)
    .await
    .unwrap();
}

async fn rover(app: &common::TestApp, token: &str) -> serde_json::Value {
    let list = app
        .api(Method::GET, "/api/v1/projects", token, None)
        .await
        .json();
    list.as_array()
        .unwrap()
        .iter()
        .find(|p| p["code"] == "ROVER")
        .cloned()
        .expect("ROVER listed")
}

#[tokio::test]
async fn managers_set_the_default_cad_program_of_their_group() {
    let app = app_or_skip!();
    let (_, manager) = app.user_in("maria", Role::Editor, &["ROVER"]).await;
    let (_, member) = app.user_in("max", Role::Editor, &["ROVER"]).await;
    make_manager(&app, "maria", "ROVER").await;
    let url = "/api/v1/projects/ROVER/default-cad";

    // no default at first
    let p = rover(&app, &member).await;
    assert_eq!(p["default_cad"], "");
    assert_eq!(p["default_cad_version"], "");

    // a plain member may not change it
    let r = app
        .api(
            Method::PUT,
            url,
            &member,
            Some(json!({ "program": "inventor", "version": "2026" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());

    // the manager may
    let r = app
        .api(
            Method::PUT,
            url,
            &manager,
            Some(json!({ "program": "inventor", "version": " 2026 " })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["default_cad"], "inventor");
    assert_eq!(r.json()["default_cad_version"], "2026");
    let p = rover(&app, &member).await;
    assert_eq!(p["default_cad"], "inventor", "every member sees it");
    assert_eq!(p["default_cad_version"], "2026");
    let id = app.project_id("ROVER").await;
    assert!(app
        .audit_actions(id)
        .await
        .contains(&"project.default_cad".to_string()));

    // unknown programs, neutral groups and odd versions are refused, nothing changes
    for body in [
        json!({ "program": "nonsense" }),
        json!({ "program": "exchange" }),
        json!({ "program": "solidworks", "version": "<script>" }),
        json!({ "program": "solidworks", "version": "9".repeat(33) }),
    ] {
        let r = app
            .api(Method::PUT, url, &manager, Some(body.clone()))
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}: {}", r.text());
    }
    assert_eq!(rover(&app, &member).await["default_cad"], "inventor");

    // removing the program drops the version with it
    let r = app
        .api(
            Method::PUT,
            url,
            &manager,
            Some(json!({ "program": "", "version": "2026" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["default_cad"], "");
    assert_eq!(r.json()["default_cad_version"], "");

    // a group the caller cannot see answers like a missing one
    let r = app
        .api(
            Method::PUT,
            "/api/v1/projects/NOPE/default-cad",
            &manager,
            Some(json!({ "program": "inventor" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    app.teardown().await;
}

#[tokio::test]
async fn the_project_page_shows_and_saves_the_default_cad_program() {
    let app = app_or_skip!();
    app.user("admin", Role::Admin).await;
    app.user_in("max", Role::Editor, &["ROVER"]).await;
    let admin = app.login("admin").await;
    let max = app.login("max").await;
    let id = app.project_id("ROVER").await;
    let page = format!("/projects/{id}");

    let html = app.get_page(&page, Some(&admin)).await.text();
    assert!(html.contains("id=\"default-cad\""));
    assert!(html.contains(&format!("action=\"/projects/{id}/default-cad\"")));

    let r = app
        .post_form(
            &format!("/projects/{id}/default-cad"),
            Some(&admin),
            ORIGIN,
            &[("program", "solidworks"), ("version", "2025")],
        )
        .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    let html = app.get_page(&page, Some(&admin)).await.text();
    assert!(html.contains("<strong>SolidWorks 2025</strong>"), "{html}");
    assert!(html.contains("<option value=\"solidworks\" selected>"));

    // a member who does not manage the group sees it, but gets no form
    let html = app.get_page(&page, Some(&max)).await.text();
    assert!(html.contains("<strong>SolidWorks 2025</strong>"));
    assert!(!html.contains(&format!("action=\"/projects/{id}/default-cad\"")));
    let r = app
        .post_form(
            &format!("/projects/{id}/default-cad"),
            Some(&max),
            ORIGIN,
            &[("program", "inventor"), ("version", "")],
        )
        .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "back with an error");
    let row: (String, String) =
        sqlx::query_as("SELECT default_cad, default_cad_version FROM projects WHERE id = $1")
            .bind(id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(row, ("solidworks".into(), "2025".into()));
    app.teardown().await;
}

#[tokio::test]
async fn the_formats_list_names_the_family_of_each_format() {
    let app = app_or_skip!();
    let (_, token) = app.user("max", Role::Viewer).await;
    let formats = app
        .api(Method::GET, "/api/v1/formats", &token, None)
        .await
        .json();
    let all: Vec<&serde_json::Value> = formats["systems"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["formats"].as_array().unwrap())
        .collect();
    let family = |id: &str| {
        all.iter()
            .find(|f| f["id"] == id)
            .map(|f| f["family"].clone())
            .unwrap()
    };
    assert_eq!(family("inventor-part"), "inventor");
    assert_eq!(family("solidworks-assembly"), "solidworks");
    assert!(family("step").is_null(), "neutral formats have none");
    app.teardown().await;
}
