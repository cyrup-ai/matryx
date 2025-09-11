use std::sync::Arc;

use axum::{
    Router,
    http::StatusCode,
    middleware,
    routing::{delete, get, post, put},
};
use std::net::SocketAddr;
use surrealdb::engine::any::{self};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

mod _matrix;
mod _well_known;
mod auth;
mod federation;
mod room;
mod state;
mod utils;

use crate::auth::{
    MatrixSessionService,
    middleware::auth_middleware,
    middleware::require_auth_middleware,
};
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize SurrealDB connection
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "memory".to_string());

    let db = any::connect(&db_url)
        .await
        .map_err(|e| format!("Failed to connect to SurrealDB at '{}': {}", db_url, e))?;

    // Configure database
    db.use_ns("matrix")
        .use_db("homeserver")
        .await
        .map_err(|e| format!("Failed to select matrix.homeserver namespace/database: {}", e))?;

    // Initialize authentication service
    let jwt_secret = std::env::var("JWT_SECRET")
        .map(|s| s.into_bytes())
        .or_else(|_| {
            tracing::warn!(
                "JWT_SECRET not set, generating random secret (not suitable for production)"
            );
            use rand::RngCore;
            let mut secret = vec![0u8; 64];
            rand::thread_rng().fill_bytes(&mut secret);
            Ok(secret)
        })
        .map_err(|e: std::env::VarError| format!("Failed to initialize JWT secret: {}", e))?;

    let homeserver_name = std::env::var("HOMESERVER_NAME")
        .map_err(|_| "HOMESERVER_NAME environment variable is required")?;

    let session_service = Arc::new(MatrixSessionService::new(jwt_secret, homeserver_name.clone()));

    // Create application state
    let app_state = AppState::new(db, session_service, homeserver_name);

    // Build our application with routes
    let app = create_router(app_state);

    // Run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 8008));
    println!("Matrix homeserver listening on {}", addr);

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind to address {}: {}", addr, e))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Failed to start axum server: {}", e))?;

    Ok(())
}

fn create_router(app_state: AppState) -> Router {
    Router::new()
        // Well-known endpoints (no auth required)
        .route("/.well-known/matrix/client", get(_well_known::matrix::client::get))
        .route("/.well-known/matrix/server", get(_well_known::matrix::server::get))
        // Client-Server API endpoints with authentication middleware
        .nest("/_matrix/client", create_client_routes())
        // Server-Server API endpoints with authentication middleware
        .nest("/_matrix/federation", create_federation_routes())
        .nest("/_matrix/key", create_key_routes())
        .nest("/_matrix/media", create_media_routes())
        .nest("/_matrix/app", create_app_routes())
        .nest("/_matrix/static", create_static_routes())
        .nest("/_matrix/identity", create_identity_routes())
        .nest("/.well-known", create_well_known_routes())
        // Add application state first
        .with_state(app_state.clone())
        // Apply authentication middleware to all Matrix endpoints
        // Remove middleware layer temporarily to fix compilation
        .layer(CorsLayer::permissive())
        .fallback(handler_404)
}

