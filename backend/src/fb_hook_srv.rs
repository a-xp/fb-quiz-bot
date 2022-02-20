use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server};
use serde_json::Value;
use urldecode::decode;

#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn process_text(&self, message: TextMessage);
    async fn process_other(&self, request: Request<Body>) -> Response<Body>;
}

#[derive(Default)]
struct NoOpHandler {}

#[async_trait]
impl MessageHandler for NoOpHandler {
    async fn process_text(&self, message: TextMessage) {
        log::info!("Processing {:?}", message)
    }

    async fn process_other(&self, _: Request<Body>) -> Response<Body> {
        Response::builder().status(404).body(Body::empty()).unwrap()
    }
}

pub struct FacebookHookServer {
    sync: bool,
    token: String,
    handler: Arc<dyn MessageHandler + Send + Sync>,
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct TextMessage {
    pub text: String,
    pub from: String,
    pub to: String,
}

impl Default for FacebookHookServer {
    fn default() -> Self {
        FacebookHookServer {
            sync: true,
            token: "TOKEN".to_string(),
            handler: Arc::new(NoOpHandler::default()),
        }
    }
}

impl FacebookHookServer {
    pub fn new_sync(
        token: &str,
        handler: Arc<dyn MessageHandler + Send + Sync>,
    ) -> FacebookHookServer {
        FacebookHookServer {
            sync: true,
            token: token.to_string(),
            handler,
        }
    }

    pub fn new_async(
        token: &str,
        handler: Arc<dyn MessageHandler + Send + Sync>,
    ) -> FacebookHookServer {
        FacebookHookServer {
            sync: false,
            token: token.to_string(),
            handler,
        }
    }

    pub async fn start(&'static self, port: u16) -> anyhow::Result<()> {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        log::info!("Server is listening on {}", port);
        Server::bind(&addr)
            .serve(make_service_fn(|_conn| async move {
                Ok::<_, Infallible>(service_fn(move |r| self.router(r)))
            }))
            .await?;
        anyhow::Ok(())
    }

    async fn router(&self, request: Request<Body>) -> Result<Response<Body>, Infallible> {
        log::info!("{} {}", request.method(), request.uri());
        let response = match (request.method().clone(), request.uri().path()) {
            (Method::GET, "/api/webhook") => self.handle_subscribe(request).await,
            (Method::POST, "/api/webhook") => self.handle_event(request).await,
            _ => self.handler.clone().process_other(request).await,
        };
        Ok(response)
    }

    async fn handle_subscribe(&self, request: Request<Body>) -> Response<Body> {
        let params = get_query(&request);
        if params.contains_key("hub.mode") && params.contains_key("hub.verify_token") {
            let verification = decode(params.get("hub.verify_token").unwrap().to_string());
            if verification == self.token {
                if *params.get("hub.mode").unwrap() == "subscribe" {
                    return Response::builder()
                        .status(200)
                        .body(Body::from(params.get("hub.challenge").unwrap().to_string()))
                        .unwrap();
                }
            }
        }
        Response::builder().status(403).body(Body::empty()).unwrap()
    }

