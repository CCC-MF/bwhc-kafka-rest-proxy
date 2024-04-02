use axum::{Extension, Json, Router};
use axum::body::Body;
use axum::extract::Path;
use axum::http::{Request, StatusCode};
use axum::http::header::AUTHORIZATION;
use axum::middleware::{from_fn, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, post};
use bwhc_dto::MtbFile;
use clap::Parser;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
#[cfg(debug_assertions)]
use tower_http::trace::TraceLayer;

use crate::AppResponse::{Accepted, InternalServerError, Unauthorized};
use crate::cli::Cli;
use crate::sender::MtbFileSender;

mod auth;
mod cli;
mod sender;

#[derive(Serialize, Deserialize)]
struct RecordKey {
    #[serde(rename = "pid")]
    patient_id: String,
}

enum AppResponse<'a> {
    Accepted(&'a str),
    Unauthorized,
    InternalServerError,
}

impl IntoResponse for AppResponse<'_> {
    fn into_response(self) -> Response {
        match self {
            Accepted(request_id) => Response::builder()
                .status(StatusCode::ACCEPTED)
                .header("X-Request-Id", request_id),
            Unauthorized => Response::builder().status(StatusCode::UNAUTHORIZED),
            InternalServerError => Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR),
        }
        .body(Body::empty())
        .expect("response built")
    }
}

lazy_static! {
    static ref CONFIG: Cli = Cli::parse();
}

#[tokio::main]
async fn main() {
    #[cfg(debug_assertions)]
    {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    }

    let sender = MtbFileSender::new(&CONFIG.topic, &CONFIG.bootstrap_server);

    let app = Router::new()
        .route("/mtbfile", post(handle_post))
        .route("/mtbfile/:patient_id", delete(handle_delete))
        .layer(Extension(sender))
        .layer(from_fn(check_basic_auth));

    #[cfg(debug_assertions)]
    let app = app.layer(TraceLayer::new_for_http());

    match tokio::net::TcpListener::bind(&CONFIG.listen).await {
        Ok(listener) => {
            log::info!("Starting application listening on '{}'", CONFIG.listen);
            if let Err(err) = axum::serve(listener, app).await {
                log::error!("Error starting application: {}", err)
            }
        }
        Err(err) => log::error!("Error listening on '{}': {}", CONFIG.listen, err),
    };
}

async fn check_basic_auth(request: Request<Body>, next: Next) -> Response {
    if let Some(Ok(auth_header)) = request.headers().get(AUTHORIZATION).map(|x| x.to_str()) {
        if auth::check_basic_auth(auth_header, &CONFIG.token) {
            return next.run(request).await;
        }
    }
    Unauthorized.into_response()
}

async fn handle_delete(
    Path(patient_id): Path<String>,
    Extension(sender): Extension<MtbFileSender>,
) -> Response {
    let delete_mtb_file = MtbFile::new_with_consent_rejected(&patient_id);

    match sender.send(delete_mtb_file).await {
        Ok(request_id) => Accepted(&request_id).into_response(),
        _ => InternalServerError.into_response(),
    }
}

async fn handle_post(
    Extension(sender): Extension<MtbFileSender>,
    Json(mtb_file): Json<MtbFile>,
) -> Response {
    match sender.send(mtb_file).await {
        Ok(request_id) => Accepted(&request_id).into_response(),
        _ => InternalServerError.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use uuid::Uuid;

    use crate::AppResponse::{Accepted, InternalServerError};

    #[test]
    fn should_return_success_response() {
        let response = Accepted(&Uuid::new_v4().to_string()).into_response();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[test]
    fn should_return_error_response() {
        let response = InternalServerError.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.headers().contains_key("x-request-id"), false);
    }
}