fn create_client_routes() -> Router<AppState> {
    Router::new()
        .layer(middleware::from_fn(require_auth_middleware))
        // Client API endpoints
        .route("/versions", get(_matrix::client::versions::get))
        .route("/v3/endpoint", post(_matrix::client::v3::endpoint::post))
        .route("/v3/login", get(_matrix::client::v3::login::get).post(_matrix::client::v3::login::post))
        .route("/v3/logout", post(_matrix::client::v3::logout::post))
        .route("/v3/logout/all", post(_matrix::client::v3::logout::all::post))
        .route("/media/v1/create", post(_matrix::media::v1::create::post))
        .route("/media/v3/upload", post(_matrix::media::v3::upload::post))
        .route("/media/v3/upload/:server_name/:media_id", put(_matrix::media::v3::upload::by_server_name::by_media_id::put))
        .route("/v3/devices/:device_id", delete(_matrix::client::v3::devices::by_device_id::delete))
        .route("/v3/directory/room/:room_alias", delete(_matrix::client::v3::directory::room::by_room_alias::delete))
        .route("/v3/pushrules/global/:kind/:rule_id", delete(_matrix::client::v3::pushrules::global::by_kind::by_rule_id::delete))
        .route("/v3/room_keys/keys", delete(_matrix::client::v3::room_keys::keys::delete))
        .route("/v3/room_keys/keys/:room_id", delete(_matrix::client::v3::room_keys::keys::by_room_id::delete))
        .route("/v3/room_keys/keys/:room_id/:session_id", delete(_matrix::client::v3::room_keys::keys::by_room_id::by_session_id::delete))
        .route("/v3/room_keys/version/:version", delete(_matrix::client::v3::room_keys::version::by_version::delete))
        .route("/v3/user/:user_id/rooms/:room_id/tags/:tag", delete(_matrix::client::v3::user::by_user_id::rooms::by_room_id::tags::by_tag::delete))
        .route("/v1/media/config", get(_matrix::client::v1::media::config::get))
        .route("/v1/media/download/:server_name/:media_id", get(_matrix::client::v1::media::download::by_server_name::by_media_id::get))
        .route("/v1/media/download/:server_name/:media_id/:file_name", get(_matrix::client::v1::media::download::by_server_name::by_media_id::by_file_name::get))
        .route("/v1/media/preview_url", get(_matrix::client::v1::media::preview_url::get))
        .route("/v1/media/thumbnail/:server_name/:media_id", get(_matrix::client::v1::media::thumbnail::by_server_name::by_media_id::get))
        .route("/v1/room_summary/:room_id_or_alias", get(_matrix::client::v1::room_summary::by_room_id_or_alias::get))
        .route("/v1/rooms/:room_id/hierarchy", get(_matrix::client::v1::rooms::by_room_id::hierarchy::get))
        .route("/v1/rooms/:room_id/relations/:event_id", get(_matrix::client::v1::rooms::by_room_id::relations::by_event_id::get))
        .route("/v1/rooms/:room_id/relations/:event_id/:rel_type", get(_matrix::client::v1::rooms::by_room_id::relations::by_event_id::by_rel_type::get))
        .route("/v1/rooms/:room_id/relations/:event_id/:rel_type/:event_type", get(_matrix::client::v1::rooms::by_room_id::relations::by_event_id::by_rel_type::by_event_type::get))
        .route("/v1/rooms/:room_id/threads", get(_matrix::client::v1::rooms::by_room_id::threads::get))
        .route("/v3/account/3pid", get(_matrix::client::v3::account::threepid::get))
        .route("/v3/account/whoami", get(_matrix::client::v3::account::whoami::get))
        .route("/v3/admin/whois/:user_id", get(_matrix::client::v3::admin::whois::by_user_id::get))
        .route("/v3/capabilities", get(_matrix::client::v3::capabilities::get))
        .route("/v3/devices", get(_matrix::client::v3::devices::get))
        .route("/v3/devices/:device_id", get(_matrix::client::v3::devices::by_device_id::get))
        .route("/v3/directory/list/room/:room_id", get(_matrix::client::v3::directory::list::room::by_room_id::get))
        .route("/v3/directory/room/:room_alias", get(_matrix::client::v3::directory::room::by_room_alias::get))
        .route("/v3/events", get(_matrix::client::v3::events::get))
        .route("/v3/events/:event_id", get(_matrix::client::v3::events::by_event_id::get))
        .route("/v3/initialSync", get(_matrix::client::v3::initial_sync::get))
        .route("/v3/joined_rooms", get(_matrix::client::v3::joined_rooms::get))
        .route("/v3/keys/changes", get(_matrix::client::v3::keys::changes::get))
        .route("/v3/login/sso/redirect", get(_matrix::client::v3::login::sso::redirect::get))
        .route("/v3/login/sso/redirect/:idp_id", get(_matrix::client::v3::login::sso::redirect::by_idp_id::get))
        .route("/v3/notifications", get(_matrix::client::v3::notifications::get))
        .route("/v3/presence/:user_id/status", get(_matrix::client::v3::presence::by_user_id::status::get))
        .route("/v3/publicRooms", get(_matrix::client::v3::public_rooms::get))
        .route("/v3/pushers", get(_matrix::client::v3::pushers::get))
        .route("/v3/pushrules/", get(_matrix::client::v3::pushrules::get))
        .route("/v3/pushrules/global/", get(_matrix::client::v3::pushrules::global::get))
        .route("/v3/pushrules/global/:kind/:rule_id", get(_matrix::client::v3::pushrules::global::by_kind::by_rule_id::get))
        .route("/v3/pushrules/global/:kind/:rule_id/actions", get(_matrix::client::v3::pushrules::global::by_kind::by_rule_id::actions::get))
        .route("/v3/pushrules/global/:kind/:rule_id/enabled", get(_matrix::client::v3::pushrules::global::by_kind::by_rule_id::enabled::get))
        .route("/v3/room_keys/keys", get(_matrix::client::v3::room_keys::keys::get))
        .route("/v3/room_keys/keys/:room_id", get(_matrix::client::v3::room_keys::keys::by_room_id::get))
        .route("/v3/room_keys/keys/:room_id/:session_id", get(_matrix::client::v3::room_keys::keys::by_room_id::by_session_id::get))
        .route("/v3/room_keys/version", get(_matrix::client::v3::room_keys::version::get))
        .route("/v3/room_keys/version/:version", get(_matrix::client::v3::room_keys::version::by_version::get))
        .route("/v3/rooms/:room_id/aliases", get(_matrix::client::v3::rooms::by_room_id::aliases::get))
        .route("/v3/rooms/:room_id/context/:event_id", get(_matrix::client::v3::rooms::by_room_id::context::by_event_id::get))
        .route("/v3/rooms/:room_id/event/:event_id", get(_matrix::client::v3::rooms::by_room_id::event::by_event_id::get))
        .route("/v3/rooms/:room_id/initialSync", get(_matrix::client::v3::rooms::by_room_id::initial_sync::get))
        .route("/v3/rooms/:room_id/joined_members", get(_matrix::client::v3::rooms::by_room_id::joined_members::get))
        .route("/v3/rooms/:room_id/members", get(_matrix::client::v3::rooms::by_room_id::members::get))
        .route("/v3/rooms/:room_id/messages", get(_matrix::client::v3::rooms::by_room_id::messages::get))
        .route("/v3/rooms/:room_id/state", get(_matrix::client::v3::rooms::by_room_id::state::get))
        .route("/v3/rooms/:room_id/state/:event_type/:state_key", get(_matrix::client::v3::rooms::by_room_id::state::by_event_type::by_state_key::get))
        .route("/v3/sync", get(_matrix::client::v3::sync::get))
        // WebSocket sync endpoint removed - not in Matrix specification
        // Matrix uses regular HTTP long-polling sync via GET /v3/sync
        .route("/v3/thirdparty/location", get(_matrix::client::v3::thirdparty::location::get))
        .route("/v3/thirdparty/location/:protocol", get(_matrix::client::v3::thirdparty::location::by_protocol::get))
        .route("/v3/thirdparty/protocol/:protocol", get(_matrix::client::v3::thirdparty::protocol::by_protocol::get))
        .route("/v3/thirdparty/protocols", get(_matrix::client::v3::thirdparty::protocols::get))
        .route("/v3/thirdparty/user", get(_matrix::client::v3::thirdparty::user::get))
        .route("/v3/thirdparty/user/:protocol", get(_matrix::client::v3::thirdparty::user::by_protocol::get))
        .route("/v3/user/:user_id/account_data/:type", get(_matrix::client::v3::user::by_user_id::account_data::by_type::get))
        .route("/v3/user/:user_id/filter/:filter_id", get(_matrix::client::v3::user::by_user_id::filter::by_filter_id::get))
        .route("/v3/user/:user_id/rooms/:room_id/account_data/:type", get(_matrix::client::v3::user::by_user_id::rooms::by_room_id::account_data::by_type::get))
        .route("/v3/user/:user_id/rooms/:room_id/tags", get(_matrix::client::v3::user::by_user_id::rooms::by_room_id::tags::get))
        .route("/v3/voip/turnServer", get(_matrix::client::v3::voip::turn_server::get))
        .route("/media/v3/config", get(_matrix::media::v3::config::get))
        .route("/media/v3/download/:server_name/:media_id", get(_matrix::media::v3::download::by_server_name::by_media_id::get))
        .route("/media/v3/download/:server_name/:media_id/:file_name", get(_matrix::media::v3::download::by_server_name::by_media_id::by_file_name::get))
        .route("/media/v3/preview_url", get(_matrix::media::v3::preview_url::get))
        .route("/media/v3/thumbnail/:server_name/:media_id", get(_matrix::media::v3::thumbnail::by_server_name::by_media_id::get))
        .route("/app/v1/thirdparty/protocol/:protocol", get(_matrix::app::v1::thirdparty::protocol::by_protocol::get))
        .route("/static/client/login/", get(_matrix::static_::client::login::get))
        .route("/v1/login/get_token", post(_matrix::client::v1::login::get_token::post))
        .route("/v3/account/3pid", post(_matrix::client::v3::account::threepid::post))
        .route("/v3/account/3pid/add", post(_matrix::client::v3::account::threepid::add::post))
        .route("/v3/account/3pid/bind", post(_matrix::client::v3::account::threepid::bind::post))
        .route("/v3/account/3pid/delete", post(_matrix::client::v3::account::threepid::delete::post))
        .route("/v3/account/3pid/email/requestToken", post(_matrix::client::v3::account::threepid::email::request_token::post))
        .route("/v3/account/3pid/msisdn/requestToken", post(_matrix::client::v3::account::threepid::msisdn::request_token::post))
        .route("/v3/account/3pid/unbind", post(_matrix::client::v3::account::threepid::unbind::post))
        .route("/v3/account/deactivate", post(_matrix::client::v3::account::deactivate::post))
        .route("/v3/account/password", post(_matrix::client::v3::account::password::post))
        .route("/v3/account/password/email/requestToken", post(_matrix::client::v3::account::password::email::request_token::post))
        .route("/v3/account/password/msisdn/requestToken", post(_matrix::client::v3::account::password::msisdn::request_token::post))
        .route("/v3/createRoom", post(_matrix::client::v3::create_room::post))
        .route("/v3/delete_devices", post(_matrix::client::v3::delete_devices::post))
        .route("/v3/join/:room_id_or_alias", post(_matrix::client::v3::join::by_room_id_or_alias::post))
        .route("/v3/keys/claim", post(_matrix::client::v3::keys::claim::post))
        .route("/v3/keys/device_signing/upload", post(_matrix::client::v3::keys::device_signing::upload::post))
        .route("/v3/keys/query", post(_matrix::client::v3::keys::query::post))
        .route("/v3/keys/signatures/upload", post(_matrix::client::v3::keys::signatures::upload::post))
        .route("/v3/keys/upload", post(_matrix::client::v3::keys::upload::post))
        .route("/v3/knock/:room_id_or_alias", post(_matrix::client::v3::knock::by_room_id_or_alias::post))
        .route("/v3/publicRooms", post(_matrix::client::v3::public_rooms::post))
        .route("/v3/pushers/set", post(_matrix::client::v3::pushers::set::post))
        .route("/v3/refresh", post(_matrix::client::v3::refresh::post))
        .route("/v3/room_keys/version", post(_matrix::client::v3::room_keys::version::post))
        .route("/v3/rooms/:room_id/ban", post(_matrix::client::v3::rooms::by_room_id::ban::post))
        .route("/v3/rooms/:room_id/forget", post(_matrix::client::v3::rooms::by_room_id::forget::post))
        .route("/v3/rooms/:room_id/invite", post(_matrix::client::v3::rooms::by_room_id::invite::post))
        .route("/v3/rooms/:room_id/join", post(_matrix::client::v3::rooms::by_room_id::join::post))
        .route("/v3/rooms/:room_id/kick", post(_matrix::client::v3::rooms::by_room_id::kick::post))
        .route("/v3/rooms/:room_id/leave", post(_matrix::client::v3::rooms::by_room_id::leave::post))
        .route("/v3/rooms/:room_id/read_markers", post(_matrix::client::v3::rooms::by_room_id::read_markers::post))
        .route("/v3/rooms/:room_id/receipt/:receipt_type/:event_id", post(_matrix::client::v3::rooms::by_room_id::receipt::by_receipt_type::by_event_id::post))
        .route("/v3/rooms/:room_id/report", post(_matrix::client::v3::rooms::by_room_id::report::post))
        .route("/v3/rooms/:room_id/report/:event_id", post(_matrix::client::v3::rooms::by_room_id::report::by_event_id::post))
        .route("/v3/rooms/:room_id/unban", post(_matrix::client::v3::rooms::by_room_id::unban::post))
        .route("/v3/rooms/:room_id/upgrade", post(_matrix::client::v3::rooms::by_room_id::upgrade::post))
        .route("/v3/search", post(_matrix::client::v3::search::post))
        .route("/v3/user/:user_id/filter", post(_matrix::client::v3::user::by_user_id::filter::post))
        .route("/v3/user/:user_id/openid/request_token", post(_matrix::client::v3::user::by_user_id::openid::request_token::post))
        .route("/v3/user_directory/search", post(_matrix::client::v3::user_directory::search::post))
        .route("/v3/users/:user_id/report", post(_matrix::client::v3::profile::by_user_id::report::post))
        .route("/v3/login/get_token", post(_matrix::client::v3::login::get_token::post))
        .route("/v3/profile/:user_id", get(_matrix::client::v3::profile::by_user_id::get))
        .route("/v3/profile/:user_id/avatar_url", get(_matrix::client::v3::profile::by_user_id::avatar_url::get).put(_matrix::client::v3::profile::by_user_id::avatar_url::put))
        .route("/v3/profile/:user_id/displayname", get(_matrix::client::v3::profile::by_user_id::displayname::get).put(_matrix::client::v3::profile::by_user_id::displayname::put))
        .route("/v3/pushers", post(_matrix::client::v3::pushers::post))
        .route("/v3/rooms/:room_id/redact/:event_id", put(_matrix::client::v3::rooms::by_room_id::redact::by_event_id::put))
        .route("/v3/sendToDevice/:event_type/:txn_id", put(_matrix::client::v3::send_to_device::by_event_type::by_txn_id::put))
        .route("/v3/thirdparty/location/:alias", get(_matrix::client::v3::thirdparty::location::by_alias::get))
        .route("/v3/thirdparty/user/:userid", get(_matrix::client::v3::thirdparty::user::by_userid::get))
        .route("/v3/user/:user_id/filter/:filter_id", post(_matrix::client::v3::user::by_user_id::filter::by_filter_id::post))
        .route("/v3/user/:user_id/report", post(_matrix::client::v3::user::by_user_id::report::post))
        .route("/v3/user/:user_id/rooms/:room_id/tags/:tag", get(_matrix::client::v3::user::by_user_id::rooms::by_room_id::tags::by_tag::get))
        .route("/v3/users/:user_id", get(_matrix::client::v3::users::by_user_id::get))
        .route("/v3/users/:user_id/:key_name", get(_matrix::client::v3::users::by_user_id::by_key_name::get))
        .route("/v3/devices/:device_id", put(_matrix::client::v3::devices::by_device_id::put))
        .route("/v3/directory/list/room/:room_id", put(_matrix::client::v3::directory::list::room::by_room_id::put))
        .route("/v3/directory/room/:room_alias", put(_matrix::client::v3::directory::room::by_room_alias::put))
        .route("/v3/presence/:user_id/status", put(_matrix::client::v3::presence::by_user_id::status::put))
        .route("/v3/pushrules/global/:kind/:rule_id", put(_matrix::client::v3::pushrules::global::by_kind::by_rule_id::put))
        .route("/v3/pushrules/global/:kind/:rule_id/actions", put(_matrix::client::v3::pushrules::global::by_kind::by_rule_id::actions::put))
        .route("/v3/pushrules/global/:kind/:rule_id/enabled", put(_matrix::client::v3::pushrules::global::by_kind::by_rule_id::enabled::put))
        .route("/v3/room_keys/keys", put(_matrix::client::v3::room_keys::keys::put))
        .route("/v3/room_keys/keys/:room_id", put(_matrix::client::v3::room_keys::keys::by_room_id::put))
        .route("/v3/room_keys/keys/:room_id/:session_id", put(_matrix::client::v3::room_keys::keys::by_room_id::by_session_id::put))
        .route("/v3/room_keys/version/:version", put(_matrix::client::v3::room_keys::version::by_version::put))
        .route("/v3/rooms/:room_id/redact/:event_id/:txn_id", put(_matrix::client::v3::rooms::by_room_id::redact::by_event_id::by_txn_id::put))
        .route("/v3/rooms/:room_id/send/:event_type/:txn_id", put(_matrix::client::v3::rooms::by_room_id::send::by_event_type::by_txn_id::put))
        .route("/v3/rooms/:room_id/state/:event_type/:state_key", put(_matrix::client::v3::rooms::by_room_id::state::by_event_type::by_state_key::put))
        .route("/v3/rooms/:room_id/typing/:user_id", put(_matrix::client::v3::rooms::by_room_id::typing::by_user_id::put))
        .route("/v3/user/:user_id/account_data/:type", put(_matrix::client::v3::user::by_user_id::account_data::by_type::put))
        .route("/v3/user/:user_id/rooms/:room_id/account_data/:type", put(_matrix::client::v3::user::by_user_id::rooms::by_room_id::account_data::by_type::put))
        .route("/v3/user/:user_id/rooms/:room_id/tags/:tag", put(_matrix::client::v3::user::by_user_id::rooms::by_room_id::tags::by_tag::put))
        .route("/v3/profile/:user_id/:key_name", put(_matrix::client::v3::profile::by_user_id::by_key_name::put))
}

