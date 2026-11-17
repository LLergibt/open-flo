# Android: тестовый экран

Нативное приложение Kotlin / Jetpack Compose получает сообщение из `GET /api/message`. Сервер читает текст из SQLite. Экран показывает загрузку, сообщение или ошибку с повторной попыткой. Это каркас с демонстрационными данными; расчёты цикла и аккаунты на этом этапе ещё не реализованы.

## Окружение на macOS

Установить Android Studio через Homebrew:

```bash
brew install --cask android-studio
open -a "Android Studio"
```

В мастере установки загрузить Android SDK. Через SDK Manager установить Android SDK Platform 35, Build-Tools 35.0.0, Platform-Tools и Android Emulator. В Device Manager создать виртуальный телефон; на Apple Silicon выбрать ARM64-образ.

Для закреплённой сборки использовать JDK 17. Если совместимый JDK уже доступен в Android Studio, выбрать его в настройке Gradle JDK. Для отдельной установки:

```bash
brew install openjdk@17
export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
export ANDROID_HOME="$HOME/Library/Android/sdk"
export PATH="$ANDROID_HOME/platform-tools:$PATH"
```

Не требуется устанавливать глобальный Gradle: используется проверяемый Gradle Wrapper. Согласование лицензий SDK выполняется при установке соответствующих пакетов.

## Запуск в Android Studio

1. Сначала запустить API по инструкции в корне репозитория.
2. Открыть `/Users/georgijnazarov/projects/open-source-flo/apps/android` как проект Android Studio.
3. Дождаться Gradle Sync и выбрать Gradle JDK 17.
4. Запустить виртуальный телефон в Device Manager.
5. Выбрать конфигурацию `app` и нажать Run.

По умолчанию debug-сборка обращается к `http://10.0.2.2:8080`. Адрес `10.0.2.2` в Android Emulator указывает на Mac, на котором работает API. `localhost` внутри эмулятора указывает на сам эмулятор.

После изменения текста в демонстрационной БД нажать «Обновить». При недоступном API появится ошибка; после запуска сервера доступна повторная попытка.

## Сборка из терминала

```bash
cd /Users/georgijnazarov/projects/open-source-flo/apps/android
./gradlew assembleDebug
./gradlew installDebug
```

APK создаётся в `app/build/outputs/apk/debug/app-debug.apk`. `installDebug` требует запущенного эмулятора или подключённого Android-устройства.

Для физического телефона включить USB debugging, подключить его и подтвердить доверие компьютеру. При доступном `adb` можно пробросить порт API через USB:

```bash
adb reverse tcp:8080 tcp:8080
./gradlew installDebug -PAPI_BASE_URL=http://127.0.0.1:8080
```

Вариант с сервером в локальной сети:

```bash
./gradlew assembleDebug -PAPI_BASE_URL=http://192.168.1.10:8080
```

Для LAN-сценария API должен слушать доступный сетевой интерфейс; настройки описаны в корневой инструкции. HTTP разрешён только в debug-сборке для синтетических данных.

## Release и сеть

Release-сборка запрещает незашифрованный HTTP через Android Network Security Configuration. Без явно заданного HTTPS-адреса задача release завершается ошибкой:

```bash
./gradlew assembleRelease -PAPI_BASE_URL=https://your-server.example
```

Адрес не должен содержать пароль, query-параметры или fragment. Для проверки интерфейса подходят debug-сборки. Release signing и публикация пока не настроены.

Запрос выполняется на `Dispatchers.IO` через штатный `HttpURLConnection`; connect/read timeout — по 8 секунд. Перенаправления отключены, ответ ограничен 64 КиБ, JSON проверяется. Отмена ViewModel не преобразуется в пользовательскую ошибку. Записи дневника, секреты и реальные пользовательские данные в этом каркасе не используются.

## Закреплённые версии и происхождение wrapper

| Компонент | Версия |
|---|---|
| Android Gradle Plugin | 8.9.2 |
| Gradle | 8.11.1 |
| JDK | 17 |
| Kotlin и Compose Compiler plugin | 2.1.20 |
| Compose BOM | 2025.04.01 |
| Compile / Target SDK | 35 / 35 |
| Minimum SDK | 26 |

Gradle Wrapper (`gradlew`, `gradlew.bat`, `gradle-wrapper.jar`) получен из официального репозитория Gradle, тег `v8.11.1`. Скрипты сохраняют исходные лицензионные заголовки.

- Wrapper JAR SHA256: `2db75c40782f5e8ba1fc278a5574bab070adccb2d21ca5a6e5ed840888448046`.
- Gradle binary ZIP SHA256: `f397b287023acdba1e9f6fc5ea72d22dd63669d59ed4a289a29b1a76eee151c6`.

Checksum дистрибутива закреплён в `gradle/wrapper/gradle-wrapper.properties`. Перед добавлением JAR его checksum сверен с официальной публикацией Gradle. Первый запуск скачивает закреплённый дистрибутив и зависимости сборки.

Источники:

- [Homebrew: Android Studio](https://formulae.brew.sh/cask/android-studio).
- [Совместимость AGP 8.9](https://developer.android.com/build/releases/agp-8-9-0-release-notes).
- [Compose Compiler plugin](https://developer.android.com/develop/ui/compose/setup-compose-dependencies-and-compiler).
- [Gradle Wrapper](https://docs.gradle.org/8.11.1/userguide/gradle_wrapper.html).
- [Официальные checksum Gradle](https://services.gradle.org/distributions/).
- [Файлы wrapper в официальном репозитории](https://github.com/gradle/gradle/tree/v8.11.1/gradle/wrapper).
- [Сеть Android Emulator](https://developer.android.com/studio/run/emulator-networking-address).

## Границы проверки

Успешная сборка APK подтверждает компиляцию и упаковку. Запуск интерфейса и соединение с API на эмуляторе или физическом Android требуют отдельной проверки на устройстве. Состояние фактически выполненных проверок фиксируется в отчёте текущей задачи.
