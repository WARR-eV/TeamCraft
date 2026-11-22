//! Checking out several revisions at once (`POST /api/v1/checkouts`, issue
//! #415): a result per revision, refusals do not stop the others, hidden
//! revisions answer like missing ones, overlapping batches give every
//! revision one winner, and only changed revisions are audited.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
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

/// Creates a part in `project` and checks it in again, so nobody holds it.
async fn free_part(app: &common::TestApp, token: &str, name: &str, project: &str) -> String {
    let number = app
        .create_item(
            token,
            json!({ "item_type": "part", "name": name, "project": project }),
        )
        .await;
    let r = app
        .api(
            Method::POST,
            &format!("/api/v1/items/{number}/revisions/A/checkin"),
            token,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    number
}

async fn revision_id(app: &common::TestApp, number: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT r.id FROM revisions r JOIN items i ON i.id = r.item_id
          WHERE i.number = $1 AND r.code = 'A'",
    )
    .bind(number)
    .fetch_one(&app.pool)
    .await
    .expect("revision exists")
}

async fn holder(app: &common::TestApp, number: &str) -> Option<String> {
    sqlx::query_scalar(
        "SELECT u.username FROM revisions r JOIN items i ON i.id = r.item_id
           LEFT JOIN users u ON u.id = r.checked_out_by
          WHERE i.number = $1 AND r.code = 'A'",
    )
    .bind(number)
    .fetch_one(&app.pool)
    .await
    .expect("revision exists")
}

async fn checkout_audits(app: &common::TestApp, number: &str) -> usize {
    let id = revision_id(app, number).await;
    app.audit_actions(id)
        .await
        .iter()
        .filter(|a| *a == "revision.checkout")
        .count()
}

async fn batch(app: &common::TestApp, token: &str, revisions: Value) -> common::Response {
    app.api(
        Method::POST,
        "/api/v1/checkouts",
        token,
        Some(json!({ "revisions": revisions })),
    )
    .await
}

fn by_item<'a>(body: &'a Value, item: &str) -> &'a Value {
    body["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["item"] == item)
        .unwrap_or_else(|| panic!("no outcome for {item}: {body}"))
}

fn refs<S: AsRef<str>>(numbers: &[S]) -> Value {
    Value::Array(
        numbers
            .iter()
            .map(|n| json!({ "item": n.as_ref(), "code": "A" }))
            .collect(),
    )
}

