# Gateway Google: запуск на телефоне и ПК

Gateway — отдельный сервер, а не Android APK. Он хранит Google OAuth tokens и
обращается к Gmail/Calendar от имени владельца. Часы не должны получать OAuth
client secret или refresh token. Зависимости: Node.js **22.13+** со встроенным
`node:sqlite` и Python 3.9+ для импорта конфигурации. `npm install` не нужен.

**Текущее состояние:** сервер и сценарий авторизации проверены автоматическими
тестами с подставным OAuth-сервером. В прошивке ещё нет готового HTTPS-клиента
Gateway и вывода QR. Настройка ниже проверяет сервер отдельно; она пока не
включает Gmail/Calendar на часах. Поиск Google и Cloud Translation сервером
пока не реализованы. Переводчик на часах использует небольшой локальный словарь.

## Android / Termux — основной вариант

Установите Termux из [F-Droid](https://f-droid.org/en/packages/com.termux/)
или [официального GitHub](https://github.com/termux/termux-app#installation).
Приложение и его дополнения должны быть из одного источника.

В Termux:

```sh
pkg update
pkg install nodejs-lts python git
termux-setup-storage
git clone --branch Documentation https://github.com/Xopilot-AI-NN/Android-For-ESP32S3.git
cd Android-For-ESP32S3/GoogleAuthGateway
```

В Google Cloud выберите свой проект → **Google Auth Platform**:

1. **Branding**: заполните название приложения и контактный адрес.
2. **Audience**: для личной проверки External / Testing; добавьте свой аккаунт
   в Test users.
3. В **APIs & Services → Library** включите Google Calendar API и Gmail API,
   если будете проверять обе службы.
4. **Clients → Create client → Web application**. В Authorized redirect URIs
   добавьте ровно `http://localhost:8080/oauth/callback`. JavaScript origins
   для этого серверного потока не нужны. Скачайте JSON.

Импортируйте скачанный JSON, подставив его настоящее имя. Не перемещайте его
в репозиторий и не вставляйте содержимое в чат:

```sh
bash run.sh init "$HOME/storage/downloads/client_secret_ВАШ_ФАЙЛ.json"
bash run.sh check
bash run.sh start
```

Оставьте последнюю команду работающей. Во второй сессии Termux перейдите в ту же
папку и создайте тестовое устройство:

```sh
bash run.sh register 'Мои часы'
```

Команда выведет путь приватного JSON устройства. Используйте этот путь далее:

```sh
bash run.sh pair '/полный/путь/устройства.json' calendar,gmail
```

Откройте выданную одноразовую ссылку в браузере **этого же телефона**, выберите
аккаунт и разрешения. Затем выполните, подставив выданный Pairing ID:

```sh
bash run.sh poll '/полный/путь/устройства.json' PAIRING_ID
```

Проверьте адрес аккаунта. Только если он ваш, подтвердите:

```sh
bash run.sh confirm '/полный/путь/устройства.json' PAIRING_ID
```

Это ручная проверка серверного протокола. В готовой интеграции такое подтверждение
должны запрашивать сами часы. Ссылка живёт пять минут; после истечения создайте
новую командой `pair`. Команда `confirm` сохраняет отдельный приватный session JSON.
Через час сессию можно обновить командой `renew DEVICE_JSON`; отключить аккаунт —
`logout SESSION_JSON`. Файлы устройства и сессии содержат секреты.

Android может остановить Termux в фоне. Скрипт запрашивает wake lock, если команда
доступна; при необходимости разрешите Termux работу без ограничения батареи.
Для постоянной доступности сервер должен постоянно работать и иметь интернет.
Остановка: Ctrl+C; повторный запуск не требует повторного импорта JSON.

## Linux / macOS / Windows

На Linux/macOS установите Node.js 22.13+ и Python, затем используйте те же
`bash run.sh init`, `check`, `start` и команды устройства. Скачанный файл обычно
находится в Downloads или Загрузки; укажите его полный путь.

На Windows в PowerShell из этой папки:

```powershell
./run.ps1 init "$HOME/Downloads/client_secret_ВАШ_ФАЙЛ.json"
./run.ps1 check
./run.ps1 start
```

Другие команды аналогичны: `./run.ps1 register`, `pair`, `poll`, `confirm`,
`renew`, `logout`, `test`. Windows-сценарий предоставлен, но на Windows пока
не проверялся. На Windows ограничьте доступ к приватной папке средствами ACL:
POSIX-права 0600 сами по себе не обеспечивают эквивалентную защиту NTFS.

## Доступ с другого устройства

`localhost` всегда означает устройство, где открыт браузер. Если Gateway работает
на ПК, ссылка localhost с телефона ведёт на телефон. Для входа с другого устройства
нужен собственный **HTTPS-домен**, доступный браузеру и часам, и reverse proxy
с действительным сертификатом. Сервер слушает только loopback, порт 8080.

Пример Caddy на машине Gateway:

```caddyfile
gateway.example.org {
    reverse_proxy 127.0.0.1:8080
}
```

Это пример домена: замените его своим и настройте DNS/доступность 80 и 443.
В Google Console добавьте `https://ВАШ-ДОМЕН/oauth/callback`, снова скачайте JSON
и импортируйте:

```sh
bash run.sh init '/путь/к/новому/client_secret.json' --base-url https://ВАШ-ДОМЕН
bash run.sh start
```

Импорт сохраняет существующий ключ шифрования. Не подменяйте его вручную: старые
учётные данные станут нечитаемыми. HTTPS-прокси должен находиться на той же машине;
не выставляйте внутренний HTTP-порт в интернет. Для Termux с мобильным интернетом
доступ извне обычно требует отдельного HTTPS-туннеля или сервера; один IP телефона
не решает доступность. Настройка такого размещения зависит от вашего домена.

## Где личные данные и что защищает Git

- OAuth-конфигурация: `$XDG_CONFIG_HOME/aosp-wear-gateway/config.env`, по умолчанию
  `~/.config/aosp-wear-gateway/config.env`.
- База и приватные JSON: `$XDG_STATE_HOME/aosp-wear-gateway`, по умолчанию
  `~/.local/state/aosp-wear-gateway`.
- В Termux это внутренний HOME приложения. Не размещайте базу/конфигурацию
  в общей памяти `/sdcard` или в checkout проекта.
- На POSIX папки создаются с правами 0700, конфигурация и база — 0600.
  Токены аккаунта в SQLite шифруются AES-256-GCM; ключ хранится отдельно в config.
  Резервная копия базы вместе с ключом даёт доступ к аккаунту.
- Импорт и сервер запрещают размещать приватные данные внутри checkout.
  `.gitignore` исключает известные имена секретов, базы, логи и образы диска.
- Из корня проекта выполните `python3 tools/install_git_guard.py`, чтобы установить
  проверки pre-commit/pre-push в этом клоне. Проверка индекса:
  `python3 tools/check_public_files.py`. Hooks не устанавливаются автоматически
  при `git clone` и не заменяют проверку содержимого перед публикацией.

Не публикуйте OAuth JSON, session/device JSON, базу, логи с личной информацией,
фотографии и скриншоты. Если секрет уже попал в Git, удаление файла не удаляет
историю: отзовите секрет в Google Console и отдельно очистите историю.

Проверки сервера: `bash run.sh test`. Они используют только синтетические
учётные данные и не входят в ваш Google-аккаунт.

Документация Google: [создание credentials](https://developers.google.com/workspace/guides/create-credentials),
[OAuth web server flow](https://developers.google.com/identity/protocols/oauth2/web-server).
