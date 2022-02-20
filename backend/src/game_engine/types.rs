use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::game_engine::game_def::{Game, QuestionId, TopicId};
use crate::game_engine::types::ResponseMessage::AnswerQuestion;
use crate::game_engine::types::SessionState::{Answering, New};

pub type GameId = u32;
pub type ChannelId = String;

#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Channel {
    pub name: String,
    pub channel_id: ChannelId,
    pub token: String,
    pub game_id: Option<GameId>,
}

#[derive(PartialEq, Debug, Clone, Hash, Eq, Default)]
pub struct PlayerId {
    pub channel_id: String,
    pub id: String,
}

#[derive(PartialEq, Debug, Clone, Default)]
pub struct GameSession {
    pub player_id: PlayerId,
    pub game_id: GameId,
    pub state: SessionState,
    pub results: Vec<TopicResult>,
    pub score: u16,
}

impl GameSession {
    pub fn new(player_id: &PlayerId, game_id: GameId) -> GameSession {
        GameSession {
            player_id: player_id.clone(),
            game_id,
            state: SessionState::New,
            results: Default::default(),
            score: 0,
        }
    }

    pub fn record(&mut self, topic_id: TopicId, score: u8) {
        self.score += score as u16;
        self.results.push(TopicResult { topic_id, score })
    }

    pub fn has_played(&self, topic_id: TopicId) -> bool {
        self.results.iter().any(|r| r.topic_id == topic_id)
    }
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct TopicResult {
    pub topic_id: u8,
    pub score: u8,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct AnswerAttempt {
    pub question_id: QuestionId,
    pub attempt: u8,
}

#[derive(PartialEq, Debug, Clone)]
pub enum SessionState {
    New,
    Deciding,
    Answering(AnswerAttempt),
    ChoosingTopic,
    Terminated,
    Complete,
}

impl SessionState {
    pub fn answering(question_id: QuestionId, attempt: u8) -> SessionState {
        Answering(AnswerAttempt {
            question_id,
            attempt,
        })
    }
}

impl Default for SessionState {
    fn default() -> Self {
        New
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct PlayerPersonalInfo {
    pub id: PlayerId,
    pub name: String,
    pub lastname: String,
}

#[derive(PartialEq, Debug, Clone)]
pub struct PlayerMessage {
    pub player_id: PlayerId,
    pub text: String,
}

#[derive(PartialEq, Debug, Clone)]
pub enum ResponseMessage {
    Greeting(String),
    Rephrase,
    Rules(Vec<String>),
    AnswerQuestion(String),
    PleaseRetry,
    PleaseRetryLimits(u8),
    Incorrect,
    Correct(u16),
    GameComplete(u16),
    ChooseNextTopic,
    AlreadyAnswered,
    Quit,
}

impl ResponseMessage {
    pub fn greeting(game_name: &str) -> ResponseMessage {
        ResponseMessage::Greeting(game_name.to_string())
    }

    pub fn rules(topics: Vec<&str>) -> ResponseMessage {
        ResponseMessage::Rules(topics.iter().map(|t| t.to_string()).collect())
    }

    pub fn answer_question(question: &str) -> ResponseMessage {
        AnswerQuestion(question.to_string())
    }
}

pub trait GameApplicationContext: Send + Sync {
    fn responder(&self) -> &dyn ResponseSender;
    fn sessions(&self) -> &dyn SessionRepository;
    fn definitions(&self) -> &dyn DefinitionsRepository;
}

pub trait ResponseTextFormatter: Send + Sync {
    fn format(&self, message: ResponseMessage) -> String;
}

pub struct Response {
    pub to: PlayerId,
    pub channel: Arc<Channel>,
    pub message: ResponseMessage,
    pub format: Arc<dyn ResponseTextFormatter>,
}

#[async_trait]
pub trait ResponseSender: Send + Sync {
    async fn respond(&self, response: Response);
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn get_by_id(&self, game_id: u32, player_id: &PlayerId) -> Option<GameSession>;
    async fn store(&self, session: &GameSession);
}

#[async_trait]
pub trait PlayerDetailsProvider: Send + Sync {
    async fn fetch_details(&self, id: &PlayerId) -> Option<PlayerPersonalInfo>;
}

#[async_trait]
pub trait DefinitionsRepository: Send + Sync {
    async fn get_game_by_id(&self, game_id: GameId) -> Option<Arc<Game>>;
    async fn get_channel_by_id(&self, channel_id: &ChannelId) -> Option<Arc<Channel>>;
}
