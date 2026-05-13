use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use once_cell::sync::Lazy;
use regex::Regex;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage};

use crate::db::{self, Pool};
use crate::events::Course;
use crate::media;

static EMAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,}$").unwrap()
});

pub async fn on_message(
    bot: Bot,
    msg: Message,
    pool: Pool,
    course: Arc<Course>,
) -> Result<(), teloxide::RequestError> {
    if let Err(e) = on_message_inner(&bot, &msg, &pool, &course).await {
        tracing::error!("message handler error: {:?}", e);
        if let Some(user) = msg.from.as_ref() {
            let _ = bot
                .send_message(
                    ChatId(user.id.0 as i64),
                    "Что-то пошло не так. Попробуйте /start.",
                )
                .await;
        }
    }
    Ok(())
}

async fn on_message_inner(bot: &Bot, msg: &Message, pool: &Pool, course: &Course) -> Result<()> {
    let Some(user) = msg.from.as_ref() else {
        return Ok(());
    };
    let tg_id = user.id.0 as i64;
    let chat = msg.chat.id;
    let text = msg.text().unwrap_or("").trim();
    let username = user.username.as_deref();
    let first_name = Some(user.first_name.as_str());

    match text {
        "/start" => handle_start(bot, chat, tg_id, pool, course).await,
        "/restart" => handle_restart(bot, chat, tg_id, pool, course).await,
        "/help" => {
            bot.send_message(
                chat,
                "Команды:\n\
                 /start — начать или продолжить курс\n\
                 /restart — начать курс заново",
            )
            .await?;
            Ok(())
        }
        _ => {
            if db::get_user(pool, tg_id).await?.is_none() {
                handle_email_input(bot, chat, tg_id, username, first_name, text, pool, course).await
            } else {
                bot.send_message(
                    chat,
                    "Пожалуйста, используйте кнопки. Команды: /start, /restart, /help.",
                )
                .await?;
                Ok(())
            }
        }
    }
}

async fn handle_email_input(
    bot: &Bot,
    chat: ChatId,
    tg_id: i64,
    username: Option<&str>,
    first_name: Option<&str>,
    text: &str,
    pool: &Pool,
    course: &Course,
) -> Result<()> {
    if EMAIL_RE.is_match(text) {
        db::upsert_user(pool, tg_id, text, username, first_name).await?;
        bot.send_message(
            chat,
            format!(
                "Email сохранён: {}\n\
                 Пожалуйста, указывайте этот же email при заполнении анкет.",
                text
            ),
        )
        .await?;
        send_intro(bot, chat, course, false).await?;
    } else {
        bot.send_message(
            chat,
            "Похоже, это не email. Пожалуйста, отправьте корректный email одним сообщением \
             (например, name@example.com).",
        )
        .await?;
    }
    Ok(())
}

async fn handle_start(
    bot: &Bot,
    chat: ChatId,
    tg_id: i64,
    pool: &Pool,
    course: &Course,
) -> Result<()> {
    if db::get_user(pool, tg_id).await?.is_none() {
        bot.send_message(
            chat,
            "Здравствуйте! Чтобы зарегистрироваться, пожалуйста, отправьте ваш email \
             одним сообщением. Этот же email нужно будет указывать при заполнении анкет.",
        )
        .await?;
        return Ok(());
    }
    let has_session = db::active_session(pool, tg_id).await?.is_some();
    send_intro(bot, chat, course, has_session).await?;
    Ok(())
}

async fn handle_restart(
    bot: &Bot,
    chat: ChatId,
    tg_id: i64,
    pool: &Pool,
    course: &Course,
) -> Result<()> {
    if db::get_user(pool, tg_id).await?.is_none() {
        return handle_start(bot, chat, tg_id, pool, course).await;
    }
    send_intro(bot, chat, course, true).await?;
    Ok(())
}

async fn send_intro(bot: &Bot, chat: ChatId, course: &Course, restart_mode: bool) -> Result<()> {
    let Some(intro) = course.first() else {
        bot.send_message(chat, "Курс пуст. Свяжитесь с администратором.")
            .await?;
        return Ok(());
    };

    let files: Vec<_> = intro
        .files
        .iter()
        .filter_map(|f| course.resolve_file(f))
        .collect();
    media::send_files(bot, chat, &files).await;

    let (button_text, callback_data) = if restart_mode {
        ("Начать курс заново".to_string(), "crs:restart".to_string())
    } else {
        let label = intro
            .buttons
            .first()
            .map(|b| b.text.clone())
            .unwrap_or_else(|| "Начать курс".to_string());
        (label, "crs:start".to_string())
    };

    let kb = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        button_text,
        callback_data,
    )]]);

    let text = if intro.text.trim().is_empty() {
        "Добро пожаловать!"
    } else {
        intro.text.as_str()
    };
    bot.send_message(chat, text).reply_markup(kb).await?;
    Ok(())
}

pub async fn on_callback(
    bot: Bot,
    q: CallbackQuery,
    pool: Pool,
    course: Arc<Course>,
) -> Result<(), teloxide::RequestError> {
    if let Err(e) = on_callback_inner(&bot, &q, &pool, &course).await {
        tracing::error!("callback handler error: {:?}", e);
    }
    let _ = bot.answer_callback_query(q.id.clone()).await;
    Ok(())
}

