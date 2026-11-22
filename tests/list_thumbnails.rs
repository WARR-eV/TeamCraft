//! Thumbnails in lists (#420): the main thumbnail of a revision, its route
//! on web and API (read rights, ETag, 404 without a picture) and the
//! thumbnail fields of the item search and the vault explorer, filled for a
//! whole page in one batched query (the statement count of a list page does
//! not grow with its rows).

mod common;

use std::io::Write;

use axum::http::{header, Method, StatusCode};
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

/// A minimal PNG (`w` × `h`); the extractor checks the chunk structure only.
fn tiny_png(w: u32, h: u32) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32fast::hash(&body).to_be_bytes());
    }
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = w.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(
        &mut out,
        b"IDAT",
        &[0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
    );
    chunk(&mut out, b"IEND", &[]);
    out
}

/// An Inventor-like file: OLE compound file, PNG in the summary information.
fn inventor_file(png: &[u8]) -> Vec<u8> {
    let mut ole = cfb::CompoundFile::create(std::io::Cursor::new(Vec::new())).unwrap();
    let mut summary = vec![0u8; 48];
    summary.extend_from_slice(png);
    ole.create_stream("/\u{5}SummaryInformation")
        .unwrap()
        .write_all(&summary)
        .unwrap();
    ole.flush().unwrap();
    ole.into_inner().into_inner()
}

/// Creates an item in GEARBOX and returns its number.
async fn new_part(app: &common::TestApp, token: &str, name: &str) -> String {
    app.create_item(
        token,
        json!({ "item_type": "part", "name": name, "project": "GEARBOX" }),
    )
    .await
}

