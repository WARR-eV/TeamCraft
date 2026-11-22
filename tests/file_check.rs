//! Duplicate checks before a file becomes a new item (issue #359): the
//! read-only `POST /projects/{code}/file-check` (same place, same name in
//! another folder, same content – current files of the group only, names
//! compared composed and whatever their case, what outsiders and bad requests
//! get) and the opt-in guard of uploads (`refuse_clash`, `refuse_same_name`):
//! refused atomically with 409 and nothing stored, new versions never refused,
//! the item's folder counted, chunked sessions guarded through `finish` and
//! through the `uploads` field of a multipart upload, one winner when two
//! uploads race for the same new file.

mod common;

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use teamcraft::domain::Role;

macro_rules! app_or_skip {
    () => {
        match common::setup().await {
            Some(app) => app,
            None => return,
        }
    };
}

const V1: &[u8] = b"\x00bracket, first version";
const V2: &[u8] = b"\x00another bracket";
const V3: &[u8] = b"\x00a third content";

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Creates a part in `project` and uploads `files` into `folder` (no guard).
async fn item(
    app: &common::TestApp,
    token: &str,
    project: &str,
    name: &str,
    folder: &str,
    files: &[(&str, &[u8])],
) -> String {
    let number = app
        .create_item(
            token,
            json!({ "item_type": "part", "name": name, "project": project }),
        )
        .await;
    if !files.is_empty() {
        let r = upload(app, token, &number, files, &[("folder", folder)]).await;
        assert!(r.status.is_success(), "upload to {number}: {}", r.text());
    }
    number
}

/// Multipart upload into revision A of `number`.
async fn upload(
    app: &common::TestApp,
    token: &str,
    number: &str,
    files: &[(&str, &[u8])],
    fields: &[(&str, &str)],
) -> common::Response {
    app.api_multipart(
        &format!("/api/v1/items/{number}/revisions/A/files"),
        token,
        files,
        fields,
    )
    .await
}

/// The file names of revision A of `number`.
async fn file_names(app: &common::TestApp, token: &str, number: &str) -> Vec<String> {
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
        .map(|f| f["filename"].as_str().unwrap().to_string())
        .collect()
}

async fn check(
    app: &common::TestApp,
    token: &str,
    project: &str,
    files: Value,
) -> common::Response {
    app.api(
        Method::POST,
        &format!("/api/v1/projects/{project}/file-check"),
        token,
        Some(json!({ "files": files })),
    )
    .await
}

/// The checked files of a successful check of ROVER.
async fn check_ok(app: &common::TestApp, token: &str, files: Value) -> Vec<Value> {
    let r = check(app, token, "ROVER", files).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.json()["files"].as_array().unwrap().clone()
}

