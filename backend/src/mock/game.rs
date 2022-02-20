use std::ops::DerefMut;
use std::sync::Arc;

use async_trait::async_trait;
use atomic_refcell::AtomicRefCell;

use crate::game_engine::game_def::Game;
use crate::game_engine::types::{
    Channel, ChannelId, DefinitionsRepository, GameApplicationContext, GameId, PlayerId, Response,
    ResponseMessage, ResponseSender, ResponseTextFormatter, SessionRepository,
};
use crate::services::sessions::InMemorySessionRepository;

pub struct MockContext {
    messages: AtomicRefCell<Vec<ResponseMessage>>,
    sessions: InMemorySessionRepository,
    game: Arc<Game>,
    channel: Arc<Channel>,
}

impl MockContext {
    pub async fn new() -> Self {
        MockContext {
            messages: Default::default(),
            sessions: Default::default(),
            game: Arc::new(create_test_game().await),
            channel: Arc::new(create_test_channel()),
        }
    }
}

impl MockContext {
    pub fn results(&self) -> Vec<ResponseMessage> {
        std::mem::take(self.messages.borrow_mut().deref_mut())
    }
}

#[async_trait]
impl ResponseSender for Arc<MockContext> {
    async fn respond(&self, response: Response) {
        self.messages.borrow_mut().push(response.message)
    }
}

#[async_trait]
impl DefinitionsRepository for Arc<MockContext> {
    async fn get_game_by_id(&self, _: GameId) -> Option<Arc<Game>> {
        Some(self.game.clone())
    }

    async fn get_channel_by_id(&self, channel_id: &ChannelId) -> Option<Arc<Channel>> {
        Some(self.channel.clone())
    }
}

impl GameApplicationContext for Arc<MockContext> {
    fn responder(&self) -> &dyn ResponseSender {
        self
    }
    fn sessions(&self) -> &dyn SessionRepository {
        &self.sessions
    }

    fn definitions(&self) -> &dyn DefinitionsRepository {
        self
    }
}

async fn create_test_game() -> Game {
    let file = std::env::current_dir()
        .unwrap()
        .join("src")
        .join("test_resources")
        .join("games")
        .join("game-1.json");
    Game::load(&file).await.unwrap()
}

fn create_test_channel() -> Channel {
    Channel {
        name: "test channel".to_string(),
        channel_id: "1".to_string(),
        token: "token".to_string(),
        game_id: Some(1),
    }
}
