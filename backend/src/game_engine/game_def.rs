use crate::game_engine::types::{GameId, ResponseMessage, ResponseTextFormatter};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type TopicId = u8;
#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub struct QuestionId(u8, u8);

impl QuestionId {
    pub fn topic(&self) -> u8 {
        self.0
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Game {
    pub id: GameId,
    pub name: String,
    #[serde(default)]
    generic_answers: GenericAnswers,
    topics: Vec<Topic>,
    pub max_attempt: Option<u8>,
    #[serde(default)]
    responses: ResponseTemplates,
}

impl Game {
    pub fn is_yes(&self, text: &str) -> bool {
        self.generic_answers.yes.iter().any(|s| s == text)
    }

    pub fn is_no(&self, text: &str) -> bool {
        self.generic_answers.no.iter().any(|s| s == text)
    }

    pub fn is_stop(&self, text: &str) -> bool {
        self.generic_answers.stop.iter().any(|s| s == text)
    }

    pub fn find_topic(&self, text: &str) -> Option<TopicId> {
        self.topics
            .iter()
            .position(|t| t.key.contains(text))
            .map(|id| id as TopicId)
    }

    pub fn get_question_from_topic(&self, id: TopicId) -> QuestionId {
        let num = self.topics[id as usize].questions.len() as u8;
        let r: u8 = rand::random();
        QuestionId(id, r % num)
    }

    pub fn get_question_text(&self, question_id: QuestionId) -> String {
        self.topics[question_id.0 as usize].questions[question_id.1 as usize]
            .text
            .clone()
    }

    pub async fn load(path: &PathBuf) -> anyhow::Result<Game> {
        let content = tokio::fs::read(path).await?;
        let game: Game = serde_json::from_slice(content.as_slice())?;
        anyhow::Ok(game)
    }

    pub fn is_correct_answer(&self, question_id: QuestionId, text: &str) -> bool {
        self.topics[question_id.0 as usize].questions[question_id.1 as usize]
            .answers
            .iter()
            .any(|a| a == text)
    }

    pub fn get_bonus(&self, topic_id: TopicId) -> u8 {
        self.topics[topic_id as usize].bonus
    }

    pub fn is_complete(&self, num_answers: u8) -> bool {
        self.topics.len() as u8 == num_answers
    }

    pub fn topic_keys(&self) -> Vec<String> {
        self.topics.iter().map(|t| t.key.clone()).collect()
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Topic {
    name: String,
    key: String,
    questions: Vec<Question>,
    bonus: u8,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Question {
    text: String,
    answers: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GenericAnswers {
    pub yes: Vec<String>,
    pub no: Vec<String>,
    pub stop: Vec<String>,
}

impl Default for GenericAnswers {
    fn default() -> Self {
        GenericAnswers {
            yes: vec!["yes".to_string(), "да".to_string()],
            no: vec!["no".to_string(), "нет".to_string()],
            stop: vec!["stop".to_string(), "стоп".to_string()],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResponseTemplates {
    greeting: String,
    rephrase: String,
    rules: String,
    answer_question: String,
    please_retry: String,
    please_retry_limits: String,
    incorrect: String,
    correct: String,
    game_complete: String,
    choose_next_topic: String,
    already_answered: String,
    quit: String,
}

impl ResponseTextFormatter for Game {
    fn format(&self, response: ResponseMessage) -> String {
        match response {
            ResponseMessage::Greeting(game_name) => {
                self.responses.greeting.replace("#NAME", game_name.as_str())
            }
            ResponseMessage::Rephrase => self.responses.rephrase.clone(),
            ResponseMessage::Rules(topics) => self
                .responses
                .rules
                .replace("#TOPICS", topics.join(", ").as_str()),
            ResponseMessage::AnswerQuestion(text) => self
                .responses
                .answer_question
                .replace("#QUESTION", text.as_str()),
            ResponseMessage::PleaseRetry => self.responses.please_retry.clone(),
            ResponseMessage::PleaseRetryLimits(num) => self
                .responses
                .please_retry_limits
                .replace("#LEFT", num.to_string().as_str()),
            ResponseMessage::Incorrect => self.responses.incorrect.clone(),
            ResponseMessage::Correct(score) => self
                .responses
                .correct
                .replace("#SCORE", score.to_string().as_str()),
            ResponseMessage::GameComplete(score) => self
                .responses
                .game_complete
                .replace("#SCORE", score.to_string().as_str()),
            ResponseMessage::ChooseNextTopic => self.responses.choose_next_topic.clone(),
            ResponseMessage::AlreadyAnswered => self.responses.already_answered.clone(),
            ResponseMessage::Quit => self.responses.quit.clone(),
        }
    }
}

impl Default for ResponseTemplates {
    fn default() -> Self {
        ResponseTemplates {
            greeting: "Hello! Today we play #NAME. Want to join?".to_string(),
            rephrase: "I don't understand".to_string(),
            rules: "Choose a topic from: #TOPICS. Answer a question. Get your score when all topics are comnplete".to_string(),
            answer_question: "Next question: #QUESTION".to_string(),
            please_retry: "That is incorrect. Try again".to_string(),
            please_retry_limits: "That is incorrect. Try again. #LEFT attempts left".to_string(),
            incorrect: "That is incorrect".to_string(),
            correct: "That is correct. Your score: #SCORE".to_string(),
            game_complete: "Game is complete. Your score: #SCORE".to_string(),
            choose_next_topic: "Choose the next topic".to_string(),
            already_answered: "You already answered this topic".to_string(),
            quit: "Ok... Goodbye!".to_string()
        }
    }
}
