pub mod catalog;
pub mod config;
pub mod counters;
pub mod http;
pub mod identity;
pub mod model;
pub mod notifications;
pub mod sync;
pub mod teams;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: config::Config,
    pub clock: Arc<dyn Clock>,
}
