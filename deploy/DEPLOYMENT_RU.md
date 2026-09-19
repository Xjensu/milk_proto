# Развёртывание Milk Schedule через Docker Compose

Этот файл описывает локальное развёртывание проекта на Linux. Compose запускает Rails интерфейс `apps/schedule_web` на порту `3000`, Hanami API `apps/api_geteway` на порту `2300`, отдельный frontend `apps/frontend` на порту `4173`, а также PostgreSQL/PostGIS, Redis, MinIO, Outbox relay, media sanitizer и Excel processor. MinIO загружается из `quay.io`, потому что Docker Hub не отдаёт используемые репозитории `minio/minio` и `minio/mc`.

## 1. Требования

Проверьте установленные инструменты:

```bash
ruby --version
bundle --version
docker --version
docker compose version
```

Основной запуск выполняется внутри Docker, поэтому Ruby на хосте нужен прежде всего для локальных диагностических команд. Dockerfile проекта использует `ruby:3.4.2`.

Пользователь должен иметь доступ к Docker socket. После добавления в группу `docker` начните новый login shell или полностью выйдите из графической сессии и войдите снова:

```bash
sudo usermod -aG docker "$USER"
newgrp docker
docker info
```

Если `newgrp` отсутствует, выполните logout/login и затем повторите `docker info`. Если Docker остановлен:

```bash
sudo systemctl enable --now docker
```

## 2. Конфигурация

Перейдите в корень проекта:

```bash
cd /path/to/milk_schedule
```

Для нового окружения создайте локальную конфигурацию:

```bash
cp .env.example .env.local
```

`.env.local` не добавляйте в Git. Задайте уникальные значения для `POSTGRES_PASSWORD`, `REDIS_PASSWORD`, `JWT_SECRET` и `SECRET_KEY_BASE`. Для генерации случайного значения:

```bash
openssl rand -hex 32
```

В `DATABASE_URL` должны совпадать пользователь, пароль, база и хост из `POSTGRES_*`. Внутри Compose хосты имеют имена `db`, `redis` и `minio`; `localhost` между контейнерами использовать нельзя.

Для первого администратора задайте значения только на время seed-команды. Не сохраняйте пароль в документации, Git или логах:

```bash
export SEED_ADMIN_EMAIL='admin@example.local'
export SEED_ADMIN_PASSWORD='replace-with-a-long-unique-password'
```

## 3. Сборка и запуск

Проверьте итоговую конфигурацию Compose:

```bash
docker compose config >/tmp/milk-schedule-compose.yml
```

Сначала соберите локальные образы, затем запустите сервисы. Раздельные команды нужны потому, что `outbox_relay` использует локальный API image:

```bash
docker compose build
docker compose up -d
```

Проверьте состояние и логи:

```bash
docker compose ps
docker compose logs --tail=100 db api_geteway schedule_web frontend
```

## 4. Миграции и seeds

После первого запуска подготовьте схему API:

```bash
docker compose exec -T api_geteway bundle exec hanami db prepare
```

Выполните idempotent seed. Повторный запуск не должен дублировать bootstrap account, time slots и week types:

```bash
docker compose exec -T api_geteway bundle exec hanami db seed
```

Если для администратора были заданы временные переменные, передайте их в контейнер:

```bash
docker compose exec \
  -e SEED_ADMIN_EMAIL="$SEED_ADMIN_EMAIL" \
  -e SEED_ADMIN_PASSWORD="$SEED_ADMIN_PASSWORD" \
  -T api_geteway bundle exec hanami db seed
```

Сразу после первого входа смените пароль администратора штатным UI/API потоком. Не используйте встроенный пароль по умолчанию.

После применения миграций перезапустите API, чтобы ROM заново загрузил схему:

```bash
docker compose restart api_geteway outbox_relay media_sanitizer excel_processor schedule_web
```

## 5. Проверка работоспособности

Выполните проверки:

```bash
# У API нет публичного health route; Compose проверяет TCP-порт 2300.
timeout 5 bash -c '</dev/tcp/127.0.0.1/2300'
curl -fsS http://localhost:3000/up
curl -fsS http://localhost:3000/readyz
docker compose exec -T schedule_web bundle exec rails zeitwerk:check
```

Откройте `http://localhost:3000` или `http://localhost:4173` и войдите созданной учётной записью. Для проверки данных пройдите основной сценарий администратора: выбрать группу и семестр, создать workload, назначить преподавателя, разместить занятие, изменить его, обновить страницу и убедиться, что результат прочитан из API.

## 6. Обновление проекта

После изменения исходников пересоберите образы. Один `restart` не переносит исходники в уже собранный образ:

```bash
docker compose build api_geteway schedule_web frontend excel_processor
docker compose up -d --force-recreate api_geteway schedule_web frontend excel_processor
docker compose exec -T api_geteway bundle exec hanami db prepare
docker compose restart api_geteway schedule_web
```

Если изменялись миграции, соблюдайте порядок `build -> db prepare -> restart API`.

## 7. Остановка и резервное копирование

```bash
docker compose stop
docker compose down
```

Обычный `down` не удаляет данные. Не используйте `docker compose down -v`, если не хотите удалить PostgreSQL и MinIO volumes. Перед переносом данных сделайте резервную копию:

```bash
docker compose exec -T db pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" > backup.sql
```

Для production замените значения по умолчанию, не публикуйте PostgreSQL/Redis/MinIO наружу, поставьте TLS reverse proxy и настройте регулярные backups. Текущий Compose предназначен прежде всего для восстановления и локального запуска на одной машине.

## 8. Если отсутствуют исходники

Для текущего Compose необходимы каталоги `apps/frontend`, `apps/schedule_web`, `apps/api_geteway`, `apps/excel_processor` и `schemas`. Если какой-либо каталог не был перенесён, скачайте его из соответствующего репозитория проекта в указанное место. Не заменяйте отсутствующий API mock-сервисом: Rails зависит от реальных API контрактов и авторизации.
