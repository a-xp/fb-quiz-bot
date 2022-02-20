use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;

use crate::game_engine::game_def::Game;
use crate::game_engine::types::{Channel, ChannelId, DefinitionsRepository, GameId};

pub struct FileRepository {
    games: RwLock<HashMap<GameId, Arc<Game>>>,
    channels: RwLock<HashMap<ChannelId, Arc<Channel>>>,
    data_dir: PathBuf,
}

#[async_trait]
impl DefinitionsRepository for FileRepository {
    async fn get_game_by_id(&self, game_id: GameId) -> Option<Arc<Game>> {
        let l = self.games.read().unwrap();
        l.get(&game_id).cloned()
    }

    async fn get_channel_by_id(&self, channel_id: &ChannelId) -> Option<Arc<Channel>> {
        let l = self.channels.read().unwrap();
        l.get(channel_id).cloned()
    }
}

impl FileRepository {
    pub async fn load(data_dir: &PathBuf) -> anyhow::Result<FileRepository> {
        let channels = Self::load_channels(data_dir).await?;
        let games = Self::load_games(data_dir).await?;
        log::info!(
            "Loaded {} channels and {} games",
            channels.len(),
            games.len()
        );
        anyhow::Ok(FileRepository {
            games: RwLock::new(games),
            channels: RwLock::new(channels),
            data_dir: data_dir.clone(),
        })
    }

    async fn load_channels(data_dir: &PathBuf) -> anyhow::Result<HashMap<ChannelId, Arc<Channel>>> {
        let content = tokio::fs::read(data_dir.join("channels.json")).await?;
        let channels: Vec<Channel> = serde_json::from_slice(content.as_slice())?;
        anyhow::Ok(
            channels
                .into_iter()
                .map(|c| (c.channel_id.clone(), Arc::new(c)))
                .collect(),
        )
    }

    async fn load_games(data_dir: &PathBuf) -> anyhow::Result<HashMap<GameId, Arc<Game>>> {
        let mut list = tokio::fs::read_dir(data_dir).await?;
        let mut result: Vec<Game> = Default::default();
        while let Some(file) = list.next_entry().await? {
            if file.file_name().is_ascii()
                && file.file_name().to_string_lossy().starts_with("game-")
            {
                result.push(Game::load(&file.path()).await?)
            }
        }
        anyhow::Ok(result.into_iter().map(|g| (g.id, Arc::new(g))).collect())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::game_engine::types::GameId;
    use crate::services::definitions::FileRepository;

    fn test_data_dir() -> PathBuf {
        std::env::current_dir()
            .unwrap()
            .join("src")
            .join("test_resources")
            .join("games")
    }

    #[tokio::test]
    async fn test_games_are_loaded_correctly_from_file() {
        let games = FileRepository::load_games(&test_data_dir()).await.unwrap();
        assert_eq!(1, games.len());
        assert_eq!("#TEST_GAME", games.get(&1).unwrap().name)
    }

    #[tokio::test]
    async fn test_channels_are_loaded_correctly_from_file() {
        let channels = FileRepository::load_channels(&test_data_dir())
            .await
            .unwrap();
        assert_eq!(1, channels.len());
        assert_eq!("test channel", channels.get("#id1").unwrap().name)
    }
}
