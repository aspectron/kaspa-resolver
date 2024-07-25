use std::time::SystemTime;

use crate::imports::*;

#[derive(Debug)]
struct Inner {
    ts: AtomicU64,
    // ts : Instant,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            ts: AtomicU64::new(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Session {
    inner: Arc<Inner>,
}

impl Session {
    #[inline]
    pub fn ts(&self) -> u64 {
        self.inner.ts.load(Ordering::Relaxed)
    }
    #[inline]
    pub fn lifetime(&self, now: u64) -> Duration {
        let ts = self.inner.ts.load(Ordering::Relaxed);
        Duration::from_secs(now - ts)
    }
    #[inline]
    pub fn touch(&self) {
        self.inner.ts.store(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::Relaxed,
        );
    }
}

pub struct Sessions {
    sessions: RwLock<HashMap<String, Session>>,
    ttl: Duration,
    capacity: usize,
}

impl Sessions {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            ttl,
            capacity,
        }
    }

    pub fn get(&self, key: &str) -> Option<Session> {
        self.sessions.read().unwrap().get(key).cloned()
    }

    pub fn set(&self, key: &str, session: Session) {
        self.sessions
            .write()
            .unwrap()
            .insert(key.to_string(), session);
    }

    pub fn remove(&self, key: &str) {
        self.sessions.write().unwrap().remove(key);
    }

    pub fn cleanup(&self) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut sessions = self.sessions.write().unwrap();
        sessions.retain(|_, session| session.lifetime(now) < self.ttl);
        if sessions.len() > self.capacity {
            let to_remove = {
                let mut vec = sessions.iter().collect::<Vec<_>>();
                vec.sort_by_key(|(_, session)| session.ts());
                let to_remove = vec.len() - self.capacity;
                vec.iter()
                    .take(to_remove)
                    .map(|(key, _)| (*key).clone())
                    .collect::<Vec<_>>()
            };

            for key in to_remove {
                sessions.remove(key.as_str());
            }
        }
    }
}