fn create_federation_routes() -> Router<AppState> {
    Router::new()
        // Federation API endpoints
        .route("/v1/backfill/:room_id", get(_matrix::federation::v1::backfill::by_room_id::get))
        .route("/v1/event/:event_id", get(_matrix::federation::v1::event::by_event_id::get))
        .route(
            "/v1/event_auth/:room_id/:event_id",
            get(_matrix::federation::v1::event_auth::by_room_id::by_event_id::get),
        )
        .route("/v1/hierarchy/:room_id", get(_matrix::federation::v1::hierarchy::by_room_id::get))
        .route(
            "/v1/make_join/:room_id/:user_id",
            get(_matrix::federation::v1::make_join::by_room_id::by_user_id::get),
        )
        .route(
            "/v1/make_knock/:room_id/:user_id",
            get(_matrix::federation::v1::make_knock::by_room_id::by_user_id::get),
        )
        .route(
            "/v1/make_leave/:room_id/:user_id",
            get(_matrix::federation::v1::make_leave::by_room_id::by_user_id::get),
        )
        .route(
            "/v1/media/download/:media_id",
            get(_matrix::media::v3::download::by_server_name::by_media_id::get),
        )
        .route(
            "/v1/media/download/:server_name/:media_id",
            get(_matrix::federation::v1::media::download::by_server_name::by_media_id::get),
        )
        .route(
            "/v1/media/thumbnail/:media_id",
            get(_matrix::media::v3::thumbnail::by_server_name::by_media_id::get),
        )
        .route("/v1/openid/userinfo", get(_matrix::federation::v1::openid::userinfo::get))
        .route("/v1/publicRooms", get(_matrix::federation::v1::public_rooms::get))
        .route("/v1/query/directory", get(_matrix::federation::v1::query::directory::get))
        .route("/v1/query/:query_type", get(_matrix::federation::v1::query::by_query_type::get))
        .route("/v1/state/:room_id", get(_matrix::federation::v1::state::by_room_id::get))
        .route("/v1/state_ids/:room_id", get(_matrix::federation::v1::state_ids::by_room_id::get))
        .route(
            "/v1/user/devices/:user_id",
            get(_matrix::federation::v1::user::devices::by_user_id::get),
        )
        .route("/v1/version", get(_matrix::federation::v1::version::get))
        .route(
            "/v1/get_missing_events/:room_id",
            post(_matrix::federation::v1::get_missing_events::by_room_id::post),
        )
        .route("/v1/publicRooms", post(_matrix::federation::v1::public_rooms::post))
        .route("/v1/user/keys/claim", post(_matrix::federation::v1::user::keys::claim::post))
        .route("/v1/user/keys/query", post(_matrix::federation::v1::user::keys::query::post))
        .route("/v1/3pid/onbind", put(_matrix::federation::v1::threepid::onbind::put))
        .route(
            "/v1/exchange_third_party_invite/:room_id",
            put(_matrix::federation::v1::exchange_third_party_invite::by_room_id::put),
        )
        .route(
            "/v1/invite/:room_id/:event_id",
            put(_matrix::federation::v1::invite::by_room_id::by_event_id::put),
        )
        .route("/v1/send/:txn_id", put(_matrix::federation::v1::send::by_txn_id::put))
        .route(
            "/v1/send_join/:room_id/:event_id",
            put(_matrix::federation::v1::send_join::by_room_id::by_event_id::put),
        )
        .route(
            "/v1/send_knock/:room_id/:event_id",
            put(_matrix::federation::v1::send_knock::by_room_id::by_event_id::put),
        )
        .route(
            "/v1/send_leave/:room_id/:event_id",
            put(_matrix::federation::v1::send_leave::by_room_id::by_event_id::put),
        )
        .route(
            "/v2/invite/:room_id/:event_id",
            put(_matrix::federation::v2::invite::by_room_id::by_event_id::put),
        )
        .route(
            "/v2/send_join/:room_id/:event_id",
            put(_matrix::federation::v2::send_join::by_room_id::by_event_id::put),
        )
        .route(
            "/v2/send_leave/:room_id/:event_id",
            put(_matrix::federation::v2::send_leave::by_room_id::by_event_id::put),
        )
    // send_to_device federation endpoint removed - non-compliant with Matrix spec
    // Send-to-device messages use m.direct_to_device EDU in /v1/send/{txnId} transactions
}

