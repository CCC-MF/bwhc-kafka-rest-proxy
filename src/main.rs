use std::env;
use std::time::Duration;

use axum::{Extension, Json, Router};
use axum::body::Body;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{delete, post};
use bwhc_dto::MtbFile;
use rdkafka::ClientConfig;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone)]
struct KafkaConfig {
    dst_topic: String,
}

#[derive(Serialize, Deserialize)]
struct RecordKey {
    #[serde(rename = "pid")]
    patient_id: String,
    #[serde(rename = "eid", skip_serializing_if = "Option::is_none")]
    episode_id: Option<String>,
}

#[tokio::main]
async fn main() {
    let boostrap_servers = env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or("kafka:9094".into());
    let dst_topic = env::var("APP_KAFKA_TOPIC").unwrap_or("etl-processor_input".into());

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", boostrap_servers.as_str())
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");

    let app = Router::new()
        .route("/mtbfile", post(handle_post))
        .route("/mtbfile/:patient_id", delete(handle_delete))
        .layer(Extension(producer))
        .layer(Extension(KafkaConfig { dst_topic }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_delete(
    Path(patient_id): Path<String>,
    Extension(producer): Extension<FutureProducer>,
    Extension(kafka_config): Extension<KafkaConfig>,
) -> Response {
    let delete_mtb_file = MtbFile::new_with_consent_rejected(&patient_id);

    match send_mtb_file(producer, &kafka_config.dst_topic, delete_mtb_file).await {
        Ok(request_id) => success_response(&request_id),
        _ => error_response(),
    }
}

async fn handle_post(
    Extension(producer): Extension<FutureProducer>,
    Extension(kafka_config): Extension<KafkaConfig>,
    Json(mtb_file): Json<MtbFile>,
) -> Response {
    match send_mtb_file(producer, &kafka_config.dst_topic, mtb_file).await {
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
        episode_id: if mtb_file.episode.id.is_empty() {
            None
        } else {
            Some(mtb_file.episode.id.to_string())
        },
    };

    let record_headers = OwnedHeaders::default().insert(Header {
        key: "request_id",
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
