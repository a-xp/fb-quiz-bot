use std::path::PathBuf;

use async_trait::async_trait;
use hyper::{Body, Request, Response};
use quiz::fb_hook_srv::{FacebookHookServer, MessageHandler, TextMessage};
use quiz::game_engine::engine::GameEngine;
use quiz::game_engine::types::{
    DefinitionsRepository, GameApplicationContext, PlayerId, PlayerMessage, ResponseSender,
    SessionRepository,
};
use quiz::services::definitions::FileRepository;
use quiz::services::response::FbResponseService;
use quiz::services::sessions::InMemorySessionRepository;
use std::sync::Arc;

const DATA_DIR: &str = "./deploy/data";

#[tokio::main]
async fn main() {
    env_logger::init();
    let ctx = create_context().await;
    let token = get_confirmation_token();
    log::info!("Using token {}", token);
    let server = Box::leak(Box::new(FacebookHookServer::new_async(
        token.as_str(),
        HandlerAdapter::new(ctx),
    )));
    if let Err(err) = server.start(get_port()).await {
        log::error!("Server failed to start {}", err)
    }
}

struct HandlerAdapter {
    engine: GameEngine,
    ctx: &'static dyn GameApplicationContext,
}

impl HandlerAdapter {
    pub fn new(ctx: &'static dyn GameApplicationContext) -> Arc<HandlerAdapter> {
        Arc::new(HandlerAdapter {
            engine: Default::default(),
            ctx,
        })
    }
}

#[async_trait]
impl MessageHandler for HandlerAdapter {
    async fn process_text(&self, message: TextMessage) {
        self.engine
            .process_message(
                PlayerMessage {
                    player_id: PlayerId {
                        channel_id: message.to,
                        id: message.from,
                    },
                    text: message.text,
                },
                self.ctx,
            )
            .await;
    }

    async fn process_other(&self, request: Request<Body>) -> Response<Body> {
        Response::builder().status(404).body(Body::empty()).unwrap()
    }
}

struct WebApplicationContext {
    responder: FbResponseService,
    sessions: InMemorySessionRepository,
    definitions: FileRepository,
}

impl GameApplicationContext for WebApplicationContext {
    fn responder(&self) -> &dyn ResponseSender {
        &self.responder
    }

    fn sessions(&self) -> &dyn SessionRepository {
        &self.sessions
    }

    fn definitions(&self) -> &dyn DefinitionsRepository {
        &self.definitions
    }
}

async fn create_context() -> &'static WebApplicationContext {
    let path = std::env::current_dir()
        .unwrap()
        .join(get_data_dir().as_str());
    Box::leak(Box::new(WebApplicationContext {
        responder: FbResponseService::new(),
        sessions: InMemorySessionRepository::default(),
        definitions: FileRepository::load(&path)
            .await
            .expect("Failed to load definitions"),
    }))
}

fn get_port() -> u16 {
    std::env::var("PORT")
        .unwrap_or("3021".to_string())
        .parse()
        .expect("Invalid port")
}

fn get_confirmation_token() -> String {
    std::env::var("TOKEN").unwrap_or("MY_TEST_TOKEN".to_string())
}

fn get_data_dir() -> String {
    std::env::var("DATA_DIR").unwrap_or(DATA_DIR.to_string())
}