async fn upload(
    app: &common::TestApp,
    token: &str,
    number: &str,
    files: &[(&str, &[u8])],
) -> Value {
    let r = app
        .api_multipart(
            &format!("/api/v1/items/{number}/revisions/A/files"),
            token,
            files,
            &[],
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    r.json()
}

fn file_id(files: &Value, name: &str) -> String {
    files
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["filename"] == name)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn revision_id(app: &common::TestApp, token: &str, number: &str) -> String {
    app.api(
        Method::GET,
        &format!("/api/v1/items/{number}/revisions/A"),
        token,
        None,
    )
    .await
    .json()["revision"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn revision_thumbnail_route_and_list_fields() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["GEARBOX"]).await;
    let (_, outsider) = app.user("outsider", Role::Viewer).await;
    let cookie = app.login("eng").await;
    let outsider_cookie = app.login("outsider").await;

    // a part with a drawing, a part model and a STEP export: the part wins
    let bracket = new_part(&app, &eng, "Bracket").await;
    let png = tiny_png(64, 64);
    let drawing_png = tiny_png(32, 32);
    let files = upload(
        &app,
        &eng,
        &bracket,
        &[
            ("bracket.idw", &inventor_file(&drawing_png)),
            ("bracket.ipt", &inventor_file(&png)),
            ("bracket.step", b"ISO-10303-21;"),
        ],
    )
    .await;
    let ipt_id = file_id(&files, "bracket.ipt");
    let bracket_rev = revision_id(&app, &eng, &bracket).await;
    // an item with only a STEP file has no thumbnail, one without files neither
    let plate = new_part(&app, &eng, "Plate").await;
    upload(&app, &eng, &plate, &[("plate.step", b"ISO-10303-21;")]).await;
    let plate_rev = revision_id(&app, &eng, &plate).await;
    let empty = new_part(&app, &eng, "Empty").await;
    let empty_rev = revision_id(&app, &eng, &empty).await;

    // API: the picture of the main file, with its ETag
    let api_uri = format!("/api/v1/revisions/{bracket_rev}/thumbnail");
    let r = app.api(Method::GET, &api_uri, &eng, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.header("content-type").as_deref(), Some("image/png"));
    assert_eq!(r.body, png, "the part, not the drawing");
    let etag = r.header("etag").expect("an ETag");
    // a conditional request answers 304 without the image
    let request = axum::http::Request::builder()
        .uri(&api_uri)
        .header(header::AUTHORIZATION, format!("Bearer {eng}"))
        .header(header::IF_NONE_MATCH, &etag)
        .body(axum::body::Body::empty())
        .unwrap();
    let r = app.send(request).await;
    assert_eq!(r.status, StatusCode::NOT_MODIFIED);
    assert!(r.body.is_empty());
    // no picture: 404
    for rev in [&plate_rev, &empty_rev] {
        let r = app
            .api(
                Method::GET,
                &format!("/api/v1/revisions/{rev}/thumbnail"),
                &eng,
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{rev}");
    }
    let r = app
        .api(
            Method::GET,
            &format!("/api/v1/revisions/{}/thumbnail", Uuid::new_v4()),
            &eng,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "unknown revision");

    // web: the same picture for the session
    let web_uri = format!("/revisions/{bracket_rev}/thumbnail");
    let r = app.get_page(&web_uri, Some(&cookie)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body, png);
    let r = app
        .get_page(&format!("/revisions/{empty_rev}/thumbnail"), Some(&cookie))
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    // list payloads: the item search and the favorites (explorer rows)
    let r = app
        .api(Method::GET, "/api/v1/items?project=GEARBOX", &eng, None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let items = r.json()["items"].as_array().unwrap().clone();
    let row = |number: &str| {
        items
            .iter()
            .find(|i| i["number"] == number)
            .unwrap_or_else(|| panic!("{number} listed"))
            .clone()
    };
    assert_eq!(row(&bracket)["thumbnail_file_id"], ipt_id.as_str());
    assert!(row(&bracket)["thumbnail_sha256"].is_string());
    assert!(row(&plate)["thumbnail_file_id"].is_null());
    assert!(row(&empty)["thumbnail_file_id"].is_null());
    let r = app
        .api(
            Method::POST,
            &format!("/api/v1/items/{bracket}/favorite"),
            &eng,
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let r = app.api(Method::GET, "/api/v1/favorites", &eng, None).await;
    assert_eq!(r.json()[0]["thumbnail_file_id"], ipt_id.as_str());

    // web lists: the item list (also as search results) and the vault explorer
    let thumb = format!("/files/{ipt_id}/thumbnail?v=");
    for page in [
        "/items".to_string(),
        "/items?q=Bracket".to_string(),
        "/vault?q=Bracket".to_string(),
    ] {
        let html = app.get_page(&page, Some(&cookie)).await.text();
        assert!(html.contains(&thumb), "{page} shows the thumbnail");
        assert!(html.contains("loading=\"lazy\""), "{page} loads it lazily");
    }

    // the group becomes restricted: an outsider gets 404 on web and API
    sqlx::query("UPDATE projects SET visibility = 'restricted' WHERE code = 'GEARBOX'")
        .execute(&app.pool)
        .await
        .unwrap();
    let r = app.api(Method::GET, &api_uri, &outsider, None).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text());
    let r = app.get_page(&web_uri, Some(&outsider_cookie)).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = app
        .api(Method::GET, "/api/v1/items?q=Bracket", &outsider, None)
        .await;
    assert_eq!(r.json()["total"], 0);
    // the member still sees it
    let r = app.api(Method::GET, &api_uri, &eng, None).await;
    assert_eq!(r.status, StatusCode::OK);
    app.teardown().await;
}

/// The thumbnails of a whole page come from one call for all revisions
/// (one query, whatever the page size), with the same rule as the revision
/// page.
#[tokio::test]
async fn thumbnails_of_many_revisions_in_one_call() {
    use teamcraft::services::thumbnails;
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["GEARBOX"]).await;
    let png = tiny_png(16, 16);
    let mut with_picture = Vec::new();
    let mut without = Vec::new();
    for n in 0..12 {
        let number = new_part(&app, &eng, &format!("Part {n}")).await;
        let rev: Uuid = revision_id(&app, &eng, &number).await.parse().unwrap();
        if n % 3 == 0 {
            upload(&app, &eng, &number, &[("p.step", b"ISO-10303-21;")]).await;
            without.push(rev);
        } else {
            let files = upload(
                &app,
                &eng,
                &number,
                &[
                    ("z.idw", &inventor_file(&png)),
                    ("a.ipt", &inventor_file(&png)),
                ],
            )
            .await;
            with_picture.push((rev, file_id(&files, "a.ipt")));
        }
    }
    let mut ids: Vec<Uuid> = with_picture.iter().map(|(r, _)| *r).collect();
    ids.extend(&without);
    let thumbs = thumbnails::main_for_revisions(&app.pool, &ids)
        .await
        .unwrap();
    assert_eq!(thumbs.len(), with_picture.len());
    for (rev, file) in &with_picture {
        assert_eq!(
            thumbs[rev].file_id.to_string(),
            *file,
            "the part, not the drawing"
        );
        assert_eq!(thumbs[rev].sha256.len(), 64);
    }
    for rev in &without {
        assert!(!thumbs.contains_key(rev));
    }
    assert!(thumbnails::main_for_revisions(&app.pool, &[])
        .await
        .unwrap()
        .is_empty());
    app.teardown().await;
}

/// The SQL statements sqlx runs while a future is polled on this thread
/// (sqlx reports every statement as a `sqlx::query` tracing event). The
/// tests run on a current-thread runtime, so the router's work is seen and
/// other tests' work is not.
#[derive(Clone, Default)]
struct Statements(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Statements {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        if event.metadata().target() != "sqlx::query" {
            return;
        }
        let mut text = String::new();
        event.record(
            &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| {
                if field.name() == "message" || field.name() == "db.statement" {
                    text.push_str(&format!("{value:?} "));
                }
            },
        );
        self.0.lock().unwrap().push(text);
    }
}

async fn statements_of<F: std::future::Future>(work: F) -> Vec<String> {
    use tracing_subscriber::layer::SubscriberExt;
    let seen = Statements::default();
    {
        let _scope =
            tracing::subscriber::set_default(tracing_subscriber::registry().with(seen.clone()));
        work.await;
    }
    let mut statements = seen.0.lock().unwrap();
    std::mem::take(&mut *statements)
}

/// The list paths ask for the thumbnails of a page in one go: the number of
/// statements of the item search, the structure roots and the vault explorer
/// stays the same whether the page has 3 or 9 items with a picture (#420).
#[tokio::test]
async fn list_query_count_stays_constant() {
    let app = app_or_skip!();
    let (_, eng) = app.user_in("eng", Role::Editor, &["GEARBOX"]).await;
    let cookie = app.login("eng").await;
    let png = tiny_png(16, 16);
    let pages = [
        "/api/v1/items?project=GEARBOX&q=Counted",
        "/api/v1/projects/GEARBOX/structure",
        "/vault?q=Counted",
    ];
    let measure = |uri: &'static str| {
        let (app, eng, cookie) = (&app, &eng, &cookie);
        async move {
            statements_of(async {
                let r = if uri.starts_with("/api/") {
                    app.api(Method::GET, uri, eng, None).await
                } else {
                    app.get_page(uri, Some(cookie.as_str())).await
                };
                assert_eq!(r.status, StatusCode::OK, "{uri}: {}", r.text());
            })
            .await
        }
    };
    let mut before = Vec::new();
    for (round, range) in [0..3, 3..9].into_iter().enumerate() {
        for n in range {
            let number = new_part(&app, &eng, &format!("Counted {n}")).await;
            upload(&app, &eng, &number, &[("c.ipt", &inventor_file(&png))]).await;
        }
        let mut counts = Vec::new();
        for uri in pages {
            // once before counting: first-use updates (token, session) are not
            // part of the list. Work left over from the uploads (previews, text
            // index) only adds statements, so the smallest of three runs counts.
            measure(uri).await;
            let mut fewest = measure(uri).await;
            for _ in 0..2 {
                let run = measure(uri).await;
                if run.len() < fewest.len() {
                    fewest = run;
                }
            }
            counts.push(fewest);
        }
        if round == 0 {
            before = counts;
        } else {
            for ((uri, small), large) in pages.iter().zip(&before).zip(&counts) {
                assert!(!small.is_empty(), "{uri}: statements are seen");
                assert_eq!(
                    small.len(),
                    large.len(),
                    "{uri}: 3 items ran {small:#?}, 9 items ran {large:#?}"
                );
            }
        }
    }
    // the rows carry their thumbnails
    let r = app.api(Method::GET, pages[0], &eng, None).await;
    let items = r.json()["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 9);
    assert!(items.iter().all(|i| i["thumbnail_file_id"].is_string()));
    app.teardown().await;
}
