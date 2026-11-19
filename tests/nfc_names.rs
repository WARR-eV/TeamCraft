//! File and folder names in Unicode NFC. macOS hands out decomposed names
//! (NFD, `a` + U+0308 for `ä`); Windows and the CAD programs there write the
//! composed form and look for it. The server stores every name composed, so
//! an assembly finds its parts on every system and the same file from a Mac
//! and from Windows is one file with versions, not two (`util::nfc`,
//! migration 0073 for names stored before).

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

const NFD_NAME: &str = "Planetentra\u{308}ger2.ipt";
const NFC_NAME: &str = "Planetentr\u{e4}ger2.ipt";
const NFD_FOLDER: &str = "Design/Radkonstruktion/Za\u{308}hne";
const NFC_FOLDER: &str = "Design/Radkonstruktion/Z\u{e4}hne";

/// The `folders` field of an upload: `{"<name>": "<folder>"}`.
fn folders(name: &str, folder: &str) -> String {
    let mut map = serde_json::Map::new();
    map.insert(name.to_string(), Value::String(folder.to_string()));
    Value::Object(map).to_string()
}

async fn files_of(app: &common::TestApp, token: &str, number: &str) -> Vec<Value> {
    let r = app
        .api(
            Method::GET,
            &format!("/api/v1/items/{number}/revisions/A"),
            token,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.json()["files"].as_array().unwrap().clone()
}

async fn check_in(
    app: &common::TestApp,
    token: &str,
    number: &str,
    name: &str,
    content: &[u8],
    folders: &str,
) {
    let r = app
        .api_multipart(
            &format!("/api/v1/items/{number}/revisions/A/checkin"),
            token,
            &[(name, content)],
            &[("comment", "nfc"), ("folders", folders)],
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
}

async fn check_out(app: &common::TestApp, token: &str, number: &str) {
    let r = app
        .api(
            Method::POST,
            &format!("/api/v1/items/{number}/revisions/A/checkout"),
            token,
            None,
        )
        .await;
    assert!(r.status.is_success(), "{}", r.text());
}

#[tokio::test]
async fn decomposed_names_are_stored_composed_and_stay_one_file() {
    let app = app_or_skip!();
    let (_, token) = app.user_in("mac", Role::Editor, &["ROCKET"]).await;
    let number = app
        .create_item(
            &token,
            json!({ "item_type": "part", "name": "Planetenträger", "project": "ROCKET" }),
        )
        .await;

    // from a Mac: name and folder decomposed
    let in_folder = folders(NFD_NAME, NFD_FOLDER);
    check_in(
        &app,
        &token,
        &number,
        NFD_NAME,
        b"v1 from the mac",
        &in_folder,
    )
    .await;
    let files = files_of(&app, &token, &number).await;
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["filename"], NFC_NAME);
    assert_eq!(files[0]["folder"], NFC_FOLDER);
    assert_eq!(files[0]["version"], 1);

    // from Windows: the composed name is the same file, a new version
    check_out(&app, &token, &number).await;
    let in_folder = folders(NFC_NAME, NFC_FOLDER);
    check_in(
        &app,
        &token,
        &number,
        NFC_NAME,
        b"v2 from windows",
        &in_folder,
    )
    .await;
    // and from the Mac again
    check_out(&app, &token, &number).await;
    let in_folder = folders(NFD_NAME, NFD_FOLDER);
    check_in(
        &app,
        &token,
        &number,
        NFD_NAME,
        b"v3 from the mac",
        &in_folder,
    )
    .await;
    let files = files_of(&app, &token, &number).await;
    assert_eq!(files.len(), 1, "one file, not one per form: {files:?}");
    assert_eq!(files[0]["filename"], NFC_NAME);
    assert_eq!(files[0]["folder"], NFC_FOLDER);
    assert_eq!(files[0]["version"], 3);
    app.teardown().await;
}

#[tokio::test]
async fn migration_0073_composes_names_stored_before() {
    let app = app_or_skip!();
    let (_, token) = app.user_in("mac", Role::Editor, &["ROCKET"]).await;
    let number = app
        .create_item(
            &token,
            json!({ "item_type": "assembly", "name": "Rad Mitte", "project": "ROCKET" }),
        )
        .await;
    let in_folder = folders("Rad Mitte.iam", "Design");
    check_in(
        &app,
        &token,
        &number,
        "Rad Mitte.iam",
        b"assembly",
        &in_folder,
    )
    .await;
    let file_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT f.id FROM files f JOIN revisions r ON r.id = f.revision_id
         JOIN items i ON i.id = r.item_id WHERE i.number = $1",
    )
    .bind(&number)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    let version_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM file_versions WHERE file_id = $1")
            .bind(file_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();

    // what a server before the fix stored for a folder imported from a Mac
    sqlx::query("UPDATE files SET filename = $2, folder = $3 WHERE id = $1")
        .bind(file_id)
        .bind("Rad Mitte Gera\u{308}t.iam")
        .bind(NFD_FOLDER)
        .execute(&app.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE revisions SET name = $2 WHERE id = (SELECT revision_id FROM files WHERE id = $1)",
    )
    .bind(file_id)
    .bind("Rad Mitte Gera\u{308}t")
    .execute(&app.pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM file_references WHERE file_version_id = $1")
        .bind(version_id)
        .execute(&app.pool)
        .await
        .unwrap();
    for name in [
        NFD_NAME,
        NFC_NAME,
        "Motor_shaft_hu\u{308}lse.ipt",
        "bolt.ipt",
    ] {
        sqlx::query("INSERT INTO file_references (file_version_id, name) VALUES ($1, $2)")
            .bind(version_id)
            .bind(name)
            .execute(&app.pool)
            .await
            .unwrap();
    }

    let migration = include_str!("../migrations/0073_nfc_file_names.sql");
    for _ in 0..2 {
        // applied twice: the second run changes nothing
        sqlx::raw_sql(migration).execute(&app.pool).await.unwrap();
        let (filename, folder): (String, String) =
            sqlx::query_as("SELECT filename, folder FROM files WHERE id = $1")
                .bind(file_id)
                .fetch_one(&app.pool)
                .await
                .unwrap();
        assert_eq!(filename, "Rad Mitte Ger\u{e4}t.iam");
        assert_eq!(folder, NFC_FOLDER);
        let item_name: String = sqlx::query_scalar(
            "SELECT r.name FROM revisions r JOIN files f ON f.revision_id = r.id WHERE f.id = $1",
        )
        .bind(file_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
        assert_eq!(item_name, "Rad Mitte Ger\u{e4}t", "the item name too");
        let mut refs: Vec<String> =
            sqlx::query_scalar("SELECT name FROM file_references WHERE file_version_id = $1")
                .bind(version_id)
                .fetch_all(&app.pool)
                .await
                .unwrap();
        refs.sort();
        assert_eq!(
            refs,
            vec![
                "Motor_shaft_h\u{fc}lse.ipt".to_string(),
                NFC_NAME.to_string(),
                "bolt.ipt".to_string()
            ]
        );
    }
    app.teardown().await;
}
