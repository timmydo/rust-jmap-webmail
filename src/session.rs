use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

pub struct Session {
    pub username: String,
    pub password: String,
    pub api_url: String,
    pub account_id: String,
    pub download_url: Option<String>,
}

pub struct SessionStore {
    sessions: RwLock<HashMap<Uuid, Session>>,
}

impl SessionStore {
    pub fn new() -> Self {
        SessionStore {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub fn create(&self, session: Session) -> Uuid {
        let id = Uuid::now_v7();
        self.sessions.write().unwrap().insert(id, session);
        id
    }

    pub fn get<F, R>(&self, id: &Uuid, f: F) -> Option<R>
    where
        F: FnOnce(&Session) -> R,
    {
        self.sessions.read().unwrap().get(id).map(f)
    }

    pub fn remove(&self, id: &Uuid) -> Option<Session> {
        self.sessions.write().unwrap().remove(id)
    }

    pub fn exists(&self, id: &Uuid) -> bool {
        self.sessions.read().unwrap().contains_key(id)
    }
}

pub fn parse_session_cookie(cookie_header: &str) -> Option<Uuid> {
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(value) = cookie.strip_prefix("session=") {
            return Uuid::parse_str(value).ok();
        }
    }
    None
}

pub fn make_session_cookie(id: &Uuid) -> String {
    format!(
        "session={}; HttpOnly; SameSite=Strict; Path=/",
        id
    )
}

pub fn clear_session_cookie() -> String {
    "session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0".to_string()
}
