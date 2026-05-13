use std::path::{Path, PathBuf};

use teloxide::prelude::*;
use teloxide::types::InputFile;

pub async fn send_files(bot: &Bot, chat: ChatId, files: &[PathBuf]) {
    for path in files {
        send_one(bot, chat, path).await;
    }
}

pub async fn send_one(bot: &Bot, chat: ChatId, path: &Path) {
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