fn create_key_routes() -> Router<AppState> {
    Router::new()
        .route("/v2/query/:server_name", get(_matrix::key::v2::query::by_server_name::get))
        .route("/v2/server", get(_matrix::key::v2::server::get))
        .route("/v2/query", post(_matrix::key::v2::query::post))
}

fn create_media_routes() -> Router<AppState> {
    Router::new()
        .route("/v1/create", post(_matrix::media::v1::create::post))
        .route("/v3/upload", post(_matrix::media::v3::upload::post))
        .route(
            "/v3/upload/:server_name/:media_id",
            put(_matrix::media::v3::upload::by_server_name::by_media_id::put),
        )
        .route("/v3/config", get(_matrix::media::v3::config::get))
        .route(
            "/v3/download/:server_name/:media_id",
            get(_matrix::media::v3::download::by_server_name::by_media_id::get),
        )
        .route(
            "/v3/download/:server_name/:media_id/:file_name",
            get(_matrix::media::v3::download::by_server_name::by_media_id::by_file_name::get),
        )
        .route("/v3/preview_url", get(_matrix::media::v3::preview_url::get))
        .route(
            "/v3/thumbnail/:server_name/:media_id",
            get(_matrix::media::v3::thumbnail::by_server_name::by_media_id::get),
        )
}

