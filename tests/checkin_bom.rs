//! *Update the BOM from the CAD file* at check-in (#407): the field
//! `bom_from_references` of `POST …/checkin` (and of `check_in` on
//! `POST /uploads/{id}/finish`) applies the plan of `…/bom/from-references`
//! to the version being checked in, in the check-in's transaction – new
//! children added, CAD-counted quantities corrected, an *assumed 1* never
//! applied over a line, unreferenced lines kept and reported, missing files
//! reported, circular children refused. Without the field, on a park and on
//! a refused upload the BOM stays as it was.

mod common;

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
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

/// A STEP assembly `name` that places each external file `count` times
/// (the counts are stored with its references, `cadrefs::instance_counts`).
fn step_assembly(name: &str, children: &[(&str, u32)]) -> Vec<u8> {
    assert!(children.len() <= 8, "product ids stay below the usage ids");
    let product = |n: usize| {
        format!(
            "#{n}1=PRODUCT('P{n}','P{n}','',(#9));\n\
             #{n}2=PRODUCT_DEFINITION_FORMATION('','',#{n}1);\n\
             #{n}3=PRODUCT_DEFINITION('design','',#{n}2,#8);\n"
        )
    };
    // 1 = the assembly, 2.. = the external files
    let mut data = (1..=children.len() + 1).map(product).collect::<String>();
    let mut usage = 100;
    for (i, (file, count)) in children.iter().enumerate() {
        let k = i + 2;
        let doc = 900 + 10 * i;
        data += &format!(
            "#{doc}=DOCUMENT_FILE('{file}','',$,#1,'','');\n\
             #{}=APPLIED_DOCUMENT_REFERENCE(#{doc},'',(#{k}3));\n",
            doc + 1
        );
        for _ in 0..*count {
            data +=
                &format!("#{usage}=NEXT_ASSEMBLY_USAGE_OCCURRENCE('{usage}','','',#13,#{k}3,$);\n");
            usage += 1;
        }
    }
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
         FILE_NAME('{name}','2026-11-11T10:00:00',(''),(''),'','','');\n\
         FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n{data}ENDSEC;\nEND-ISO-10303-21;\n"
    )
    .into_bytes()
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// A SOLIDWORKS assembly file (old format: file names as UTF-16 strings, no
/// instance counts – every quantity is an *assumed 1*).
fn sldasm(names: &[&str]) -> Vec<u8> {
    let mut out = b"\x00\x01".to_vec();
    for name in names {
        out.extend(utf16(&format!("C:\\Lib\\{name}")));
        out.extend([0, 0, 7, 0]);
    }
    out
}

async fn item(app: &common::TestApp, token: &str, kind: &str) -> String {
    app.create_item(
        token,
        json!({ "item_type": kind, "name": format!("{kind} in ROCKET"), "project": "ROCKET" }),
    )
    .await
}

/// `POST …/checkin` with files and text fields.
async fn checkin(
    app: &common::TestApp,
    token: &str,
    number: &str,
    files: &[(&str, &[u8])],
    fields: &[(&str, &str)],
) -> common::Response {
    app.api_multipart(
        &format!("/api/v1/items/{number}/revisions/A/checkin"),
        token,
        files,
        fields,
    )
    .await
}

/// The first check-in of a new item (checked out to its author), without
/// the BOM field.
async fn first(app: &common::TestApp, token: &str, number: &str, filename: &str, bytes: &[u8]) {
    let r = checkin(app, token, number, &[(filename, bytes)], &[]).await;
    assert_eq!(r.status, StatusCode::OK, "{filename}: {}", r.text());
    assert!(
        r.json()["bom"].is_null(),
        "no field, no BOM step: {}",
        r.text()
    );
}

async fn checkout(app: &common::TestApp, token: &str, number: &str) {
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

async fn add_line(app: &common::TestApp, token: &str, parent: &str, child: &str, quantity: f64) {
    let r = app
        .api(
            Method::POST,
            &format!("/api/v1/items/{parent}/revisions/A/bom"),
            token,
            Some(json!({ "child": child, "quantity": quantity })),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
}

/// The BOM as `child number → quantity`, sorted by number.
async fn bom(app: &common::TestApp, token: &str, number: &str) -> Vec<(String, f64)> {
    let r = app
        .api(
            Method::GET,
            &format!("/api/v1/items/{number}/revisions/A/bom"),
            token,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let mut lines: Vec<(String, f64)> = r
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["child_number"].as_str().unwrap().to_string(),
                l["quantity"].as_f64().unwrap(),
            )
        })
        .collect();
    lines.sort_by(|a, b| a.0.cmp(&b.0));
    lines
}