#[tokio::test]
async fn checks_out_the_free_parts_and_reports_the_rest() {
    let app = app_or_skip!();
    let (_, max) = app.user_in("max", Role::Editor, &["ROVER"]).await;
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let (_, eva) = app.user_in("eva", Role::Editor, &["ROVER"]).await;

    let free = free_part(&app, &max, "Wheel hub", "ROVER").await;
    let mine = free_part(&app, &max, "Axle", "ROVER").await;
    let held = free_part(&app, &max, "Bracket", "ROVER").await;
    let released = free_part(&app, &max, "Cover", "ROVER").await;
    let in_review = free_part(&app, &max, "Spacer", "ROVER").await;
    app.release(&max, ("eva", &eva), &[released.as_str()]).await;
    app.submit_change(&max, "Review", &[in_review.as_str()], &["eva"])
        .await;
    for (number, token) in [(&mine, &max), (&held, &erika)] {
        let r = app
            .api(
                Method::POST,
                &format!("/api/v1/items/{number}/revisions/A/checkout"),
                token,
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    }
    let audits_mine = checkout_audits(&app, &mine).await;
    let audits_free = checkout_audits(&app, &free).await;

    let mut list = refs(&[&free, &mine, &held, &released, &in_review]);
    // a duplicate (once by code, once by id) is answered once
    let free_id = revision_id(&app, &free).await;
    list.as_array_mut().unwrap().push(json!({ "id": free_id }));
    let r = batch(&app, &max, list).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    assert_eq!(body["revisions"].as_array().unwrap().len(), 5, "{body}");
    assert_eq!(body["checked_out"], 2, "{body}");
    assert_eq!(body["refused"], 3, "{body}");

    let o = by_item(&body, &free);
    assert_eq!(o["ok"], true);
    assert_eq!(o["already"], false);
    assert_eq!(o["revision_id"], json!(free_id));
    assert_eq!(o["checked_out_by_name"], "max (test)");
    let o = by_item(&body, &mine);
    assert_eq!(o["ok"], true);
    assert_eq!(o["already"], true, "held by the actor before: {o}");
    let o = by_item(&body, &held);
    assert_eq!(o["ok"], false);
    assert_eq!(o["reason"], "locked");
    assert_eq!(o["checked_out_by_name"], "erika (test)");
    let o = by_item(&body, &released);
    assert_eq!(o["reason"], "released");
    assert!(
        o["message"].as_str().unwrap().contains("revise it first"),
        "{o}"
    );
    assert!(o["checked_out_by_name"].is_null());
    let o = by_item(&body, &in_review);
    assert_eq!(o["reason"], "in_review");
    let summary = body["summary"].as_str().unwrap();
    assert!(
        summary.starts_with("Checked out 2 revisions; 3 refused"),
        "{summary}"
    );

    // the database follows the result
    assert_eq!(holder(&app, &free).await.as_deref(), Some("max"));
    assert_eq!(holder(&app, &mine).await.as_deref(), Some("max"));
    assert_eq!(holder(&app, &held).await.as_deref(), Some("erika"));
    assert_eq!(holder(&app, &released).await, None);
    assert_eq!(holder(&app, &in_review).await, None);

    // audit: one record for the revision that changed, none for the one max already held
    assert_eq!(checkout_audits(&app, &free).await, audits_free + 1);
    assert_eq!(checkout_audits(&app, &mine).await, audits_mine);

    // running the same batch again changes nothing and audits nothing
    let r = batch(&app, &max, refs(&[&free, &mine])).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(by_item(&r.json(), &free)["already"], true);
    assert_eq!(checkout_audits(&app, &free).await, audits_free + 1);
}

#[tokio::test]
async fn permissions_viewers_and_other_groups() {
    let app = app_or_skip!();
    let (_, max) = app.user_in("max", Role::Editor, &["ROVER"]).await;
    let (_, vic) = app.user_in("vic", Role::Viewer, &["ROVER"]).await;
    let (_, olga) = app.user_in("olga", Role::Editor, &["ARM"]).await;
    let rover = free_part(&app, &max, "Wheel hub", "ROVER").await;
    let arm = free_part(&app, &olga, "Gripper", "ARM").await;

    // a viewer may not check out anything: the whole request is refused
    let r = batch(&app, &vic, refs(&[&rover])).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());
    assert_eq!(holder(&app, &rover).await, None);

    // an editor of another group: forbidden for that part, the own part goes through
    let r = batch(&app, &olga, refs(&[&rover, &arm])).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    assert_eq!(by_item(&body, &rover)["reason"], "forbidden");
    assert!(by_item(&body, &rover)["message"]
        .as_str()
        .unwrap()
        .contains("ROVER"));
    assert_eq!(by_item(&body, &arm)["ok"], true);
    assert_eq!(holder(&app, &rover).await, None);
    assert_eq!(holder(&app, &arm).await.as_deref(), Some("olga"));
}

#[tokio::test]
async fn bad_requests() {
    let app = app_or_skip!();
    let (_, max) = app.user_in("max", Role::Editor, &["ROVER"]).await;
    let r = batch(&app, &max, json!([{ "item": "PRT-000001" }])).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let too_many: Vec<Value> = (0..501).map(|_| json!({ "id": Uuid::new_v4() })).collect();
    let r = batch(&app, &max, Value::Array(too_many)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let r = batch(&app, &max, json!([])).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["summary"], "Nothing to check out.");
}

