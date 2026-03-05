use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;
use vex_core::schema::LogEntry;

pub type BuildLogChannels = Arc<DashMap<Uuid, broadcast::Sender<LogEntry>>>;

pub fn new() -> BuildLogChannels {
    Arc::new(DashMap::new())
}

pub fn subscribe(
    channels: &BuildLogChannels,
    deployment_id: Uuid,
) -> broadcast::Receiver<LogEntry> {
    channels
        .entry(deployment_id)
        .or_insert_with(|| broadcast::channel(512).0)
        .subscribe()
}

pub async fn send(
    channels: &BuildLogChannels,
    pool: &PgPool,
    deployment_id: Uuid,
    message: String,
) {
    let entry = LogEntry {
        timestamp: Utc::now().to_rfc3339(),
        message: message.clone(),
    };

    let _ = sqlx::query("INSERT INTO build_logs (id, deployment_id, message) VALUES ($1, $2, $3)")
        .bind(Uuid::now_v7())
        .bind(deployment_id)
        .bind(&message)
        .execute(pool)
        .await;

    if let Some(tx) = channels.get(&deployment_id) {
        let _ = tx.send(entry);
    }
}

pub fn remove(channels: &BuildLogChannels, deployment_id: Uuid) {
    channels.remove(&deployment_id);
}
