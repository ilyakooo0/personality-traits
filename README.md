# personality-traits-bot

Telegram-бот для практикума по снижению тревожности (Rust + teloxide + SQLite).

## Что реализовано

- Авторизация по email (одно сообщение с корректным email сохраняет пользователя).
- Кнопка «Начать курс» при первом запуске и «Начать курс заново» — после старта.
- Сценарий курса описан в `questionnaire/abfi-1/events.yaml`. Сообщения отправляются с задержкой `delay_minutes`, которая отсчитывается от нажатия кнопки в предыдущем сообщении.
- Сообщения вида `day_*` сопровождаются inline-кнопкой «Начать практику», `practice_*` — кнопкой «Завершить практику» (тексты берутся прямо из yaml).
- Все нажатия инлайн-кнопок логируются в таблицу `button_events` (telegram_id, message_id, button_id, event_target, время).
- Медиа из `questionnaire/abfi-1/data/`: `.png/.jpg/.jpeg` → `sendPhoto`, `.mp4` → `sendVideo`, `.mp3/.m4a` → `sendAudio`, `.ogg` → `sendVoice`, всё прочее (включая `.pdf` и `.webp`) → `sendDocument`. Отсутствующие файлы (например, `foo.mp4`) логируются как warning и пропускаются.

## Команды

- `/start` — зарегистрироваться (если ещё нет) или показать главное меню.
- `/restart` — завершить текущую сессию и начать курс заново.
- `/help` — список команд.

## Запуск

```sh
# 1. Скопируйте .env.example в .env и поставьте токен бота от @BotFather:
cp .env.example .env
# отредактируйте TELOXIDE_TOKEN

# 2. Запустите бота:
cargo run --release
```

При первом запуске:
- автоматически создаётся `bot.db` (SQLite);
- применяются миграции из `migrations/`;
- бот начинает long-polling Telegram.

## Переменные окружения

| Переменная        | Значение по умолчанию (см. `.env.example`)        |
|-------------------|----------------------------------------------------|
| `TELOXIDE_TOKEN`  | токен бота от @BotFather                           |
| `DATABASE_URL`    | `sqlite://bot.db?mode=rwc`                         |
| `EVENTS_FILE`     | `questionnaire/abfi-1/events.yaml`                 |
| `DATA_DIR`        | `questionnaire/abfi-1/data`                        |
| `RUST_LOG`        | `info,personality_traits_bot=debug,teloxide=info`  |

## Структура хранения

- `users` — `telegram_id`, `email`, `username`, `first_name`, `registered_at`.
- `course_sessions` — одна активная сессия на пользователя; `finished_at IS NULL` ⇒ курс идёт.
- `scheduled_messages` — очередь отложенной отправки (фоновый шедулер раз в 10 c забирает `send_after <= now`).
- `button_events` — журнал всех нажатий: `message_id`, `button_id`, `action`, `event_target`, `pressed_at`.

## Поток выполнения

1. Пользователь пишет `/start` → бот просит email.
2. После сохранения email бот отправляет первое сообщение из yaml (`id: start`) с кнопкой «Начать курс».
3. Нажатие кнопки → создаётся сессия, в `scheduled_messages` ставится следующее событие.
4. Шедулер каждые 10 секунд забирает «созревшие» сообщения, отправляет их, обновляет `current_index`.
5. Если у отправленного события нет кнопок (например, `day_1_end`), шедулер сразу планирует следующее по порядку с его `delay_minutes`.
6. Если кнопка ведёт на несуществующий `event` (например, опечатка `message_1` вместо `practice_1`), используется fallback — следующее сообщение по порядку.

## Известные нюансы yaml

- `event: message_1` в `day_1` — опечатка, по факту это `practice_1`. Обрабатывается fallback'ом.
- `id: day 4` (с пробелом) парсится, но не достижим по id. Обработчик переходит к нему через «next in order».
- `foo.mp4` в `start` отсутствует — будет warning в логах.

## Отладка

Поднять уровень логирования:

```sh
RUST_LOG=debug cargo run
```
