use std::io::Write;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::{Builder, Runtime};

use quiz::game_engine::engine::GameEngine;
use quiz::game_engine::game_def::Game;
use quiz::game_engine::types::ResponseMessage::*;
use quiz::game_engine::types::{
    Channel, ChannelId, DefinitionsRepository, GameApplicationContext, GameId, PlayerId,
    PlayerMessage, ResponseMessage, ResponseSender, ResponseTextFormatter, SessionRepository,
};
use quiz::services::sessions::InMemorySessionRepository;

#[tokio::main]
async fn main() {
    let app = Box::leak(Box::new(ConsoleApp::new().await));
    let mut line = String::new();
    loop {
        line.clear();
        std::io::stdin().read_line(&mut line).expect("Failed");
        if line == ":q" {
            break;
        }
        app.process(line.as_str()).await;
    }
}

struct ConsoleApp {
    engine: GameEngine,
    game: Game,
    repo: InMemorySessionRepository,
    channel: Channel,
}

impl ConsoleApp {
    async fn new() -> ConsoleApp {
        ConsoleApp {
            engine: Default::default(),
            game: create_test_game().await,
            repo: Default::default(),
            channel: Channel {
                name: "console".to_string(),
                channel_id: "1".to_string(),
                token: "".to_string(),
                game_id: Some(1),
            },
        }
    }

    pub async fn process(&'static self, text: &str) {
        self.engine
            .process_message(
                PlayerMessage {
                    player_id: PlayerId {
                        id: "1".to_string(),
                        channel_id: "channel_id".to_string(),
                    },
                    text: text.to_string(),
                },
                self,
            )
            .await;
    }
}

async fn create_test_game() -> Game {
    let file = std::env::current_dir()
        .unwrap()
        .join("deploy")
        .join("data")
        .join("game1.json");
    Game::load(&file).await.unwrap()
}

#[async_trait]
impl ResponseSender for ConsoleApp {
    async fn respond(
        &self,
        to: &PlayerId,
        response: ResponseMessage,
        format: &dyn ResponseTextFormatter,
    ) {
        println!(format.format(response))
    }
}

#[async_trait]
impl DefinitionsRepository for ConsoleApp {
    async fn get_game_by_id(&self, game_id: GameId) -> Option<&Game> {
        Some(&self.game)
    }

    async fn get_channel_by_id(&self, channel_id: &ChannelId) -> Option<&Channel> {
        Some(&self.channel)
    }
}

impl GameApplicationContext for ConsoleApp {
    fn responder(&self) -> &dyn ResponseSender {
        self
    }

    fn sessions(&self) -> &dyn SessionRepository {
        &self.repo
    }

    fn definitions(&self) -> &dyn DefinitionsRepository {
        self
    }
}
