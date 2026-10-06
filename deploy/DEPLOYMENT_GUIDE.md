# Deploy from your development PC to a separate server

Your current PC is the development and image-build machine. The application runs on a different Linux server. All commands below identify the machine on which they must run.

The server's IP does not need to be entered in `.env`. Nginx listens on all server interfaces at port 80, and your router forwards traffic to that host. Deployment detects the server's private IPv4 address and LAN subnet automatically.

## 1. What happens on each machine

| Development PC | Deployment server |
| --- | --- |
| Contains application source and independent application Git repositories | Contains the deployment bundle and private configuration |
| Builds Docker images | Pulls published Docker images |
| Pushes images to Docker Hub | Runs Compose and database migrations |
| Packages matching deployment support files | Stores PostgreSQL, MinIO, Redis and Consul data |
| Does not need the server's address in `.env` | Detects its own network when deployment runs |

Use `compose.build.yml` on the development PC. Use `docker-compose.yml` on the server through the supplied scripts. The build commands do not start or restart your development containers.

Each release consists of custom Docker images with one shared tag plus a matching archive. The archive contains Compose, scripts, configuration templates, schemas and the shared discovery adapters required by the current runtime. It excludes application sources, private environment files, generated credentials and local stored data.

## 2. One-time preparation on the development PC

Required: Docker with the Compose plugin, Python 3, Git and a POSIX shell. On Windows, run the shell commands through WSL.

Use the existing project directory:

```bash
cd /home/xjensu/Desktop/milk_schedule
```

The build machine needs the compatible sources under `apps/schedule_web`, `apps/api_geteway`, `apps/excel_processor` and `apps/scheduler`. The web, API and Excel directories are independently versioned repositories. `apps/frontend` is retained for the optional legacy frontend and is also inspected by the packaging script. A root checkout alone does not supply all these source directories. Keep this complete development workspace for building releases.

Create the build settings file only if it is absent:

```bash
[ -f .env ] || cp .env.compose.example .env
nano .env
```

Keep `IMAGE_NAMESPACE=xjensu`. Leave `ADMIN_INTERFACE` and `ADMIN_LAN_CIDRS` empty for the normal router/LAN setup; no `LAN_BIND_IP` entry is needed. An existing local `.env.local` remains development configuration and is not transferred to the server.

Sign in to Docker Hub:

```bash
docker login --username xjensu
```

Enter your Docker Hub access token at the prompt. Create the image repositories under your account and choose whether they are public or private.

| Required repository | Purpose |
| --- | --- |
| `xjensu/table-bsut-by` | Rails web interface |
| `xjensu/table-api` | Hanami API and its background workers/operations console |
| `xjensu/excel-processor` | Excel processing |
| `xjensu/table-scheduler` | Scheduling solver |
| `xjensu/table-discovery` | Discovery registrars and ACL initialization |
| `xjensu/table-minio-init` | Storage bucket initialization |
| `xjensu/table-edge` | Nginx ingress |

Optional repositories are `xjensu/table-fluentd` for monitoring and `xjensu/table-frontend` for the legacy frontend. The normal deployment does not enable these profiles.

Build for the server's architecture. If both machines are Linux amd64, no architecture override is needed. If the build machine differs from the amd64 server, set `DOCKER_DEFAULT_PLATFORM=linux/amd64` before building. ARM support for the current pinned dependencies has not been verified.

## 3. Build and publish a release — development PC

Choose a new tag for every release, for example `2026-10-06-1`. Do not overwrite a tag already used by the server.

```bash
./deploy/bin/build-images.sh 2026-10-06-1 --push
python3 deploy/bin/package-release.py 2026-10-06-1
```

The first command builds and pushes all seven required custom images. Without `--push`, it only builds. The second command creates:

```text
dist/milk-schedule-2026-10-06-1.tar.gz
```

For example, the web and Excel images are published as:

```text
xjensu/table-bsut-by:2026-10-06-1
xjensu/excel-processor:2026-10-06-1
```