async fn revision_id(app: &common::TestApp, number: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT r.id FROM revisions r JOIN items i ON i.id = r.item_id
         WHERE i.number = $1 AND r.code = 'A'",
    )
    .bind(number)
    .fetch_one(&app.pool)
    .await
    .unwrap()
}

async fn checked_out(app: &common::TestApp, number: &str) -> bool {
    sqlx::query_scalar(
        "SELECT r.checked_out_by IS NOT NULL FROM revisions r JOIN items i ON i.id = r.item_id
         WHERE i.number = $1 AND r.code = 'A'",
    )
    .bind(number)
    .fetch_one(&app.pool)
    .await
    .unwrap()
}

/// The `details` of the `bom.from_references` audit entries of a revision.
async fn bom_audits(app: &common::TestApp, revision: Uuid) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT details FROM audit_log
         WHERE action = 'bom.from_references' AND entity_id = $1 ORDER BY id",
    )
    .bind(revision)
    .fetch_all(&app.pool)
    .await
    .unwrap()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap_or_else(|| panic!("not a list: {v}"))
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

/// New children added, a differing CAD count corrected, an unreferenced line
/// kept and reported, a missing file reported – in the check-in's
/// transaction, with one `bom.from_references` audit entry.
#[tokio::test]
async fn a_checkin_with_the_box_updates_the_bom_from_the_cad_file() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["ROCKET"]).await;
    let bolt = item(&app, &eng, "part").await;
    first(&app, &eng, &bolt, "bolt.stp", b"ISO-10303-21;\n").await;
    let nut = item(&app, &eng, "part").await;
    first(&app, &eng, &nut, "nut.stp", b"ISO-10303-21;\n").await;
    // glue has no CAD geometry: a BOM line only
    let glue = item(&app, &eng, "part").await;
    first(&app, &eng, &glue, "glue.txt", b"2K epoxy").await;

    // version 1 places the bolt once; checked in without the field: no BOM
    let asm = item(&app, &eng, "assembly").await;
    first(
        &app,
        &eng,
        &asm,
        "pump.stp",
        &step_assembly("pump.stp", &[("bolt.stp", 1)]),
    )
    .await;
    assert!(bom(&app, &eng, &asm).await.is_empty(), "API default: off");

    checkout(&app, &eng, &asm).await;
    add_line(&app, &eng, &asm, &bolt, 1.0).await;
    add_line(&app, &eng, &asm, &glue, 2.0).await;
    // version 2: four bolts, two nuts and a file TeamCraft does not have
    let v2 = step_assembly(
        "pump.stp",
        &[("bolt.stp", 4), ("nut.stp", 2), ("ghost.stp", 1)],
    );
    let r = checkin(
        &app,
        &eng,
        &asm,
        &[("pump.stp", v2.as_slice())],
        &[("comment", "more bolts"), ("bom_from_references", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    let result = &body["bom"];
    assert_eq!(
        strings(&result["added"]),
        std::slice::from_ref(&nut),
        "{body}"
    );
    assert_eq!(
        strings(&result["updated"]),
        std::slice::from_ref(&bolt),
        "{body}"
    );
    assert_eq!(
        strings(&result["kept_not_in_cad"]),
        std::slice::from_ref(&glue),
        "{body}"
    );
    assert_eq!(
        strings(&result["not_in_teamcraft"]),
        ["ghost.stp"],
        "{body}"
    );
    assert_eq!(result["refused"], json!([]), "{body}");
    assert!(body["revision"]["checked_out_by"].is_null(), "{body}");

    let mut expected = vec![(bolt.clone(), 4.0), (glue.clone(), 2.0), (nut.clone(), 2.0)];
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(bom(&app, &eng, &asm).await, expected, "glue is kept");

    let rev = revision_id(&app, &asm).await;
    let audits = bom_audits(&app, rev).await;
    assert_eq!(audits.len(), 1, "{audits:?}");
    assert_eq!(audits[0]["source"], "checkin");
    assert_eq!(audits[0]["added"], 1);
    assert_eq!(audits[0]["updated"], 1);
    let actions = app.audit_actions(rev).await;
    assert!(actions.contains(&"bom.update".to_string()), "{actions:?}");
    assert!(
        actions.contains(&"revision.checkin".to_string()),
        "{actions:?}"
    );

    // the same check-in again changes nothing: no further audit entry
    checkout(&app, &eng, &asm).await;
    let r = checkin(&app, &eng, &asm, &[], &[("bom_from_references", "true")]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let again = r.json();
    assert_eq!(again["bom"]["added"], json!([]), "{again}");
    assert_eq!(again["bom"]["updated"], json!([]), "{again}");
    assert_eq!(
        strings(&again["bom"]["kept_not_in_cad"]),
        std::slice::from_ref(&glue)
    );
    assert_eq!(bom_audits(&app, rev).await.len(), 1);
    app.teardown().await;
}

/// An *assumed 1* (a file without instance counts) adds a line, but never
/// replaces the quantity of an existing one.
#[tokio::test]
async fn an_assumed_quantity_never_overwrites_an_existing_line() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["ROCKET"]).await;
    let plate = item(&app, &eng, "part").await;
    first(&app, &eng, &plate, "Plate.SLDPRT", b"\x00plate").await;
    let bolt = item(&app, &eng, "part").await;
    first(&app, &eng, &bolt, "Bolt.SLDPRT", b"\x00bolt").await;
    let asm = item(&app, &eng, "assembly").await;
    first(&app, &eng, &asm, "Stage.SLDASM", &sldasm(&["Plate.SLDPRT"])).await;

    checkout(&app, &eng, &asm).await;
    add_line(&app, &eng, &asm, &plate, 3.0).await;
    let r = checkin(
        &app,
        &eng,
        &asm,
        &[(
            "Stage.SLDASM",
            sldasm(&["Plate.SLDPRT", "Bolt.SLDPRT"]).as_slice(),
        )],
        &[("bom_from_references", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    assert_eq!(
        strings(&body["bom"]["added"]),
        std::slice::from_ref(&bolt),
        "{body}"
    );
    assert_eq!(body["bom"]["updated"], json!([]), "assumed: {body}");
    let mut expected = vec![(bolt.clone(), 1.0), (plate.clone(), 3.0)];
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(bom(&app, &eng, &asm).await, expected);
    app.teardown().await;
}

/// A child that would make the BOM circular is refused; the other children
/// are still added.
#[tokio::test]
async fn a_circular_child_is_refused_at_checkin() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["ROCKET"]).await;
    let part = item(&app, &eng, "part").await;
    first(&app, &eng, &part, "Plate.SLDPRT", b"\x00plate").await;
    let top = item(&app, &eng, "assembly").await;
    let sub = item(&app, &eng, "assembly").await;
    first(&app, &eng, &sub, "Sub.SLDASM", &sldasm(&["Plate.SLDPRT"])).await;
    // the sub-assembly already contains the top assembly
    checkout(&app, &eng, &sub).await;
    add_line(&app, &eng, &sub, &top, 1.0).await;

    let r = checkin(
        &app,
        &eng,
        &top,
        &[(
            "Top.SLDASM",
            sldasm(&["Sub.SLDASM", "Plate.SLDPRT"]).as_slice(),
        )],
        &[("bom_from_references", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    assert_eq!(
        strings(&body["bom"]["added"]),
        std::slice::from_ref(&part),
        "{body}"
    );
    assert_eq!(body["bom"]["refused"][0]["child"], json!(sub), "{body}");
    assert!(body["bom"]["refused"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("circular"));
    assert_eq!(bom(&app, &eng, &top).await, [(part.clone(), 1.0)]);
    app.teardown().await;
}

/// A park (upload without check-in) never touches the BOM, and a refused
/// check-in (409: the file moved on since its base version) leaves files and
/// BOM as they were – the revision stays checked out.
#[tokio::test]
async fn park_and_a_refused_checkin_leave_the_bom_alone() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["ROCKET"]).await;
    let bolt = item(&app, &eng, "part").await;
    first(&app, &eng, &bolt, "bolt.stp", b"ISO-10303-21;\n").await;
    let asm = item(&app, &eng, "assembly").await;
    first(
        &app,
        &eng,
        &asm,
        "pump.stp",
        &step_assembly("pump.stp", &[("bolt.stp", 1)]),
    )
    .await;

    // park version 2 (four bolts): the files endpoint, the lock stays
    checkout(&app, &eng, &asm).await;
    let v2 = step_assembly("pump.stp", &[("bolt.stp", 4)]);
    let r = app
        .api_multipart(
            &format!("/api/v1/items/{asm}/revisions/A/files"),
            &eng,
            &[("pump.stp", v2.as_slice())],
            &[("bom_from_references", "true")],
        )
        .await;
    assert!(r.status.is_success(), "{}", r.text());
    assert!(
        bom(&app, &eng, &asm).await.is_empty(),
        "park: BOM untouched"
    );
    assert!(checked_out(&app, &asm).await);

    // a check-in based on version 1 is refused: nothing changes
    let v3 = step_assembly("pump.stp", &[("bolt.stp", 6)]);
    let bases = json!({ "pump.stp": 1 }).to_string();
    let r = checkin(
        &app,
        &eng,
        &asm,
        &[("pump.stp", v3.as_slice())],
        &[("bases", bases.as_str()), ("bom_from_references", "true")],
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert!(
        bom(&app, &eng, &asm).await.is_empty(),
        "refused: BOM untouched"
    );
    assert!(checked_out(&app, &asm).await, "the lock stays");
    let rev = revision_id(&app, &asm).await;
    assert!(bom_audits(&app, rev).await.is_empty());

    // the check-in of the parked version with the box: four bolts
    let r = checkin(&app, &eng, &asm, &[], &[("bom_from_references", "true")]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(bom(&app, &eng, &asm).await, [(bolt.clone(), 4.0)]);
    app.teardown().await;
}

/// A revision without a structure file is left alone (`bom` is null), and
/// a field that is not "true" counts as unticked.
#[tokio::test]
async fn no_structure_file_or_an_unticked_box_leaves_the_bom_alone() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["ROCKET"]).await;
    let bolt = item(&app, &eng, "part").await;
    first(&app, &eng, &bolt, "bolt.stp", b"ISO-10303-21;\n").await;
    let doc = item(&app, &eng, "part").await;
    first(&app, &eng, &doc, "notes.txt", b"bolt.stp").await;
    checkout(&app, &eng, &doc).await;
    let r = checkin(&app, &eng, &doc, &[], &[("bom_from_references", "true")]).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json()["bom"].is_null(), "{}", r.text());

    let asm = item(&app, &eng, "assembly").await;
    let v1 = step_assembly("pump.stp", &[("bolt.stp", 2)]);
    let r = checkin(
        &app,
        &eng,
        &asm,
        &[("pump.stp", v1.as_slice())],
        &[("bom_from_references", "false")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json()["bom"].is_null());
    assert!(bom(&app, &eng, &asm).await.is_empty());
    app.teardown().await;
}

/// The chunked path: `check_in` with `bom_from_references` on
/// `POST /uploads/{id}/finish`.
#[tokio::test]
async fn a_chunked_checkin_updates_the_bom_too() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["ROCKET"]).await;
    let bolt = item(&app, &eng, "part").await;
    first(&app, &eng, &bolt, "bolt.stp", b"ISO-10303-21;\n").await;
    let asm = item(&app, &eng, "assembly").await;
    let content = step_assembly("pump.stp", &[("bolt.stp", 3)]);
    let r = app
        .api(
            Method::POST,
            "/api/v1/uploads",
            &eng,
            Some(
                json!({ "item": asm, "revision": "A", "filename": "pump.stp",
                          "size": content.len() }),
            ),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_string();
    let request = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/v1/uploads/{id}"))
        .header(header::AUTHORIZATION, format!("Bearer {eng}"))
        .header(header::CONTENT_TYPE, "application/offset+octet-stream")
        .header("Upload-Offset", "0")
        .body(Body::from(content.clone()))
        .unwrap();
    let r = app.send(request).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let r = app
        .api(
            Method::POST,
            &format!("/api/v1/uploads/{id}/finish"),
            &eng,
            Some(json!({ "check_in": true, "bom_from_references": true })),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let body = r.json();
    assert_eq!(
        strings(&body["bom"]["added"]),
        std::slice::from_ref(&bolt),
        "{body}"
    );
    assert_eq!(bom(&app, &eng, &asm).await, [(bolt.clone(), 3.0)]);
    app.teardown().await;
}

/// The web check-in form offers the box (ticked) on an assembly and the
/// form's field updates the BOM.
#[tokio::test]
async fn the_web_checkin_form_has_the_box_ticked() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["ROCKET"]).await;
    let bolt = item(&app, &eng, "part").await;
    first(&app, &eng, &bolt, "bolt.stp", b"ISO-10303-21;\n").await;
    let asm = item(&app, &eng, "assembly").await;
    first(
        &app,
        &eng,
        &asm,
        "pump.stp",
        &step_assembly("pump.stp", &[("bolt.stp", 2)]),
    )
    .await;
    checkout(&app, &eng, &asm).await;
    let rev = revision_id(&app, &asm).await;
    let cookie = app.login("eng").await;
    let page = app
        .get_page(&format!("/revisions/{rev}"), Some(&cookie))
        .await;
    assert_eq!(page.status, StatusCode::OK);
    let html = page.text();
    assert!(
        html.contains(r#"name="bom_from_references" value="true" checked"#),
        "the box, ticked"
    );

    let boundary = "teamcraft-test-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"comment\"\r\n\r\nweb\r\n\
         --{boundary}\r\nContent-Disposition: form-data; name=\"bom_from_references\"\r\n\r\ntrue\r\n\
         --{boundary}--\r\n"
    );
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/revisions/{rev}/checkin"))
        .header(header::HOST, "localhost:8080")
        .header(header::ORIGIN, "http://localhost:8080")
        .header(header::COOKIE, &cookie)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let r = app.send(request).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.text());
    assert_eq!(bom(&app, &eng, &asm).await, [(bolt.clone(), 2.0)]);
    assert!(!checked_out(&app, &asm).await);
    app.teardown().await;
}
