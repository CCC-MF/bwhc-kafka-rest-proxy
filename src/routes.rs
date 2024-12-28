use crate::sender::MtbFileSender;
use crate::AppResponse::{Accepted, InternalServerError};
use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use bwhc_dto::MtbFile;

pub async fn handle_delete(
    Path(patient_id): Path<String>,
    Extension(sender): Extension<MtbFileSender>,
) -> Response {
    let delete_mtb_file = MtbFile::new_with_consent_rejected(&patient_id);

    match sender.send(delete_mtb_file).await {
        Ok(request_id) => Accepted(&request_id).into_response(),
        _ => InternalServerError.into_response(),
    }
}

pub async fn handle_post(
    Extension(sender): Extension<MtbFileSender>,
    Json(mtb_file): Json<MtbFile>,
) -> Response {
    match sender.send(mtb_file).await {
        Ok(request_id) => Accepted(&request_id).into_response(),
        _ => InternalServerError.into_response(),
    }
}
