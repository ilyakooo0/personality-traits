use std::path::{Path, PathBuf};

use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, InputFile};

const CAPTION_LIMIT: usize = 1024;

pub async fn send_event_message(
    bot: &Bot,
    chat: ChatId,
    files: &[PathBuf],
    text: &str,
    keyboard: Option<InlineKeyboardMarkup>,
) {
    if files.is_empty() {
        send_text(bot, chat, text, keyboard).await;
        return;
    }

    let fits_in_caption = text.chars().count() <= CAPTION_LIMIT;
    if files.len() == 1 && fits_in_caption {
        match send_one_with_caption(bot, chat, &files[0], text, keyboard.clone()).await {
            Ok(()) => return,
            Err(e) => tracing::warn!(
                "failed to send {} with caption: {}; falling back to separate messages",
                files[0].display(),
                e
            ),
        }
    }

    for path in files {
        send_one(bot, chat, path).await;
    }
    send_text(bot, chat, text, keyboard).await;
}

async fn send_text(bot: &Bot, chat: ChatId, text: &str, keyboard: Option<InlineKeyboardMarkup>) {
    let text = if text.trim().is_empty() {
        "(пустое сообщение)"
    } else {
        text
    };
    let result = match keyboard {
        Some(kb) => bot.send_message(chat, text).reply_markup(kb).await.map(|_| ()),
        None => bot.send_message(chat, text).await.map(|_| ()),
    };
    if let Err(e) = result {
        tracing::error!("failed to send text to {}: {}", chat, e);
    }
}

async fn send_one_with_caption(
    bot: &Bot,
    chat: ChatId,
    path: &Path,
    caption: &str,
    keyboard: Option<InlineKeyboardMarkup>,
) -> Result<(), teloxide::RequestError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "webp" => {
            let req = bot
                .send_photo(chat, InputFile::file(path))
                .caption(caption);
            match keyboard {
                Some(kb) => req.reply_markup(kb).await?,
                None => req.await?,
            };
        }
        "mp4" => {
            let req = bot
                .send_video(chat, InputFile::file(path))
                .caption(caption);
            match keyboard {
                Some(kb) => req.reply_markup(kb).await?,
                None => req.await?,
            };
        }
        "mp3" | "m4a" => {
            let req = bot
                .send_audio(chat, InputFile::file(path))
                .caption(caption);
            match keyboard {
                Some(kb) => req.reply_markup(kb).await?,
                None => req.await?,
            };
        }
        "ogg" => {
            let req = bot
                .send_voice(chat, InputFile::file(path))
                .caption(caption);
            match keyboard {
                Some(kb) => req.reply_markup(kb).await?,
                None => req.await?,
            };
        }
        _ => {
            let req = bot
                .send_document(chat, InputFile::file(path))
                .caption(caption);
            match keyboard {
                Some(kb) => req.reply_markup(kb).await?,
                None => req.await?,
            };
        }
    }
    Ok(())
}

async fn send_one(bot: &Bot, chat: ChatId, path: &Path) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let result = match ext.as_str() {
        "jpg" | "jpeg" | "png" | "webp" => bot
            .send_photo(chat, InputFile::file(path))
            .await
            .map(|_| ()),
        "mp4" => bot
            .send_video(chat, InputFile::file(path))
            .await
            .map(|_| ()),
        "mp3" | "m4a" => bot
            .send_audio(chat, InputFile::file(path))
            .await
            .map(|_| ()),
        "ogg" => bot
            .send_voice(chat, InputFile::file(path))
            .await
            .map(|_| ()),
        _ => bot
            .send_document(chat, InputFile::file(path))
            .await
            .map(|_| ()),
    };

    if let Err(e) = result {
        tracing::warn!(
            "failed to send {} as {}: {}; retrying as document",
            path.display(),
            ext,
            e
        );
        if let Err(e2) = bot.send_document(chat, InputFile::file(path)).await {
            tracing::error!("failed to send {} as document: {}", path.display(), e2);
        }
    }
}