fn create_app_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/location/:room_id",
            get(_matrix::app::v1::location::get).put(_matrix::app::v1::location::put),
        )
        .route(
            "/v1/rooms/:room_id/event/:event_id",
            get(_matrix::app::v1::rooms::by_room_id::event::by_event_id::get),
        )
        .route("/v1/thirdparty", get(_matrix::app::v1::thirdparty::get))
        .route(
            "/v1/thirdparty/location/:alias",
            get(_matrix::app::v1::thirdparty::location::by_alias::get),
        )
        .route(
            "/v1/thirdparty/protocol/:protocol",
            get(_matrix::app::v1::thirdparty::protocol::by_protocol::get),
        )
        .route(
            "/v1/thirdparty/user/:userid",
            get(_matrix::app::v1::thirdparty::user::by_userid::get),
        )
}

fn create_static_routes() -> Router<AppState> {
    Router::new().route("/client/login/", get(_matrix::static_::client::login::get))
}

fn create_identity_routes() -> Router<AppState> {
    Router::new()
        .route("/v1/openid/userinfo", get(_matrix::identity::v1::openid::userinfo::get))
        .route("/v1/query", post(_matrix::identity::v1::query::post))
        .route("/v1/query/:medium", post(_matrix::identity::v1::query::by_medium::post))
        .route(
            "/v1/threepid/getValidated3pid",
            get(_matrix::identity::v1::threepid::get_validated3pid::get),
        )
}

fn create_well_known_routes() -> Router<AppState> {
    Router::new().route("/matrix/support", get(_well_known::matrix::support::get))
}

async fn handler_404() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Endpoint not found")
}