async fn on_callback_inner(
    bot: &Bot,
    q: &CallbackQuery,
    pool: &Pool,
    course: &Course,
) -> Result<()> {
    let tg_id = q.from.id.0 as i64;
    let chat = ChatId(tg_id);
    let data = q.data.clone().unwrap_or_default();

    if let Some(msg) = q.message.as_ref() {
        let (cid, mid) = match msg {
            MaybeInaccessibleMessage::Regular(m) => (m.chat.id, m.id),
            MaybeInaccessibleMessage::Inaccessible(im) => (im.chat.id, im.message_id),
        };
        let _ = bot.edit_message_reply_markup(cid, mid).await;
    }

    match data.as_str() {
        "crs:start" => handle_course_start(bot, chat, tg_id, pool, course, false).await,
        "crs:restart" => handle_course_start(bot, chat, tg_id, pool, course, true).await,
        d if d.starts_with("btn:") => handle_event_button(bot, chat, tg_id, pool, course, d).await,
        _ => {
            tracing::warn!("unknown callback data: {}", data);
            Ok(())
        }
    }
}

async fn handle_course_start(
    bot: &Bot,
    chat: ChatId,
    tg_id: i64,
    pool: &Pool,
    course: &Course,
    restart: bool,
) -> Result<()> {
    if db::get_user(pool, tg_id).await?.is_none() {
        bot.send_message(chat, "Пожалуйста, сначала отправьте ваш email одним сообщением.")
            .await?;
        return Ok(());
    }

    db::finish_all_active(pool, tg_id).await?;
    let session = db::start_session(pool, tg_id).await?;

    let intro = course
        .first()
        .ok_or_else(|| anyhow::anyhow!("course has no events"))?;
    let button = intro.buttons.first();
    let target = button.map(|b| b.event.as_str()).unwrap_or("");

    db::record_button(
        pool,
        tg_id,
        Some(session.id),
        &intro.id,
        button.map(|b| b.id.as_str()).unwrap_or("0"),
        if restart { "restart_course" } else { "start_course" },
        if target.is_empty() { None } else { Some(target) },
    )
    .await?;

    if let Some(next_idx) = course.next_after_button(0, target) {
        schedule_next(pool, &session, course, next_idx, Utc::now()).await?;
        if restart {
            bot.send_message(chat, "Курс начат заново. Скоро вы получите первое сообщение.")
                .await?;
        } else {
            bot.send_message(chat, "Курс начат. Скоро вы получите первое сообщение.")
                .await?;
        }
    } else {
        db::finish_session(pool, session.id).await?;
        bot.send_message(chat, "В курсе нет следующих сообщений после стартового.")
            .await?;
    }
    Ok(())
}

async fn handle_event_button(
    bot: &Bot,
    chat: ChatId,
    tg_id: i64,
    pool: &Pool,
    course: &Course,
    data: &str,
) -> Result<()> {
    let parts: Vec<&str> = data.splitn(3, ':').collect();
    if parts.len() != 3 {
        return Ok(());
    }
    let event_idx: usize = parts[1].parse().unwrap_or(usize::MAX);
    let btn_idx: usize = parts[2].parse().unwrap_or(usize::MAX);

    let Some(session) = db::active_session(pool, tg_id).await? else {
        bot.send_message(chat, "Нет активного курса. Нажмите /start, чтобы начать.")
            .await?;
        return Ok(());
    };

    let Some(event) = course.at(event_idx) else {
        tracing::warn!("callback references unknown event idx {}", event_idx);
        return Ok(());
    };
    let Some(button) = event.buttons.get(btn_idx) else {
        tracing::warn!(
            "callback references unknown button idx {} on event {}",
            btn_idx,
            event.id
        );
        return Ok(());
    };

    let target = button.event.as_str();
    db::record_button(
        pool,
        tg_id,
        Some(session.id),
        &event.id,
        &button.id,
        &button.action,
        if target.is_empty() { None } else { Some(target) },
    )
    .await?;

    let Some(next_idx) = course.next_after_button(event_idx, target) else {
        db::finish_session(pool, session.id).await?;
        bot.send_message(chat, "Поздравляем, вы прошли весь практикум!")
            .await?;
        return Ok(());
    };

    schedule_next(pool, &session, course, next_idx, Utc::now()).await?;
    Ok(())
}

async fn schedule_next(
    pool: &Pool,
    session: &db::Session,
    course: &Course,
    next_idx: usize,
    trigger_at: chrono::DateTime<Utc>,
) -> Result<()> {
    let next = course
        .at(next_idx)
        .ok_or_else(|| anyhow::anyhow!("invalid next_idx {}", next_idx))?;
    let send_after = trigger_at + chrono::Duration::minutes(next.delay_minutes.max(0));
    db::schedule(pool, session.id, session.telegram_id, &next.id, send_after).await?;
    tracing::info!(
        "scheduled '{}' (idx {}) for user {} at {} (delay {} min)",
        next.id,
        next_idx,
        session.telegram_id,
        send_after,
        next.delay_minutes
    );
    Ok(())
}

pub async fn send_event_with_keyboard(
    bot: &Bot,
    chat: ChatId,
    course: &Course,
    event_idx: usize,
) -> Result<()> {
    let event = course
        .at(event_idx)
        .ok_or_else(|| anyhow::anyhow!("event idx {} not found", event_idx))?;

    let files: Vec<_> = event
        .files
        .iter()
        .filter_map(|f| course.resolve_file(f))
        .collect();
    media::send_files(bot, chat, &files).await;

    let rows: Vec<Vec<InlineKeyboardButton>> = event
        .buttons
        .iter()
        .enumerate()
        .map(|(i, b)| {
            vec![InlineKeyboardButton::callback(
                b.text.clone(),
                format!("btn:{}:{}", event_idx, i),
            )]
        })
        .collect();

    let text = if event.text.trim().is_empty() {
        "(пустое сообщение)"
    } else {
        event.text.as_str()
    };

    if rows.is_empty() {
        bot.send_message(chat, text).await?;
    } else {
        bot.send_message(chat, text)
            .reply_markup(InlineKeyboardMarkup::new(rows))
            .await?;
    }
    Ok(())
}
