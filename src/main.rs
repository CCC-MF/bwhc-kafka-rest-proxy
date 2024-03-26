use std::time::Duration;

use axum::{Extension, Json, Router};
use axum::body::Body;
use axum::extract::Path;
use axum::http::{Request, StatusCode};
use axum::http::header::AUTHORIZATION;
use axum::middleware::{from_fn, Next};
use axum::response::Response;
use axum::routing::{delete, post};
use bwhc_dto::MtbFile;
use clap::Parser;
use lazy_static::lazy_static;
use rdkafka::ClientConfig;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
#[cfg(debug_assertions)]
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::cli::Cli;

mod auth;
mod cli;

#[derive(Serialize, Deserialize)]
struct RecordKey {
    #[serde(rename = "pid")]
    patient_id: String,
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

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", CONFIG.bootstrap_server.as_str())
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");

    let app = Router::new()
        .route("/mtbfile", post(handle_post))
        .route("/mtbfile/:patient_id", delete(handle_delete))
        .layer(Extension(producer))
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
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(Body::empty())
        .expect("response built")
}

async fn handle_delete(
    Path(patient_id): Path<String>,
    Extension(producer): Extension<FutureProducer>,
) -> Response {
    let delete_mtb_file = MtbFile::new_with_consent_rejected(&patient_id);

    match send_mtb_file(producer, &CONFIG.topic, delete_mtb_file).await {
        Ok(request_id) => success_response(&request_id),
        _ => error_response(),
    }
}

async fn handle_post(
    Extension(producer): Extension<FutureProducer>,
    Json(mtb_file): Json<MtbFile>,
) -> Response {
    match send_mtb_file(producer, &CONFIG.topic, mtb_file).await {
        Ok(request_id) => success_response(&request_id),
        _ => error_response(),
    }
}

async fn send_mtb_file(
    producer: FutureProducer,
    topic: &str,
    mtb_file: MtbFile,
) -> Result<String, ()> {
    let request_id = Uuid::new_v4();

    let record_key = RecordKey {
        patient_id: mtb_file.patient.id.to_string(),
    };

    let record_headers = OwnedHeaders::default().insert(Header {
        key: "requestId",
        value: Some(&request_id.to_string()),
    });

    let record_key = serde_json::to_string(&record_key).map_err(|_| ())?;

    match serde_json::to_string(&mtb_file) {
        Ok(json) => {
            producer
                .send(
                    FutureRecord::to(topic)
                        .key(&record_key)
                        .headers(record_headers)
                        .payload(&json),
                    Duration::from_secs(1),
                )
                .await
                .map_err(|_| ())
                .map(|_| ())?;
            Ok(request_id.to_string())
        }
        Err(_) => Err(()),
    }
}

fn success_response(request_id: &str) -> Response {
    Response::builder()
        .status(StatusCode::ACCEPTED)
        .header("X-Request-Id", request_id)
        .body(Body::empty())
        .expect("response built")
}

fn error_response() -> Response {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::empty())
        .expect("response built")
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use uuid::Uuid;

    use crate::{error_response, success_response};

    #[test]
    fn should_return_success_response() {
        let response = success_response(&Uuid::new_v4().to_string());
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[test]
    fn should_return_error_response() {
        let response = error_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.headers().contains_key("x-request-id"), false);
    }

}