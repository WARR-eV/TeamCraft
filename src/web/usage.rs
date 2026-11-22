//! *Admin → Usage statistics* (#383): what the desktop apps of this server
//! report, the switch for sharing an aggregate with the TeamCraft
//! developers (off by default) and the exact JSON that would be sent.

use axum::{extract::State, response::Response};
use serde::Deserialize;
use serde_json::json;

use super::{finish, render, WebCtx};
use crate::{domain::Role, error::AppResult, services::usage, AppState};

pub async fn page(State(st): State<AppState>, ctx: WebCtx) -> AppResult<Response> {
    ctx.me.require(Role::Admin)?;
    let o = usage::overview(&st.db, &st.config).await?;
    let pretty = |v: &serde_json::Value| serde_json::to_string_pretty(v).unwrap_or_default();
    render(
        &st,
        &ctx,
        "admin/usage.html",
        json!({
            "sharing": o.sharing,
            "desktop": o.desktop,
            "next": pretty(&o.next),
            "last": o.last.as_ref().map(pretty),
            "last_sent_at": o.last_sent_at.map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string()),
            "actions": usage::ACTIONS,
        }),
    )
}

#[derive(Deserialize)]
pub struct ShareForm {
    /// `"1"` turns sharing on, anything else off.
    share: String,
}

pub async fn set_share(
    State(st): State<AppState>,
    ctx: WebCtx,
    axum::Form(form): axum::Form<ShareForm>,
) -> AppResult<Response> {
    let on = form.share == "1";
    let result = usage::set_share(&st.db, &st.config, &ctx.me, on).await;
    finish(&st, result, "/admin/usage", |()| {
        (
            "/admin/usage".to_string(),
            if on {
                "Usage statistics are shared with the TeamCraft developers once a day."
            } else {
                "Usage statistics are no longer shared."
            }
            .to_string(),
        )
    })
}
