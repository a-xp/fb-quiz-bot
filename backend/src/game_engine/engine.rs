use regex::internal::Input;

use crate::game_engine::game_def::{Game, QuestionId};
use crate::game_engine::types::ResponseMessage::{
    AlreadyAnswered, ChooseNextTopic, Correct, GameComplete, Greeting, Incorrect, PleaseRetry,
    PleaseRetryLimits, Quit, Rephrase, Rules,
};
use crate::game_engine::types::SessionState::{
    Answering, ChoosingTopic, Complete, Deciding, New, Terminated,
};
use crate::game_engine::types::*;
use crate::text_util::answer_to_standard;
use std::sync::Arc;

#[derive(Default)]
pub struct GameEngine {}

#[derive(Clone)]
struct MessageContext {
    player_id: PlayerId,
    message: String,
    game: Arc<Game>,
    channel: Arc<Channel>,
    session: GameSession,
    app_context: &'static dyn GameApplicationContext,
}

impl MessageContext {
    pub fn new(
        app_context: &'static dyn GameApplicationContext,
        game: Arc<Game>,
        channel: Arc<Channel>,
        message: PlayerMessage,
    ) -> MessageContext {
        let game_id = game.id;
        MessageContext {
            message: answer_to_standard(message.text.as_str()),
            player_id: message.player_id.clone(),
            game,
            channel,
            session: GameSession::new(&message.player_id, game_id),
            app_context,
        }
    }

    async fn respond(&self, response: ResponseMessage) {
        self.app_context
            .responder()
            .respond(Response {
                to: self.player_id.clone(),
                channel: self.channel.clone(),
                message: response,
                format: self.game.clone(),
            })
            .await
    }

    async fn restore_session(&mut self) {
        if let Some(session) = self
            .app_context
            .sessions()
            .get_by_id(self.game.id, &self.player_id)
            .await
        {
            self.session = session;
        }
    }

    async fn store_progress(&self) {
        self.app_context.sessions().store(&self.session).await;
    }

    async fn greet(&mut self) {
        self.respond(Greeting(self.game.name.clone())).await;
        self.session.state = Deciding;
    }

    async fn has_user_agreed_to_start(&mut self) {
        if self.game.is_yes(self.message.as_str()) {
            self.respond(Rules(self.game.topic_keys())).await;
            self.session.state = SessionState::ChoosingTopic;
        } else if self.game.is_no(self.message.as_str()) {
            self.respond(Quit).await;
            self.session.state = SessionState::Terminated;
        } else {
            self.respond(Rephrase).await;
        }
    }

    async fn choose_topic(&mut self) {
        if let Some(topic_id) = self.game.find_topic(self.message.as_str()) {
            if self.session.has_played(topic_id) {
                self.respond(AlreadyAnswered).await;
            } else {
                let question_id = self.game.get_question_from_topic(topic_id);
                self.session.state = SessionState::answering(question_id, 0);
                self.respond(ResponseMessage::AnswerQuestion(
                    self.game.get_question_text(question_id),
                ))
                .await;
            }
        } else {
            self.respond(Rephrase).await;
        }
    }

    async fn check_if_terminated(&mut self) -> bool {
        if self.session.state == Terminated {
            return true;
        }
        if self.game.is_stop(self.message.as_str()) {
            self.session.state = Terminated;
            self.respond(Quit).await;
            self.store_progress().await;
            return true;
        }
        return false;
    }

    async fn answer_was_correct(&mut self, question_id: QuestionId) {
        self.session.record(
            question_id.topic(),
            self.game.get_bonus(question_id.topic()),
        );
        self.respond(Correct(self.session.score)).await;
        self.session.state = ChoosingTopic;
    }

    async fn answer_was_incorrect(
        &mut self,
        max_attempt: u8,
        num_attempt: u8,
        question_id: QuestionId,
    ) {
        let next_attempt = num_attempt + 1;
        if next_attempt >= max_attempt {
            self.respond(Incorrect).await;
            self.session.state = ChoosingTopic;
            self.session.record(question_id.topic(), 0);
        } else {
            self.respond(PleaseRetryLimits(max_attempt - next_attempt))
                .await;
            self.session.state = SessionState::answering(question_id, next_attempt);
        }
    }

    async fn answer_question(&mut self, attempt: AnswerAttempt) {
        if self
            .game
            .is_correct_answer(attempt.question_id, self.message.as_str())
        {
            self.answer_was_correct(attempt.question_id).await;
        } else {
            if let Some(max_attempt) = self.game.max_attempt {
                self.answer_was_incorrect(max_attempt, attempt.attempt, attempt.question_id)
                    .await;
            } else {
                self.respond(PleaseRetry).await;
            }
        }
    }

    async fn check_if_game_complete(&mut self) {
        if self.game.is_complete(self.session.results.len() as u8) {
            self.respond(GameComplete(self.session.score)).await;
            self.session.state = Complete;
        } else {
            if self.session.state == ChoosingTopic {
                self.respond(ChooseNextTopic).await;
            }
        }
    }

