# Календарь и личный дневник: текущий этап

Разрешены API/БД и только веб. Не добавлять и не запускать тесты. Разрешены компиляция, проверка типов, сборка, статическое ревью расчёта и ручная проверка интерфейса на синтетических данных. Android, телефон, деплой и push не входят в задачу.

## Права и приватность

Владелец ведёт свой календарь и дневник. Активный партнёр читает календарь, прогноз и все записи дневника текущего активного владельца через существующую пару. Изменять или удалять записи может только владелец. Настройки прогноза и подробные причины его приостановки доступны владельцу. Администратор не получает медицинские данные через API. Сервер проверяет роль, сессию и пару в согласованной транзакции чтения. Идентификатор владельца определяется сервером. Записи привязаны к владельцу с ON DELETE CASCADE. Мутации используют Origin и actor_transaction.

## API

Все маршруты в существующем auth Router (единые no-store/body limit/Origin/session policies).
- GET /api/calendar -> CalendarData (tracker или связанный partner).
- PUT /api/cycle/settings {cycle_length:number|null,period_length:number,context:string,length_method:"auto"|"manual"} ->204, только tracker.
- POST /api/cycle/periods {start_date:YYYY-MM-DD,end_date:YYYY-MM-DD|null} ->201 Period.
- PUT /api/cycle/periods/{id} те же поля ->204; принадлежность обязательна.
- DELETE /api/cycle/periods/{id} ->204; принадлежность обязательна.
- PUT /api/diary/{date} {Допустимые коды symptoms и events перечислены в общем каталоге apps/web/src/lib/diary-catalog.json.
mood:string,flow:string,pain:number,notes:string} ->204, upsert владельца.
- DELETE /api/diary/{date} ->204 только tracker.

Для календарных маршрутов клиент передаёт X-Calendar-Date с локальным YYYY-MM-DD (не timestamp). Разрешена разность с текущим UTC-днём не более1дня, иначе400. Без заголовка UTC fallback. Использовать этот as_of при валидации и прогнозе.

CalendarData = {access:"owner"|"partner",owner_display_name:string,as_of:YYYY-MM-DD,periods:Period[],entries:DiaryEntry[],prediction:Prediction,settings:CycleSettings|null}.
Для партнёра settings=null. Записи календаря и дневника читаются в одной транзакции, чтобы ответ относился к одному владельцу.
CycleSettings = {cycle_length:number|null,period_length:number,context:string,length_method:"auto"|"manual"}; default null,5,"regular","auto". cycle_length10..90, period_length1..14; context regular|irregular|pregnancy|postpartum|hormonal_contraception|paused. Настройка cycle_length=null означает отсутствие введённого числа. Правила стартовой оценки и выбора длины см. в CYCLE_ALGORITHM.md.
Period = {id:string,start_date:string,end_date:string|null}. Факты, не прогнозы. Date-only Gregorian 2000..2100, start/end не позже текущего дня; end>=start, максимум90 дней записи (не отбрасывать длительные кровотечения). Не допускать пересечения включительных диапазонов. end=null — продолжающиеся месячные: только последняя запись, не более одной открытой; фактическое отображение до as_of. При редактировании не переносить дневник автоматически: записи привязаны к календарным датам, не к номеру дня цикла. Можно исправить любые прошлые start/end, добавить историю и удалить ошибочную запись. Максимум500 периодов.
DiaryEntry = {date,symptoms,events,mood,flow,pain,notes}; date не в будущем, notes<=2000символов; pain0..3.
symptoms: cramps,headache,back_pain,breast_tenderness,bloating,fatigue,nausea,acne.
events: masturbation,orgasm,sex_protected,sex_unprotected.
mood: ""|calm|happy|sad|anxious|irritable|energetic.
flow: ""|spotting|light|medium|heavy.
Не использовать симптомы/секс/оргазм для изменения прогноза. Не писать содержимое в логи.

## Расчёт: root реализует crates/api/src/cycle.rs

Реализация чистых функций без SQL/HTTP/UI. pub(crate) structs:
CycleSettings {cycle_length:Option<i64>,period_length:i64,context:String,length_method:String}
Period {id:String,start_date:String,end_date:Option<String>}
Prediction сериализуется с полями:
{algorithm_version:string,status:string,reason:string|null,source:string|null,cycle_length:number|null,period_length:number,
next_period_start:string|null,next_period_end:string|null,ovulation_date:string|null,fertile_start:string|null,fertile_end:string|null,interval_count:number,variability_days:number|null,additional_cycles:CycleEstimate[]}.
API вызывает crate::cycle::predict(&settings,&periods,as_of:time::Date) -> Prediction.
root добавляет mod cycle в lib.rs и parsing feature time; backend эти файлы не меняет.
pub(crate) fn parse_date(&str)->Option<time::Date>, date_string(Date)->String.

Старт месячных — день1, длина цикла — разность соседних start. N=S+L, прогноз месячных N..N+D-1. ASRM calendar convention: день овуляции L-14 (1-based), дата S+(L-15), базовое окно O-5..O расширено преимущественно после расчётной даты овуляции. Это календарная оценка, не установление овуляции. Не применять для контрацепции и не обозначать оставшиеся дни безопасными.

CycleEstimate содержит kind (retrospective или projected), period_start, period_end, ovulation_date, fertile_start и fertile_end. Даты имеют формат YYYY-MM-DD или null. Ретроспективная оценка использует фактическую длину завершённого цикла. Три будущих начала проецируются от последнего фактического старта. Расширенное окно соответствует интервалу O − 6 .. O + 4 в границах цикла.

Актуальные правила расчёта, подавления оценки овуляции и выбора исходных данных описаны в CYCLE_ALGORITHM.md.

UI: минималистичный Y2K, отдельные компоненты календаря/форм месячных/настроек/дневника; общие date-only helpers. Красный факт, пунктир красный прогноз, синий окно, кольцо+подпись овуляции; легенда и текст, цвет не единственный признак. Выбор дня, прошлые/следующие месяцы и сегодня. Owner может изменить даты последних месячных через редактирование истории, настроить свою длительность, отмечать/удалять дневник дня. Партнёр просматривает календарь, прогноз и записи выбранного дня. Формы редактирования доступны владельцу. Однозначные пояснения, что именно доступно партнёру.

Медиана для чётного числа значений — среднее двух центральных с округлением .5 вверх. reason: no_periods|invalid_history|context_paused|fertility_uncertain|period_overdue|null. Для партнёра подавленные причины заменить на prediction_unavailable; не раскрывать контекст здоровья.
