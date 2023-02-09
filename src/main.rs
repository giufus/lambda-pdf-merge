use std::fmt::{Debug, Display};
use std::io::ErrorKind;
use std::ops::Deref;
use base64::{alphabet, DecodeError, Engine as _, engine::{self, general_purpose}};
use lambda_http::{Body, Error, Request, RequestExt, Response, run, service_fn};
use lopdf::Document;
use serde::{Deserialize, Serialize};
use tracing_subscriber::fmt::format;

mod pdf;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MergeRequest {
    files: Vec<String>
}

impl MergeRequest {

    pub fn new(files: Vec<String>) -> Self {
        Self { files }
    }
}

pub fn str_to_vec(input_b64: &str) -> Result<Vec<u8>, DecodeError> {
    use base64::Engine;
    general_purpose::STANDARD.decode(input_b64)
}

/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
async fn function_handler(event: Request) -> Result<Response<Body>, Error> {
    // Extract some useful information from the request
    let merge = event.payload::<MergeRequest>()?;

    if merge.is_none(){
        let resp = Response::builder()
            .status(400)
            .header("content-type", "application/json")
            .body("Bad payload".into())
            .map_err(Box::new)?;
        return Ok(resp);
    }

    let mut doc_bytes = Vec::new();
    let doc: Result<Document, ErrorKind>;


    // base 64 strings into vector of Document
    let docs = merge.unwrap().files.iter()
        .map(|s| str_to_vec(s.as_str()))
        .filter(|v| v.is_ok())
        .map(|s| Document::load_mem(s.unwrap().as_slice()).unwrap())
        .collect::<Vec<Document>>();

    doc = pdf::merge(docs);

    if doc.is_ok() {
        doc.unwrap().save_to(&mut doc_bytes);
        // Return something that implements IntoResponse.
        // It will be serialized to the right response event automatically by the runtime
        let resp = Response::builder()
            .status(200)
            .header("content-type", "application/pdf")
            //.body("Hello AWS Lambda HTTP request".into())
            .body(Body::from(doc_bytes))
            .map_err(Box::new)?;
        return Ok(resp);
    } else {
        let resp = Response::builder()
            .status(400)
            .header("content-type", "application/json")
            .body(doc.err().unwrap().to_string().into())
            .map_err(Box::new)?;
        return Ok(resp);
    }

}

#[tokio::main]
async fn main() -> Result<(), Error> {

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        // disable printing the name of the module in every log line.
        .with_target(false)
        // disabling time is handy because CloudWatch will add the ingestion time.
        .without_time()
        .init();

    run(service_fn(function_handler)).await
}