/// A list of matches as `(item_number, folder, filename)`.
fn found(list: &Value) -> Vec<(String, String, String)> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|f| {
            assert_eq!(f["code"], "A", "{f}");
            (
                f["item_number"].as_str().unwrap().to_string(),
                f["folder"].as_str().unwrap().to_string(),
                f["filename"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn hit(number: &str, folder: &str, filename: &str) -> (String, String, String) {
    (number.to_string(), folder.to_string(), filename.to_string())
}

fn message(r: &common::Response) -> String {
    r.json()["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The three kinds of match, in request order: the file at the same place
/// (folder normalised, any case), files of the same name in other folders
/// and files with the same content (the same-place file left out) – each by
/// folder; nothing of another group.
#[tokio::test]
async fn the_check_names_the_same_place_the_same_name_and_the_same_content() {
    let app = app_or_skip!();
    let (_, erika) = app
        .user_in("erika", Role::Editor, &["ROVER", "ROCKET"])
        .await;
    let bracket = item(
        &app,
        &erika,
        "ROVER",
        "Bracket",
        "Design/Struktur",
        &[("Bracket.ipt", V1)],
    )
    .await;
    let spare = item(
        &app,
        &erika,
        "ROVER",
        "Spare bracket",
        "Normteile",
        &[("bracket.IPT", V2)],
    )
    .await;
    let old = item(
        &app,
        &erika,
        "ROVER",
        "Old bracket",
        "Archiv",
        &[("Bracket_old.ipt", V1)],
    )
    .await;
    // another group: never a match
    item(
        &app,
        &erika,
        "ROCKET",
        "Rocket bracket",
        "Design/Struktur",
        &[("Bracket.ipt", V1)],
    )
    .await;

    let files = check_ok(
        &app,
        &erika,
        json!([
            { "folder": "\\design\\struktur\\", "filename": "BRACKET.ipt", "sha256": sha(V1) },
            { "folder": "Neu", "filename": "Gusset.ipt", "sha256": null },
            { "folder": "Neu", "filename": "Bracket.ipt" },
            { "filename": "copy.ipt", "sha256": sha(V1).to_uppercase() },
        ]),
    )
    .await;
    assert_eq!(files.len(), 4, "{files:?}");

    let place = &files[0];
    assert_eq!(place["folder"], "design/struktur", "{place}");
    assert_eq!(place["filename"], "BRACKET.ipt");
    assert_eq!(
        place["same_place"],
        json!({ "item_number": bracket.as_str(), "code": "A", "name": "Bracket" })
    );
    assert_eq!(
        found(&place["same_name"]),
        [hit(&spare, "Normteile", "bracket.IPT")]
    );
    assert_eq!(
        found(&place["same_content"]),
        [hit(&old, "Archiv", "Bracket_old.ipt")]
    );

    let nothing = &files[1];
    assert_eq!(nothing["folder"], "Neu");
    assert!(nothing["same_place"].is_null(), "{nothing}");
    assert_eq!(nothing["same_name"], json!([]));
    assert_eq!(nothing["same_content"], json!([]));

    let name = &files[2];
    assert!(name["same_place"].is_null(), "{name}");
    assert_eq!(
        found(&name["same_name"]),
        [
            hit(&bracket, "Design/Struktur", "Bracket.ipt"),
            hit(&spare, "Normteile", "bracket.IPT")
        ]
    );
    // no sha256: no content check
    assert_eq!(name["same_content"], json!([]));

    let content = &files[3];
    assert_eq!(content["folder"], "");
    assert!(content["same_place"].is_null(), "{content}");
    assert_eq!(content["same_name"], json!([]));
    assert_eq!(
        found(&content["same_content"]),
        [
            hit(&old, "Archiv", "Bracket_old.ipt"),
            hit(&bracket, "Design/Struktur", "Bracket.ipt")
        ]
    );
}

/// Only the current files count: the latest revision's (here B, copied by
/// the revise); a file that only an older revision has is no match and no
/// clash.
#[tokio::test]
async fn only_the_latest_revision_counts() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let (_, olaf) = app.user_in("olaf", Role::Editor, &["ROVER"]).await;
    let hub = item(&app, &erika, "ROVER", "Hub", "Design", &[("Hub.ipt", V1)]).await;
    app.release(&erika, ("olaf", &olaf), &[&hub]).await;
    let r = app
        .api(
            Method::POST,
            &format!("/api/v1/items/{hub}/revisions/A/revise"),
            &erika,
            None,
        )
        .await;
    assert!(r.status.is_success(), "revise: {}", r.text());
    let wanted = json!([{ "folder": "Design", "filename": "Hub.ipt", "sha256": sha(V1) }]);
    let files = check_ok(&app, &erika, wanted.clone()).await;
    assert_eq!(files[0]["same_place"]["item_number"], hub.as_str());
    assert_eq!(files[0]["same_place"]["code"], "B", "{}", files[0]);
    assert_eq!(files[0]["same_content"], json!([]), "{}", files[0]);

    // revision B without the file: revision A's copy is no current file
    let r = app
        .api(
            Method::GET,
            &format!("/api/v1/items/{hub}/revisions/B/files"),
            &erika,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()[0]["id"].as_str().unwrap().to_string();
    let r = app
        .api(Method::DELETE, &format!("/api/v1/files/{id}"), &erika, None)
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{}", r.text());
    let files = check_ok(&app, &erika, wanted).await;
    assert!(files[0]["same_place"].is_null(), "{}", files[0]);
    assert_eq!(files[0]["same_name"], json!([]));
    assert_eq!(files[0]["same_content"], json!([]));

    // and the guard lets the file in
    let other = item(&app, &erika, "ROVER", "Hub again", "", &[]).await;
    let r = upload(
        &app,
        &erika,
        &other,
        &[("Hub.ipt", V2)],
        &[
            ("folder", "Design"),
            ("refuse_clash", "true"),
            ("refuse_same_name", "true"),
        ],
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
}

/// A decomposed (NFD) folder and name find the composed ones the server
/// stores, whatever the case of their letters – in the check and in the
/// guard.
#[tokio::test]
async fn names_are_compared_composed_and_whatever_their_case() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let railing = item(
        &app,
        &erika,
        "ROVER",
        "Railing",
        "B\u{fc}hne/Tr\u{e4}ger",
        &[("Gel\u{e4}nder.ipt", V1)],
    )
    .await;
    let files = check_ok(
        &app,
        &erika,
        json!([{ "folder": "Bu\u{308}HNE/TRa\u{308}GER", "filename": "GELa\u{308}NDER.IPT" }]),
    )
    .await;
    // answered composed
    assert_eq!(files[0]["folder"], "B\u{fc}HNE/TR\u{e4}GER");
    assert_eq!(files[0]["filename"], "GEL\u{e4}NDER.IPT");
    assert_eq!(
        files[0]["same_place"]["item_number"],
        railing.as_str(),
        "{}",
        files[0]
    );

    let other = item(&app, &erika, "ROVER", "Railing too", "", &[]).await;
    let r = upload(
        &app,
        &erika,
        &other,
        &[("gela\u{308}nder.IPT", V2)],
        &[
            ("folder", "bu\u{308}hne/tra\u{308}ger"),
            ("refuse_clash", "true"),
        ],
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert_eq!(file_names(&app, &erika, &other).await, Vec::<String>::new());
}

fn one(folder: &str, filename: &str, sha256: Value) -> Value {
    json!([{ "folder": folder, "filename": filename, "sha256": sha256 }])
}

/// Bad folders, names and hashes, no file and too many files are a 400;
/// unknown groups 404, a missing token 401.
#[tokio::test]
async fn bad_requests_are_refused() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let long = format!("{}.ipt", "a".repeat(200));
    let not_hex = "g".repeat(64);
    for (what, files) in [
        ("..", one("../outside", "a.ipt", Value::Null)),
        ("pipe", one("a|b", "a.ipt", Value::Null)),
        ("reserved", one("CON", "a.ipt", Value::Null)),
        ("empty name", one("", " ", Value::Null)),
        ("dots", one("", "..", Value::Null)),
        ("folder in the name", one("", "Design/a.ipt", Value::Null)),
        ("backslash", one("", "Design\\a.ipt", Value::Null)),
        ("long name", one("", &long, Value::Null)),
        ("short sha256", one("", "a.ipt", json!("abc"))),
        ("empty sha256", one("", "a.ipt", json!(""))),
        ("not hex", one("", "a.ipt", json!(not_hex))),
        ("no file", json!([])),
        (
            "too many",
            Value::Array(vec![json!({ "filename": "a.ipt" }); 201]),
        ),
    ] {
        let r = check(&app, &erika, "ROVER", files).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{what}: {}", r.text());
    }
    // the most files one request may check
    let r = check(
        &app,
        &erika,
        "ROVER",
        Value::Array(vec![json!({ "filename": "a.ipt" }); 200]),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["files"].as_array().unwrap().len(), 200);
    // no list at all
    let r = app
        .api(
            Method::POST,
            "/api/v1/projects/ROVER/file-check",
            &erika,
            Some(json!({})),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    let r = check(&app, &erika, "NOPE", one("", "a.ipt", Value::Null)).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    let r = check(&app, "bad-token", "ROVER", one("", "a.ipt", Value::Null)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

/// Viewers may check (it only reads); a restricted group answers an outsider
/// like an unknown code – also for a bad request – and its members read it.
#[tokio::test]
async fn a_restricted_group_is_not_found_and_viewers_may_check() {
    let app = app_or_skip!();
    let (_, erika) = app
        .user_in("erika", Role::Editor, &["ROVER", "ROCKET"])
        .await;
    let (_, olaf) = app.user_in("olaf", Role::Editor, &["ROVER"]).await;
    let (_, vera) = app.user("vera", Role::Viewer).await;
    let nozzle = item(
        &app,
        &erika,
        "ROCKET",
        "Nozzle",
        "engine",
        &[("Nozzle.ipt", V1)],
    )
    .await;
    let wanted = json!([{ "folder": "engine", "filename": "Nozzle.ipt", "sha256": sha(V1) }]);
    let r = check(&app, &vera, "ROCKET", wanted.clone()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["files"][0]["same_place"]["item_number"],
        nozzle.as_str()
    );

    let (_, admin) = app.user("root", Role::Admin).await;
    let r = app
        .api(
            Method::PUT,
            "/api/v1/projects/ROCKET/visibility",
            &admin,
            Some(json!({ "visibility": "restricted" })),
        )
        .await;
    assert!(r.status.is_success(), "{}", r.text());
    for who in [&olaf, &vera] {
        let hidden = check(&app, who, "ROCKET", wanted.clone()).await;
        assert_eq!(hidden.status, StatusCode::NOT_FOUND, "{}", hidden.text());
        assert_eq!(
            hidden.text().replace("ROCKET", "NOPE"),
            check(&app, who, "NOPE", wanted.clone()).await.text()
        );
        let bad = check(&app, who, "ROCKET", one("..", "x.ipt", Value::Null)).await;
        assert_eq!(bad.status, StatusCode::NOT_FOUND, "{}", bad.text());
    }
    let member = check(&app, &erika, "ROCKET", wanted).await;
    assert_eq!(member.status, StatusCode::OK, "{}", member.text());
    assert_eq!(
        member.json()["files"][0]["same_place"]["item_number"],
        nozzle.as_str()
    );
}

/// `refuse_clash`: a new file another item already has at that folder and
/// name (any case; the folder the file really gets, also the item's own
/// folder) is refused with 409 and nothing of the request is stored. The
/// same name elsewhere passes, uploads without the flag are stored as
/// before, and new versions of a file of the revision are never refused.
#[tokio::test]
async fn refuse_clash_refuses_a_file_another_item_has_at_that_place() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let bracket = item(
        &app,
        &erika,
        "ROVER",
        "Bracket",
        "Design/Struktur",
        &[("Bracket.ipt", V1)],
    )
    .await;
    let copy = item(&app, &erika, "ROVER", "Bracket copy", "", &[]).await;
    let guarded = [("folder", "design/struktur"), ("refuse_clash", "true")];

    let r = upload(&app, &erika, &copy, &[("BRACKET.IPT", V2)], &guarded).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    let m = message(&r);
    assert!(
        m.contains(&format!("{bracket}/A"))
            && m.contains("Design/Struktur")
            && m.contains("open that item"),
        "{m}"
    );
    // several files: none of them is stored
    let r = upload(
        &app,
        &erika,
        &copy,
        &[("Notes.txt", V3), ("Bracket.ipt", V2)],
        &guarded,
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert_eq!(file_names(&app, &erika, &copy).await, Vec::<String>::new());
    let stored: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM files f JOIN revisions r ON r.id = f.revision_id
         JOIN items i ON i.id = r.item_id WHERE i.number = $1",
    )
    .bind(&copy)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(stored, 0);

    // without a folder the file goes into the item's folder (#354): refused there
    let r = app
        .api(
            Method::POST,
            "/api/v1/items",
            &erika,
            Some(
                json!({ "item_type": "part", "name": "Bracket in its folder",
                         "project": "ROVER", "folder": "Design/Struktur" }),
            ),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let placed = r.json()["item_number"].as_str().unwrap().to_string();
    let r = upload(
        &app,
        &erika,
        &placed,
        &[("Bracket.ipt", V2)],
        &[("refuse_clash", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());

    // the same name in another folder passes without refuse_same_name
    let r = upload(
        &app,
        &erika,
        &copy,
        &[("Bracket.ipt", V2)],
        &[("folder", "Normteile"), ("refuse_clash", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());

    // without the flag the duplicate is stored, as before
    let twin = item(&app, &erika, "ROVER", "Bracket twin", "", &[]).await;
    let r = upload(
        &app,
        &erika,
        &twin,
        &[("Bracket.ipt", V2)],
        &[("folder", "Design/Struktur")],
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    // a new version of a file of the revision is never refused
    let r = upload(
        &app,
        &erika,
        &twin,
        &[("Bracket.ipt", V3)],
        &[
            ("folder", "Design/Struktur"),
            ("refuse_clash", "true"),
            ("refuse_same_name", "true"),
        ],
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    assert_eq!(r.json()[0]["version"], 2, "{}", r.text());
    let r = upload(
        &app,
        &erika,
        &bracket,
        &[("Bracket.ipt", V3)],
        &[("refuse_clash", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    assert_eq!(r.json()[0]["version"], 2, "{}", r.text());
}

/// `refuse_same_name` also refuses the name in another folder of the group
/// (the message names the item and the folder) and includes `refuse_clash`.
#[tokio::test]
async fn refuse_same_name_refuses_the_name_in_another_folder() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let screw = item(
        &app,
        &erika,
        "ROVER",
        "Screw",
        "Normteile",
        &[("Schraube.ipt", V1)],
    )
    .await;
    let part = item(&app, &erika, "ROVER", "Screw too", "", &[]).await;

    let r = upload(
        &app,
        &erika,
        &part,
        &[("SCHRAUBE.ipt", V2)],
        &[("folder", "Design"), ("refuse_same_name", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    let m = message(&r);
    assert!(
        m.contains("in Normteile") && m.contains(&format!("{screw}/A")),
        "{m}"
    );
    assert_eq!(file_names(&app, &erika, &part).await, Vec::<String>::new());

    let r = upload(
        &app,
        &erika,
        &part,
        &[("Schraube.ipt", V2)],
        &[("folder", "Normteile"), ("refuse_same_name", "on")],
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert!(message(&r).contains("open that item"), "{}", r.text());

    // refuse_clash alone lets the name in another folder pass
    let r = upload(
        &app,
        &erika,
        &part,
        &[("SCHRAUBE.ipt", V2)],
        &[("folder", "Design"), ("refuse_clash", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    assert_eq!(file_names(&app, &erika, &part).await, ["SCHRAUBE.ipt"]);
}

/// Items without a project group have no guard.
#[tokio::test]
async fn items_without_a_group_have_no_guard() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let first = item(&app, &erika, "ROVER", "Loose", "", &[]).await;
    let second = item(&app, &erika, "ROVER", "Loose too", "", &[]).await;
    for number in [&first, &second] {
        sqlx::query("UPDATE items SET project_id = NULL WHERE number = $1")
            .bind(number)
            .execute(&app.pool)
            .await
            .unwrap();
    }
    let fields = [("folder", "Design"), ("refuse_same_name", "true")];
    for number in [&first, &second] {
        let r = upload(&app, &erika, number, &[("Loose.ipt", V1)], &fields).await;
        assert_eq!(r.status, StatusCode::CREATED, "{number}: {}", r.text());
    }
}

/// A chunked session for revision A of `item` with the content `V2`.
fn session(item: &str, filename: &str, refuse_clash: bool) -> Value {
    json!({ "item": item, "revision": "A", "filename": filename, "size": V2.len(),
            "sha256": sha(V2), "folder": "Design", "refuse_clash": refuse_clash })
}

/// Sends the whole content of a session in one chunk.
async fn send_all(app: &common::TestApp, token: &str, id: &str, content: &[u8]) {
    let request = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/v1/uploads/{id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/offset+octet-stream")
        .header("Upload-Offset", "0")
        .body(Body::from(content.to_vec()))
        .unwrap();
    let r = app.send(request).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
}

/// Chunked uploads: `refuse_clash` of `POST /uploads` is kept with the
/// session, refuses a duplicate when the session starts and again when it is
/// stored (`finish`, or the `uploads` field of a multipart upload – where the
/// request's own `refuse_clash` covers the sessions too and nothing of the
/// request is stored); without a flag the sessions are stored as before.
#[tokio::test]
async fn chunked_uploads_honour_the_guard() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let hub = item(&app, &erika, "ROVER", "Hub", "", &[]).await;
    let copy = item(&app, &erika, "ROVER", "Hub copy", "", &[]).await;

    // started while nothing clashes; the session keeps the flag
    let r = app
        .api(
            Method::POST,
            "/api/v1/uploads",
            &erika,
            Some(session(&copy, "Hub.op2", true)),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    assert_eq!(r.json()["refuse_clash"], true, "{}", r.text());
    let guarded = r.json()["id"].as_str().unwrap().to_string();
    send_all(&app, &erika, &guarded, V2).await;
    let r = app
        .api(
            Method::POST,
            "/api/v1/uploads",
            &erika,
            Some(session(&copy, "Rim.op2", false)),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    assert_eq!(r.json()["refuse_clash"], false, "{}", r.text());
    let plain = r.json()["id"].as_str().unwrap().to_string();
    send_all(&app, &erika, &plain, V2).await;

    // meanwhile the other item gets both files in that folder
    let r = upload(
        &app,
        &erika,
        &hub,
        &[("Hub.op2", V1), ("Rim.op2", V1)],
        &[("folder", "Design")],
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());

    // a new guarded session is refused at once, before its bytes are sent
    let r = app
        .api(
            Method::POST,
            "/api/v1/uploads",
            &erika,
            Some(session(&copy, "hub.OP2", true)),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());

    // finish stores nothing; the session stays and can be finished later
    let r = app
        .api(
            Method::POST,
            &format!("/api/v1/uploads/{guarded}/finish"),
            &erika,
            Some(json!({})),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert!(message(&r).contains(&format!("{hub}/A")), "{}", r.text());
    assert_eq!(file_names(&app, &erika, &copy).await, Vec::<String>::new());
    let r = app
        .api(
            Method::GET,
            &format!("/api/v1/uploads/{guarded}"),
            &erika,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    // the desktop way: the sessions join a multipart upload in the `uploads`
    // field; the request's refuse_clash covers a session without the flag,
    // and the small file of the request is not stored either
    let plain_ids = json!([plain]).to_string();
    let r = upload(
        &app,
        &erika,
        &copy,
        &[("Notes.txt", V3)],
        &[("uploads", plain_ids.as_str()), ("refuse_clash", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert!(message(&r).contains("Rim.op2"), "{}", r.text());
    assert_eq!(file_names(&app, &erika, &copy).await, Vec::<String>::new());
    // a session's own flag counts in a multipart request too
    let guarded_ids = json!([guarded]).to_string();
    let r = upload(
        &app,
        &erika,
        &copy,
        &[("Notes.txt", V3)],
        &[("uploads", guarded_ids.as_str())],
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert_eq!(file_names(&app, &erika, &copy).await, Vec::<String>::new());

    // without any flag the session is stored as before
    let r = upload(
        &app,
        &erika,
        &copy,
        &[("Notes.txt", V3)],
        &[("uploads", plain_ids.as_str())],
    )
    .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let mut names = file_names(&app, &erika, &copy).await;
    names.sort();
    assert_eq!(names, ["Notes.txt", "Rim.op2"]);
}

/// Two guarded uploads of the same new file into two items at the same time:
/// the advisory lock of the group lets exactly one of them in.
#[tokio::test]
async fn two_uploads_of_the_same_new_file_at_once_give_one_winner() {
    let app = app_or_skip!();
    let (_, erika) = app.user_in("erika", Role::Editor, &["ROVER"]).await;
    let (_, olaf) = app.user_in("olaf", Role::Editor, &["ROVER"]).await;
    let first = item(&app, &erika, "ROVER", "Wheel", "", &[]).await;
    let second = item(&app, &olaf, "ROVER", "Wheel too", "", &[]).await;
    let fields = [("folder", "Design/Rad"), ("refuse_clash", "true")];
    let (a, b) = tokio::join!(
        upload(&app, &erika, &first, &[("Wheel.ipt", V1)], &fields),
        upload(&app, &olaf, &second, &[("Wheel.ipt", V2)], &fields),
    );
    let mut statuses = [a.status.as_u16(), b.status.as_u16()];
    statuses.sort_unstable();
    assert_eq!(statuses, [201, 409], "{} / {}", a.text(), b.text());
    let stored =
        file_names(&app, &erika, &first).await.len() + file_names(&app, &olaf, &second).await.len();
    assert_eq!(stored, 1);
}
