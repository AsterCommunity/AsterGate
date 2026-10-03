use aster_forge_test::temp::SqliteTestDatabase;
use aster_gate::{
    db::entities::{
        account, account_challenge as challenge, account_identifier as identifier,
        browser_session as session, password_credential as credential,
    },
    runtime::AppState,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, sea_query::Expr,
};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery staple";
const NEW_PASSWORD: &str = "a replacement secret password";

mod support;
use support::*;
mod architecture;
mod email;
mod lifecycle;
mod mail;
mod password;
mod permissions;
mod registration;
mod security;
mod settings;
mod setup;
mod sites;