#[tokio::test]
async fn hidden_revisions_answer_like_missing_ones() {
    let app = app_or_skip!();
    let (_, max) = app.user_in("max", Role::Editor, &["ROVER"]).await;
    let (_, erika) = app.user("erika", Role::Editor).await;
    let r = app
        .api(
            Method::POST,
            "/api/v1/projects",
            &erika,
            Some(json!({ "code": "NTK", "name": "Need to know", "visibility": "restricted" })),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let secret = free_part(&app, &erika, "Secret part", "NTK").await;
    let secret_id = revision_id(&app, &secret).await;
    let missing_id = Uuid::new_v4();

    let r = batch(
        &app,
        &max,
        json!([
            { "item": secret, "code": "A" },
            { "item": "PRT-999999", "code": "A" },
            { "id": secret_id },
            { "id": missing_id },
        ]),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    let list = body["revisions"].as_array().unwrap();
    assert_eq!(list.len(), 4, "{body}");
    for o in list {
        assert_eq!(o["ok"], false, "{o}");
        assert_eq!(o["reason"], "not_found", "{o}");
        assert_eq!(o["message"], "Revision not found.", "{o}");
        assert!(o["name"].is_null(), "no name of a hidden revision: {o}");
        assert!(o["checked_out_by_name"].is_null(), "{o}");
    }
    // by code: only what was asked comes back, never the hidden revision's id
    assert_eq!(list[0]["item"], json!(secret));
    assert!(list[0]["revision_id"].is_null(), "{}", list[0]);
    assert!(list[1]["revision_id"].is_null(), "{}", list[1]);
    // by id: the id as asked, nothing more – the same shape as a missing one
    assert_eq!(list[2]["revision_id"], json!(secret_id));
    assert_eq!(list[2]["item"], "");
    assert_eq!(list[3]["revision_id"], json!(missing_id));
    assert_eq!(list[3]["item"], "");
    assert_eq!(holder(&app, &secret).await, None);
}

#[tokio::test]
async fn overlapping_batches_give_every_revision_one_winner() {
    let app = app_or_skip!();
    let (_, max) = app.user_in("max", Role::Editor, &["ROVER"]).await;
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let mut parts = Vec::new();
    for name in ["A", "B", "C", "D", "E", "F"] {
        parts.push(free_part(&app, &max, &format!("Part {name}"), "ROVER").await);
    }
    let p: Vec<&str> = parts.iter().map(String::as_str).collect();
    let mut before = std::collections::HashMap::new();
    for n in &p {
        before.insert(n.to_string(), checkout_audits(&app, n).await);
    }
    // max: A B C D, erika: D C B E F – in a different order on purpose
    let max_set = refs(&[p[0], p[1], p[2], p[3]]);
    let erika_set = refs(&[p[3], p[2], p[1], p[4], p[5]]);
    let (rm, re) = tokio::join!(batch(&app, &max, max_set), batch(&app, &erika, erika_set));
    assert_eq!(rm.status, StatusCode::OK, "{}", rm.text());
    assert_eq!(re.status, StatusCode::OK, "{}", re.text());
    let (bm, be) = (rm.json(), re.json());

    // the parts only one of them asked for go to that one
    assert_eq!(by_item(&bm, p[0])["ok"], true);
    assert_eq!(by_item(&be, p[4])["ok"], true);
    assert_eq!(by_item(&be, p[5])["ok"], true);
    // the overlap: exactly one winner per part, the other side sees it locked
    for n in [p[1], p[2], p[3]] {
        let (om, oe) = (by_item(&bm, n), by_item(&be, n));
        let max_won = om["ok"] == true;
        assert_ne!(max_won, oe["ok"] == true, "one winner for {n}: {om} {oe}");
        let (loser, winner) = if max_won { (oe, "max") } else { (om, "erika") };
        assert_eq!(loser["reason"], "locked", "{loser}");
        assert_eq!(
            loser["checked_out_by_name"],
            format!("{winner} (test)"),
            "{loser}"
        );
        assert_eq!(holder(&app, n).await.as_deref(), Some(winner));
        assert_eq!(
            checkout_audits(&app, n).await,
            before[n] + 1,
            "one audit record for {n}"
        );
    }
}
