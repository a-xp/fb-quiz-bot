use std::sync::Arc;

use async_trait::async_trait;
use hyper::client::HttpConnector;
use hyper::header::CONTENT_TYPE;
use hyper::{Body, Client, Method, Request};
use hyper_rustls::HttpsConnector;
use serde::{Deserialize, Serialize};

use crate::game_engine::types::{
    PlayerId, Response, ResponseMessage, ResponseSender, ResponseTextFormatter,
};

pub struct FbResponseService {
    client: Arc<Client<HttpsConnector<HttpConnector>, Body>>,
}

impl FbResponseService {
    pub fn new() -> Self {
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_only()
            .enable_http1()
            .build();
        FbResponseService {
            client: Arc::new(Client::builder().build(https)),
        }
    }

    async fn send_message(&self, token: &str, json: String) {
        let request = Request::builder()
            .method(Method::POST)
            .uri(format!(
                "https://graph.facebook.com/v12.0/me/messages?access_token={}",
                token
            ))
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(json))
            .unwrap();
        if let Err(err) = self.client.request(request).await {
            log::error!("Failed to respond: {}", err)
        }
    }
}

fn create_text_response(id: &str, text: &str) -> String {
    let msg = MessageWrapper {
        messaging_type: "RESPONSE".to_string(),
        recipient: MessageRecipient { id: id.to_string() },
        message: MessageContent {
            text: Some(text.to_string()),
            attachment: None,
        },
    };
    serde_json::to_string(&msg).unwrap()
}

#[async_trait]
impl ResponseSender for FbResponseService {
    async fn respond(&self, response: Response) {
        let text = response.format.format(response.message);
        let json = create_text_response(response.to.id.as_str(), text.as_str());
        self.send_message(response.channel.token.as_str(), json)
            .await;
    }
}

#[derive(Serialize, Deserialize)]
struct MessageWrapper {
    pub messaging_type: String,
    pub recipient: MessageRecipient,
    pub message: MessageContent,
}

#[derive(Serialize, Deserialize)]
struct MessageRecipient {
    pub id: String,
}

#[derive(Serialize, Deserialize)]
struct MessageContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment: Option<Attachment>,
}

#[derive(Serialize, Deserialize)]
struct Attachment {
    #[serde(rename = "type")]
    pub kind: String,
    pub payload: AttachmentPayload,
}

#[derive(Serialize, Deserialize)]
struct AttachmentPayload {
    pub is_reusable: bool,
    pub url: String,
}
