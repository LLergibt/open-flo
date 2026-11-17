# Контракт авторизации и пар

Рабочая спецификация текущего этапа. API, web и Android используют одинаковые поля. Телефон не используется для проверок.

## Модели

User: { id: string UUID, username: string, display_name: string, role: "admin" | "tracker" | "partner", is_active: boolean }. Внешние ответы никогда не содержат password_hash.

Pair: { id: string UUID, tracker: User, partner: User, created_at: string ISO8601 }. Вложенные User могут содержать только публичные поля модели, без секретов.

Общие ошибки: { error: string }; стабильные значения unauthorized, forbidden, invalid_input, invalid_credentials, conflict, rate_limited, internal_error. HTTP 401/403/400/409/429/500 соответственно. Клиенты переводят коды на русский, не показывают SQL и сырые исключения.

## Маршруты

- POST /api/auth/login { username, password, client?: "web" | "native" }; default web. Ответ { user: User, access_token: string, expires_in: 900, refresh_token?: string }. refresh_token включается в JSON только native-клиенту. Web получает HttpOnly cookies, не сохраняет токены в JS/localStorage.
- POST /api/auth/refresh: web читает refresh-cookie; native передаёт { refresh_token, client: "native" }. Ответ как login; refresh ротируется.
- POST /api/auth/logout: отзывает сессию и очищает cookies; native передаёт Authorization: Bearer access_token, при необходимости { refresh_token, client: "native" }. Результат HTTP 204. Должен безопасно завершать и истёкший web access при ещё действительном refresh.
- GET /api/me -> User. JWT из access-cookie или Authorization Bearer; приоритет Bearer, некорректный Bearer не откатывается на cookies.
- GET /api/me/pair -> { pair: Pair | null }. Только своя пара; список чужих пар доступен лишь администратору.
- DELETE /api/me/pair ->204. Любой участник может прекратить связь, последующие запросы показывают null. Старая пара не даёт новых прав.
- GET /api/admin/users -> { users: User[] }.
- POST /api/admin/users { username, display_name, password, role } ->201 User. Только admin, самостоятельная регистрация разрешена только партнёру по действующему приглашению.
- PATCH /api/admin/users/{id} { is_active?: boolean, password?: string } ->User. Смена пароля и отключение отзывают сессии; не допускать отключения последнего активного admin и собственной случайной блокировки. Изменение role пока не поддерживается.
- GET /api/admin/pairs -> { pairs: Pair[] }.
- POST /api/admin/pairs { tracker_id, partner_id } ->201 Pair. Только admin. Разные существующие активные аккаунты требуемых ролей, один активный pair на каждого участника, атомарная проверка/создание.
- DELETE /api/admin/pairs/{id} ->204. Только admin.
- GET /api/setup/status -> { needs_setup: boolean }. Только флаг наличия первого администратора; bootstrap через локальную CLI-команду.
- /health и прежний /api/message остаются публичными, содержат исключительно техническую проверку/синтетический текст. Защищённые сведения аккаунтов в них не добавляются.

## Сессии и безопасность

JWT access HS256, 15 минут; фиксированный алгоритм, проверка exp/issuer/audience, session id. Сессия и актуальный user проверяются в БД при защищённых запросах: отключение пользователя, logout и отзыв должны действовать немедленно. Оpaque refresh token 7 дней, криптографически случайный; в БД только hash. Ротация при refresh и защита от повторного использования: повтор старого token отзывает соответствующую сессию. Текущий и предыдущий hash могут сохраняться для обнаружения replay, сырые tokens не логируются.

Пароли Argon2id с индивидуальной солью, как минимум m=19456 KiB,t=2,p=1. Длина 12–128 символов с ограничением байтов; не обрезать пароль молча. username ASCII lowercase 3–64 [a-z0-9_.-], display_name 1–80. Нормализация username одинакова при входе и создании. Неверные username/password возвращают одинаковую ошибку.

Login ограничивается по частоте; ресурсоёмкое хеширование выполняется вне async runtime и ограничивается по параллельности. Ограничение размера JSON и input validation.

Ключ JWT задаётся JWT_SECRET или JWT_SECRET_FILE; без конфигурации можно сгенерировать криптографически случайный ключ 32 байта в .data/jwt.key с правами 0600, не заменяя его при перезапуске. Отказ от слабых/пустых ключей. Ключи не попадают в Git.

Web: HttpOnly cookies flo_access и flo_refresh, SameSite=Strict, Path=/api. Secure по умолчанию; COOKIE_SECURE=false допускается только для явного локального dev. Browser-changing requests проверяют Origin в списке TRUSTED_ORIGINS, без wildcard. Requests с cookie и без Origin на изменениях отклонять; native Bearer/refresh requests без browser cookies допустимы. Не доверять client=native для обхода проверки Origin, если запрос содержит cookies/Origin. Отсутствие CORS сохраняется. Origin login проверять для web, включая защиту login CSRF.

Клиенты: web использует cookies и credentials same-origin; токены не пишутся в localStorage/sessionStorage. Android хранит access/refresh только в памяти на этом этапе, после перезапуска требует вход; пароли не сохраняются. Авто-refresh не более одного повтора на исходный запрос, logout очищает локальное состояние. При 403 не обновлять токены и не зацикливаться.

Первый admin — локальная CLI create-admin, интерактивный скрытый пароль либо --password-stdin. Скрипт create-admin.sh предлагает логин и имя root, пароль вводится пользователем; аккаунты автоматически не создаются. CLI предотвращает bootstrap второго admin; остальных создаёт admin через UI. Тестовые фикстуры только в тестах и временной БД.

## Проверки

Успех входа, неверный пароль, неизвестный user, истёкший/поддельный JWT, refresh/replay/logout, отключённый user, обход admin role, перечисление чужих пар, self/wrong-role/duplicate pair, атомарность создания, отзыв связи. Web login→admin create accounts→pair→logout→member login→own pair→unlink. Android компиляция и unit/static checks без телефона. Продакшен с паролями требует HTTPS; пароль/refresh не выводить в логи и отчёты.

Дополнение: [приглашения и смена пароля](INVITES_CONTRACT.md).