The packaging script puts the same tag into the archive's `.env.compose.example`. Build and package without changing the source worktrees between these steps. `RELEASE.json` records source revisions and dirty-worktree flags; commit application changes in their owning repositories when you need a reproducible release.

If using another registry namespace, set `IMAGE_NAMESPACE` in the development PC's `.env`, and pass that same namespace with `package-release.py --namespace YOUR_NAMESPACE`.

## 4. One-time preparation — deployment server

Use a Linux server with Docker Engine, the Compose plugin, Python 3 and `iproute2` (the `ip` command). The deployment account must be able to run Docker. The firewall helper requires Docker's iptables backend and its `DOCKER-USER` chain; iptables-nft is supported. Rootless Docker and a native nftables-only backend are outside this setup.

Connect using the server's SSH hostname or address. `SERVER` below is a placeholder for that SSH destination; it is not a value you must add to `.env`.

```bash
ssh deploy@SERVER
```

Create a stable installation directory. Replace `deploy` with your actual deployment account:

```bash
sudo install -d -m 750 -o deploy -g deploy /opt/milk-schedule
```

Your router continues forwarding website traffic to this server on TCP port 80. Ensure port 80 is available on the server. A hostname can point to the router's external address; application IP detection does not configure DNS or the router.

For private Docker Hub images, log in on the server with an account token that has pull access:

```bash
docker login --username xjensu
```

## 5. Transfer and configure the release

**On the development PC:**

```bash
scp dist/milk-schedule-2026-10-06-1.tar.gz \
  deploy@SERVER:/opt/milk-schedule/
```

**On the deployment server:**

```bash
cd /opt/milk-schedule
tar -xzf milk-schedule-2026-10-06-1.tar.gz
python3 deploy/bin/init-env.py
```

`init-env.py` creates private files with random secrets and mode 600. It never overwrites an existing file. On the new server it generates new server credentials; it does not use this development PC's passwords.

### `.env` — release and container settings

Edit it on the server:

```bash
nano .env
```

For the normal deployment, the essential values are:

```dotenv
COMPOSE_PROJECT_NAME=milk_schedule
IMAGE_NAMESPACE=xjensu
IMAGE_TAG=2026-10-06-1
ADMIN_INTERFACE=
ADMIN_LAN_CIDRS=
HTTP_BIND_IP=0.0.0.0
HTTP_PORT=80
WEB_REPLICAS=2
```

No host IP entry is required. The blank network overrides enable automatic private-address/subnet detection. On a host with multiple LAN interfaces, preflight prefers the normal outbound route. Use `ADMIN_INTERFACE` only if you deliberately need a particular interface, such as a VPN.

Keep `COMPOSE_PROJECT_NAME` unchanged across updates because it determines persistent volume names. `HTTP_BIND_IP=0.0.0.0` means all server interfaces; it is not your server's actual address.

Upstream PostgreSQL, Redis, MinIO and Consul images are configured separately through `POSTGRES_IMAGE`, `REDIS_IMAGE`, `MINIO_IMAGE` and `CONSUL_IMAGE`. Leave these values unchanged unless performing an intentional infrastructure update.

### `.env.local` — server secrets and application settings

Open it privately on the server:

```bash
nano .env.local
```

Generated passwords/signing keys can remain as created. Review the bootstrap administrator email and the website origin:

```dotenv
SEED_ADMIN_EMAIL=admin@your-organization.example
CORS_ORIGINS=http://your-website.example
```

These are examples: use your actual email and browser URL, without a trailing slash in the origin. A website hostname is not a machine IP configuration. Use `https://` only if HTTPS has actually been configured.

| Values | Purpose |
| --- | --- |
| `POSTGRES_DB`, `POSTGRES_USER`, `POSTGRES_PASSWORD` | Database identity and credentials |
| `DATABASE_URL` | Must match the PostgreSQL credentials/database |
| `REDIS_PASSWORD`, `REDIS_URL` | Matching Redis authentication settings |
| `MINIO_ACCESS_KEY`, `MINIO_SECRET_KEY` | Application storage credentials |
| `MINIO_ROOT_USER`, `MINIO_ROOT_PASSWORD` | MinIO server credentials; template derives them from the application credentials |
| `JWT_SECRET`, `SECRET_KEY_BASE` | Authentication and session signing |
| `SEED_ADMIN_EMAIL`, `SEED_ADMIN_PASSWORD` | Initial application administrator |
| Bucket names, timeouts, worker and cache options | Runtime tuning |

