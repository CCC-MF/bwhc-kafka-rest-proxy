use std::time::Duration;

use bwhc_dto::MtbFile;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use uuid::Uuid;

use crate::RecordKey;

#[derive(Clone)]
pub struct MtbFileSender {
    topic: String,
    producer: FutureProducer,
}

impl MtbFileSender {
    pub fn new(topic: &str, bootstrap_server: &str) -> Self {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", bootstrap_server)
            .set("message.timeout.ms", "5000")
            .create()
            .expect("Producer creation error");

        Self {
            topic: topic.to_string(),
            producer,
        }
    }

    pub async fn send(&self, mtb_file: MtbFile) -> Result<String, ()> {
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
                self.producer
                    .send(
                        FutureRecord::to(&self.topic)
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
}
