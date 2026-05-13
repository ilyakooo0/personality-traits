mod config;
mod db;
mod events;
mod handlers;
mod media;
mod scheduler;

use std::sync::Arc;

use teloxide::prelude::*;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = config::Config::from_env()?;
    tracing::info!("loading events from {}", cfg.events_file.display());
    let course = Arc::new(events::Course::load(&cfg.events_file, &cfg.data_dir)?);
    tracing::info!("loaded {} events", course.events.len());

    let pool = db::connect(&cfg.database_url).await?;
    tracing::info!("database ready at {}", cfg.database_url);

    let bot = Bot::from_env();

    let sched_bot = bot.clone();
    let sched_pool = pool.clone();
    let sched_course = course.clone();
    tokio::spawn(async move {
        scheduler::run(sched_bot, sched_pool, sched_course).await;
    });

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handlers::on_message))
        .branch(Update::filter_callback_query().endpoint(handlers::on_callback));

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![pool, course])
        .default_handler(|_| async {})
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
