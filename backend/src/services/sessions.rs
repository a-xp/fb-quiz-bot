use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;

use crate::game_engine::types::{GameId, GameSession, PlayerId, SessionRepository};

#[derive(Default)]
pub struct InMemorySessionRepository {
    store: RwLock<HashMap<u32, HashMap<PlayerId, GameSession>>>,
}

#[async_trait]
impl SessionRepository for InMemorySessionRepository {
    async fn get_by_id(&self, game_id: GameId, player_id: &PlayerId) -> Option<GameSession> {
        let l = self.store.read().unwrap();
        l.get(&game_id)
            .and_then(|sessions| sessions.get(player_id).cloned())
    }

    async fn store(&self, session: &GameSession) {
        let mut l = self.store.write().unwrap();
        if !l.contains_key(&session.game_id) {
            l.insert(session.game_id, Default::default());
        }
        l.get_mut(&session.game_id)
            .unwrap()
            .insert(session.player_id.clone(), session.clone());
    }
}