    async fn handle_event(&self, mut request: Request<Body>) -> Response<Body> {
        let messages = parse_push_payload(request.body_mut()).await;
        if messages.len() > 0 {
            if self.sync {
                process_messages(messages, self.handler.clone()).await;
            } else {
                let handler = self.handler.clone();
                tokio::spawn(async move {
                    process_messages(messages, handler).await;
                });
            }
        }
        return Response::builder().status(200).body(Body::empty()).unwrap();
    }
}

async fn process_messages(
    mut messages: Vec<TextMessage>,
    handler: Arc<dyn MessageHandler + Send + Sync>,
) {
    while let Some(msg) = messages.pop() {
        handler.process_text(msg).await;
    }
}

fn get_query<T>(request: &Request<T>) -> HashMap<&str, &str> {
    let mut params = HashMap::new();
    querystring::querify(request.uri().query().unwrap_or_default())
        .iter()
        .for_each(|p| {
            params.insert((*p).0, (*p).1);
        });
    params
}

async fn parse_push_payload(data: &mut Body) -> Vec<TextMessage> {
    let buf = hyper::body::to_bytes(data).await.unwrap();
    let root: Value = serde_json::from_slice(buf.as_ref()).unwrap();
    log::debug!("New event: {}", root);
    let object = root["object"].as_str().unwrap_or_default();
    return if object == "page" || object == "instagram" {
        extract_messages(root)
    } else {
        Default::default()
    };
}

fn extract_messages(root: Value) -> Vec<TextMessage> {
    let mut result = Vec::new();
    if let Value::Array(entries) = &root["entry"] {
        entries.iter().for_each(|e| {
            if let Value::Array(messages) = &e["messaging"] {
                messages.iter().for_each(|msg| {
                    if msg["message"].is_object() {
                        if let (Some(from), Some(to), Some(text), None) = (
                            msg["sender"]["id"].as_str(),
                            msg["recipient"]["id"].as_str(),
                            msg["message"]["text"].as_str(),
                            msg["message"]["is_echo"].as_bool(),
                        ) {
                            result.push(TextMessage {
                                text: text.to_string(),
                                from: from.to_string(),
                                to: to.to_string(),
                            })
                        }
                    }
                })
            }
        })
    }
    result
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use atomic_refcell::AtomicRefCell;
    use hyper::{Body, Method, Request, Response, Uri};
    use serde_json::Value;

    use crate::fb_hook_srv::{extract_messages, FacebookHookServer, MessageHandler, TextMessage};

    async fn body_to_str(body: &mut Body) -> String {
        String::from_utf8(hyper::body::to_bytes(body).await.unwrap().to_vec()).unwrap()
    }

    async fn get_test_message(subpath: &str) -> String {
        let file = std::env::current_dir()
            .unwrap()
            .join("src")
            .join("test_resources")
            .join("msg")
            .join(subpath);
        tokio::fs::read_to_string(file).await.unwrap()
    }

    async fn get_test_msg_obj(subpath: &str) -> Value {
        let buf = get_test_message(subpath).await;
        serde_json::from_str(buf.as_str()).unwrap()
    }

    async fn create_message_event_push() -> Request<Body> {
        Request::builder()
            .uri(Uri::from_static("/api/webhook"))
            .method(Method::POST)
            .body(Body::from(get_test_message("new_message.json").await))
            .unwrap()
    }

    #[tokio::test]
    async fn subscribe_request_should_receive_challenge() {
        let server = FacebookHookServer::default();
        let request = Request::builder()
            .uri(Uri::from_static(
                "/api/webhook?hub.mode=subscribe&hub.challenge=1492553178&hub.verify_token=TOKEN",
            ))
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();

        let mut response = server.handle_subscribe(request).await;
        let response_str = body_to_str(response.body_mut()).await;

        assert_eq!(200, response.status().as_u16());
        assert_eq!("1492553178", response_str.as_str());
    }

    #[tokio::test]
    async fn event_push_should_receive_200() {
        let server = FacebookHookServer::default();
        let response = server.handle_event(create_message_event_push().await).await;
        assert_eq!(200, response.status().as_u16());
    }

    #[tokio::test]
    async fn user_message_is_processed() {
        let engine = Arc::new(NoOpGameEngine::default());
        let server = FacebookHookServer::new_sync("TOKEN", engine.clone());
        server.handle_event(create_message_event_push().await).await;

        assert_eq!(
            vec![TextMessage {
                text: "hello".to_string(),
                from: "4339620206152955".to_string(),
                to: "106197145160389".to_string(),
            }],
            engine.get_hist()
        )
    }

    #[tokio::test]
    async fn should_ignore_echo_messages() {
        let msg = get_test_msg_obj("echo1.json").await;
        let result = extract_messages(msg);
        assert!(result.is_empty())
    }

    #[tokio::test]
    async fn should_extract_normal_messages() {
        let msg = get_test_msg_obj("new_message.json").await;
        let result = extract_messages(msg);
        assert_eq!(
            vec![TextMessage {
                text: "hello".to_string(),
                from: "4339620206152955".to_string(),
                to: "106197145160389".to_string(),
            }],
            result
        )
    }

    #[tokio::test]
    async fn should_extract_reply_messages() {
        let msg = get_test_msg_obj("reply1.json").await;
        let result = extract_messages(msg);
        assert_eq!(
            vec![TextMessage {
                text: "А где ?".to_string(),
                from: "4826337357487893".to_string(),
                to: "17841451802358813".to_string(),
            }],
            result
        )
    }

    #[derive(Default)]
    pub struct NoOpGameEngine {
        hist: AtomicRefCell<Vec<TextMessage>>,
    }

    impl NoOpGameEngine {
        pub fn get_hist(&self) -> Vec<TextMessage> {
            let result = self.hist.borrow_mut().to_vec();
            self.hist.borrow_mut().clear();
            result
        }
    }

    #[async_trait]
    impl MessageHandler for NoOpGameEngine {
        async fn process_text(&self, message: TextMessage) {
            println!("Processing {:?}", message);
            self.hist.borrow_mut().push(message);
        }

        async fn process_other(&self, _: Request<Body>) -> Response<Body> {
            Response::builder().status(404).body(Body::empty()).unwrap()
        }
    }
}