    pub async fn process(&mut self) {
        self.restore_session().await;
        if self.check_if_terminated().await {
            return;
        }
        match &self.session.state {
            New => self.greet().await,
            Deciding => self.has_user_agreed_to_start().await,
            ChoosingTopic => self.choose_topic().await,
            Answering(attempt) => {
                self.answer_question(attempt.clone()).await;
                self.check_if_game_complete().await;
            }
            Complete => {
                self.respond(GameComplete(self.session.score)).await;
            }
            _ => {}
        }
        self.store_progress().await;
    }
}

impl GameEngine {
    pub async fn process_message(
        &self,
        message: PlayerMessage,
        app_context: &'static dyn GameApplicationContext,
    ) {
        if let Some(channel) = app_context
            .definitions()
            .get_channel_by_id(&message.player_id.channel_id)
            .await
        {
            if let Some(game_id) = channel.game_id {
                if let Some(game) = app_context.definitions().get_game_by_id(game_id).await {
                    let mut ctx = MessageContext::new(app_context, game, channel, message);
                    ctx.process().await;
                } else {
                    log::debug!(
                        "Ignoring message from {}: game {} not found",
                        game_id,
                        message.player_id.channel_id
                    );
                }
            } else {
                log::debug!(
                    "Ignoring message from {}: no games configured for channel",
                    message.player_id.channel_id
                );
            }
        } else {
            log::debug!(
                "Ignoring message from {}: no channel config",
                message.player_id.channel_id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::game_engine::engine::GameEngine;
    use crate::game_engine::types::ResponseMessage::*;
    use crate::game_engine::types::{PlayerId, PlayerMessage, ResponseMessage};
    use crate::mock::game::MockContext;

    fn make_player_id() -> PlayerId {
        PlayerId {
            channel_id: "1".to_string(),
            id: "1".to_string(),
        }
    }

    async fn run_against_mock_from_start(messages: Vec<&str>) -> Vec<ResponseMessage> {
        let app_ctx = Arc::new(MockContext::new().await);
        let engine = GameEngine::default();
        let clone_ctx = Box::leak(Box::new(app_ctx.clone()));
        for message in messages.iter() {
            engine
                .process_message(
                    PlayerMessage {
                        player_id: make_player_id(),
                        text: message.to_string(),
                    },
                    clone_ctx,
                )
                .await
        }
        app_ctx.results()
    }

    async fn run_against_mock_in_session(mut messages: Vec<&str>) -> Vec<ResponseMessage> {
        let mut sequence = vec!["hello", "yes"];
        sequence.append(&mut messages);
        run_against_mock_from_start(sequence).await.split_off(2)
    }

    #[tokio::test]
    async fn test_engine_sends_rules_if_player_want_to_play() {
        assert_eq!(
            vec![
                ResponseMessage::greeting("#TEST_GAME"),
                ResponseMessage::rules(vec!["topic1", "topic2"]),
            ],
            run_against_mock_from_start(vec!["Hello", "yes"]).await
        )
    }

    #[tokio::test]
    async fn test_engine_stops_replying_on_decline() {
        assert_eq!(
            vec![ResponseMessage::greeting("#TEST_GAME"), Quit],
            run_against_mock_from_start(vec!["Hello", "no", "hey", "hey"]).await
        )
    }

    #[tokio::test]
    async fn test_engine_stops_replying_on_stop_word() {
        assert_eq!(
            vec![ResponseMessage::greeting("#TEST_GAME"), Quit],
            run_against_mock_from_start(vec!["Hello", "stop", "hey", "hey"]).await
        )
    }

    #[tokio::test]
    async fn test_engine_sends_question_when_user_chooses_a_topic() {
        assert_eq!(
            vec![AnswerQuestion("q11".to_string())],
            run_against_mock_in_session(vec!["topic1"]).await
        )
    }

    #[tokio::test]
    async fn test_when_user_answers_correctly_his_score_is_increased() {
        assert_eq!(
            vec![
                AnswerQuestion("q11".to_string()),
                Correct(1),
                ChooseNextTopic,
            ],
            run_against_mock_in_session(vec!["topic1", "ans11"]).await
        )
    }

    #[tokio::test]
    async fn test_when_user_answers_incorrectly_his_attempts_are_decreased() {
        assert_eq!(
            vec![
                AnswerQuestion("q11".to_string()),
                PleaseRetryLimits(1),
                Incorrect,
                ChooseNextTopic,
            ],
            run_against_mock_in_session(vec!["topic1", "no", "no"]).await
        )
    }

    #[tokio::test]
    async fn test_when_no_questions_is_left_user_receives_the_final_score() {
        assert_eq!(
            vec![
                AnswerQuestion("q11".to_string()),
                Correct(1),
                ChooseNextTopic,
                AnswerQuestion("q21".to_string()),
                Correct(2),
                GameComplete(2),
            ],
            run_against_mock_in_session(vec!["topic1", "ans11", "topic2", "ans2"]).await
        )
    }

    #[tokio::test]
    async fn test_user_can_not_choose_the_same_topic_twice() {
        assert_eq!(
            vec![
                AnswerQuestion("q11".to_string()),
                Correct(1),
                ChooseNextTopic,
                AlreadyAnswered,
            ],
            run_against_mock_in_session(vec!["topic1", "ans11", "topic1"]).await
        )
    }
}