Keep container hostnames such as `db`, `redis` and `minio`; do not replace them with the server's IP or `localhost`. Consul resolves the actual internal endpoints.

The template uses `${VARIABLE}` references to keep connection settings consistent. Newly generated passwords are hexadecimal, which is safe in those URLs. Existing passwords with URL-reserved characters require correctly encoded connection URLs. Single-quote literal dotenv values containing `$` when interpolation is unwanted.

The current ingress serves HTTP. Keep `ASSUME_SSL=false` and `FORCE_SSL=false` for that setup. These variables do not install certificates or configure HTTPS.

### Other private settings

`deploy/ops/.env.local` holds separate credentials for `/ops/`. `deploy/observability/.env.local` holds optional Grafana credentials. The application administrator, database account and operations-console account are separate identities.

Do not copy development `.env.local`, `storage/` or `deploy/generated/` to a new server. Each server generates its own networking and discovery credentials. Preserve and privately back up the server's generated configuration once the server is installed.

## 6. First startup — deployment server only

From `/opt/milk-schedule`:

```bash
./deploy/bin/deploy-discovery.sh
```

This detects the server network, validates Compose, pulls the images, starts infrastructure/discovery, runs database migrations, starts application services, installs the project firewall and waits for health checks. It does not build images.

For a new installation, seed reference data and the initial administrator:

```bash
./deploy/bin/db.sh seed
./deploy/bin/compose.sh ps
./deploy/bin/compose.sh images
```

The seed uses the server's `SEED_ADMIN_EMAIL` and `SEED_ADMIN_PASSWORD`. Change the bootstrap password after first sign-in. Seeding does not reset an existing administrator's password.

Open the public website through your router/hostname. Administrator sign-in and `/ops/` are restricted to the detected LAN by default. Port forwarding alone does not grant external administrator access. Use that LAN or a deliberately configured VPN for administration; HTTP itself does not encrypt credentials.

Verify public schedules, administrator sign-in, an edit/reload, an Excel import and a notice download. Healthy containers do not prove every application workflow.

The current notice download links point to MinIO on the detected private address at port 9000. Forwarding only website port 80 does not make that storage address reachable to external clients. LAN/VPN downloads can use the default. If external downloads are required, provide an appropriately secured storage endpoint and configure `NOTICE_EXPORT_PUBLIC_ENDPOINT`; that is a separate network/proxy arrangement.

## 7. Publish an application update

**On the development PC:** build a new tag, package it and transfer it:

```bash
./deploy/bin/build-images.sh 2026-10-07-1 --push
python3 deploy/bin/package-release.py 2026-10-07-1
scp dist/milk-schedule-2026-10-07-1.tar.gz \
  deploy@SERVER:/opt/milk-schedule/
```

**On the deployment server:** back up first, extract the bundle, select the tag and deploy:

```bash
cd /opt/milk-schedule
tar -xzf milk-schedule-2026-10-07-1.tar.gz
nano .env
# Set IMAGE_TAG=2026-10-07-1 in the editor.
./deploy/bin/deploy-discovery.sh
```

The archive updates templates and deployment files, not the existing private environment files or data. Compare new templates with your current configuration and add new options deliberately. Do not copy `.env.example` over the installed `.env.local`.

Database migrations run during deployment, so review schema changes and have a verified backup. Hanami `db prepare` also runs seeds; the supplied deployment script intentionally uses `db migrate`. The Excel worker applies its own migrations when starting.

## 8. Change server configuration without rebuilding

Edit the appropriate file **on the server**, then recreate services:

```bash
cd /opt/milk-schedule
nano .env.local
./deploy/bin/compose.sh config --quiet
./deploy/bin/compose.sh up -d --no-build
python3 deploy/bin/install-project-firewall.py
```

