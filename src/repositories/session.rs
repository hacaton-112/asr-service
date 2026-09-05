use std::{collections::HashMap, time::Instant};

use async_trait::async_trait;
use mockall::automock;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::session::{PendingSession, SESSION_TTL};

#[automock]
#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, language: String) -> String;
    async fn take(&self, session_id: &str) -> Option<PendingSession>;
}

/// In-memory хранилище сессий. Сервис работает как один инстанс, а сессии
/// живут секунды до WS-подключения — реплицировать их между узлами незачем.
pub struct InMemorySessionRepositoryImpl {
    sessions: Mutex<HashMap<String, PendingSession>>,
}

impl InMemorySessionRepositoryImpl {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl SessionRepository for InMemorySessionRepositoryImpl {
    async fn create(&self, language: String) -> String {
        let session_id = Uuid::new_v4().to_string();
        let now = Instant::now();
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|_, session| session.expires_at > now);
        sessions.insert(
            session_id.clone(),
            PendingSession {
                language,
                expires_at: now + SESSION_TTL,
            },
        );
        session_id
    }

    // Сессия одноразовая: `remove` гарантирует, что повторный WS-коннект
    // с тем же sessionId ничего не найдёт.
    async fn take(&self, session_id: &str) -> Option<PendingSession> {
        self.sessions.lock().await.remove(session_id)
    }
}