A restart alone does not apply edited environment values. Use `config --quiet` to validate without printing expanded secrets. Clear stale exported shell overrides such as `IMAGE_TAG` when you intend to use file values.

If the server's LAN address changes, no IP edit in `.env` is needed. Refresh detection and apply the resulting configuration:

```bash
python3 deploy/bin/prepare-discovery.py
./deploy/bin/compose.sh up -d --no-build
./deploy/bin/compose.sh exec -T nginx nginx -t
./deploy/bin/compose.sh exec -T nginx nginx -s reload
python3 deploy/bin/install-project-firewall.py
```

Reloading Nginx applies the regenerated administrator subnet policy. Ensure the router's forwarding rule still targets the server's current address. The network is detected at deployment/preflight, not continuously in the background.

Changing `POSTGRES_PASSWORD` in a file does not change the stored PostgreSQL role password. Rotate the database credential and dependent services together. Changing signing secrets invalidates authentication/session data. Grafana stores its initialized administrator password in its own database; later rotation uses Grafana's password-change/reset process.

## 9. Backups, stopping and rollback

Back up PostgreSQL, MinIO objects, Redis files, Consul data/tokens and any monitoring volumes. Keep private copies of the server's `.env`, `.env.local`, component credential files and `deploy/generated/`. Keep older release bundles and image tags.

Example PostgreSQL logical backup **on the server**:

```bash
cd /opt/milk-schedule
umask 077
mkdir -p backups
./deploy/bin/compose.sh exec -T db sh -c \
  'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Fc' \
  > "backups/postgres-$(date +%Y%m%d-%H%M%S).dump"
```

This covers PostgreSQL only. Back up MinIO and other state separately. Use consistent snapshots or stop writers for filesystem-level volume backups. Store backups separately from the installation and verify restoration.

Stop services without removing data:

```bash
./deploy/bin/compose.sh stop
```

For an application rollback, restore the earlier matching bundle, select its tag in `.env`, pull and recreate compatible services, then reapply the firewall. An image rollback does not undo database migrations; confirm schema compatibility or use a planned database restore. Never use `down -v` or volume pruning during routine updates.

## 10. Diagnostics and optional monitoring

Run diagnostics **on the server**:

```bash
./deploy/bin/compose.sh ps -a
./deploy/bin/compose.sh logs --tail=100 api_geteway
./deploy/bin/compose.sh logs --tail=100 discovery_acl
./deploy/bin/compose.sh logs --tail=100 excel_processor
./deploy/bin/db.sh version
./deploy/bin/compose.sh exec -T nginx nginx -t
```

| Symptom | What to check |
| --- | --- |
| Pull denied / tag missing | All seven images were pushed; namespace, tag and Docker Hub access are correct. |
| `exec format error` | Image architecture matches the server. |
| Automatic address detection fails | Host IPv4/default route; use `ADMIN_INTERFACE` for unusual routing. |
| `/admin/` or `/ops/` returns 403 | Access from the detected LAN/VPN; router forwarding does not bypass the allowlist. |
| `/ops/` returns 401 | Its separate credentials; recreate the console after changing them. |
| Missing mounted configuration | Complete matching archive was extracted and preflight ran. |
| Website works, external file download fails | MinIO's private port-9000 endpoint is not reachable through website port forwarding alone. |
| Environment edit has no effect | Recreate services and check shell overrides. |
| Firewall helper fails | Docker iptables backend and `DOCKER-USER` chain. |

For monitoring, first build/push `table-fluentd` on the development PC for the same tag, then enable the server overlay as described in [observability/README.md](observability/README.md). Its host ports bind to loopback; access Grafana through an SSH tunnel.

The deployment scripts do not configure DNS, router rules, cloud security groups, TLS or automatic firewall restoration after host restarts. Reapply `install-project-firewall.py` after a host/firewall restart or container replacement.

References: [Compose image builds](https://docs.docker.com/reference/compose-file/build/), [environment interpolation](https://docs.docker.com/compose/how-tos/environment-variables/variable-interpolation/), [environment precedence](https://docs.docker.com/compose/how-tos/environment-variables/envvars-precedence/).
