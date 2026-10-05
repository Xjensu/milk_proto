# Build locally, deploy prebuilt images

## 1. Deployment model

Build and publish on your PC. The remote Linux server only pulls images and runs Compose. Runtime configuration remains in private environment files on that server; changing these files does not require rebuilding images.

| Image repository | Services using it |
| --- | --- |
| `xjensu/table-bsut-by` | Rails `schedule_web` replicas |
| `xjensu/table-api` | `api_geteway`, `ops_console`, outbox, media sanitizer, notice export and schedule generation workers |
| `xjensu/excel-processor` | Excel worker |
| `xjensu/table-scheduler` | Scheduling solver |
| `xjensu/table-discovery` | Consul ACL initializer and PostgreSQL/Redis/MinIO registrars |
| `xjensu/table-minio-init` | Private storage bucket initializer |
| `xjensu/table-edge` | Nginx ingress and its discovery supervisor |
| `xjensu/table-fluentd` | Optional monitoring profile |
| `xjensu/table-frontend` | Optional legacy frontend profile; not the active website |

All custom images in a release use the same tag, for example `2026-10-05-1`. Treat published release tags as immutable: publish a new tag for each update. PostgreSQL, Redis, MinIO and Consul use their upstream images, configured separately in `.env`.

The existing applications load shared discovery adapters and schemas from read-only mounts. Therefore, the server also needs a small deployment bundle containing these files, Compose, scripts, monitoring configuration and templates. The packaging script includes an explicit file allowlist; it excludes application sources, `.env.local`, generated Consul tokens, database/storage data and AI tooling. Transfer the bundle made from the same worktree state as the images.

## 2. Prepare your build PC

Install Docker Engine or Docker Desktop with the Compose plugin and Buildx, Python 3 and Git. Use Bash/WSL on Windows for the shell scripts. Keep compatible sources in:

```
apps/schedule_web
apps/api_geteway
apps/excel_processor
apps/scheduler
apps/frontend          # only needed for the optional legacy build
```

The web, API, Excel and legacy frontend directories have independent Git repositories. Commit each application change in its own repository before a release; a commit in the root repository does not include their source. The scheduler currently exists as local root-worktree source. Package metadata records repository revisions and whether each worktree is dirty; it does not make uncommitted application work reproducible.

From the project root, create configuration if it does not already exist:

```sh
python3 deploy/bin/init-env.py
```

This generates random secrets directly into mode-600 files and never overwrites existing files. For builds, only `.env` is read. Set `IMAGE_NAMESPACE=xjensu` there. Server networking is not needed for building.

Check the server architecture with `uname -m`: `x86_64` means `linux/amd64`; `aarch64` means `linux/arm64`. Build for the server's architecture. The current pinned dependencies must support that architecture; this change was verified on Linux amd64, not ARM.

```sh
# When your PC and server use different architectures:
export DOCKER_DEFAULT_PLATFORM=linux/amd64

docker login --username xjensu
./deploy/bin/build-images.sh 2026-10-05-1 --push
python3 deploy/bin/package-release.py 2026-10-05-1
```

`docker login` asks for your Docker Hub access token interactively. Create the corresponding repositories under your Docker Hub account and choose their visibility before publishing. The build script builds the seven required images and pushes them; it does not start the application or use the server's private runtime files. Without `--push`, it only builds.

Equivalent individual commands, after setting the tag:

```sh
export IMAGE_TAG=2026-10-05-1
docker compose --env-file .env -f compose.build.yml build --pull

docker push xjensu/table-bsut-by:2026-10-05-1
docker push xjensu/table-api:2026-10-05-1
docker push xjensu/excel-processor:2026-10-05-1
docker push xjensu/table-scheduler:2026-10-05-1
docker push xjensu/table-discovery:2026-10-05-1
docker push xjensu/table-minio-init:2026-10-05-1
docker push xjensu/table-edge:2026-10-05-1
unset IMAGE_TAG
```

For optional monitoring, also build and publish Fluentd for that tag:

```sh
IMAGE_TAG=2026-10-05-1 docker compose --env-file .env -f compose.build.yml \
  --profile observability build --pull fluentd
IMAGE_TAG=2026-10-05-1 docker compose --env-file .env -f compose.build.yml \
  --profile observability push fluentd
```

The optional legacy build uses `--profile legacy-frontend` and service `frontend`. Normally leave it disabled.

If you choose a different registry namespace, set it in `.env` for builds and pass the same value to `package-release.py --namespace YOUR_NAMESPACE`.

## 3. Prepare the remote server

Use a Linux server with Docker Engine, the Docker Compose plugin, Python 3 and `iproute2` (`ip`). The firewall helper requires Docker's iptables backend with a `DOCKER-USER` chain; iptables-nft is supported. Docker rootless/native nftables-only deployments are outside this setup. Docker permissions are required for the deployment account.

Choose a persistent installation directory, such as `/opt/milk-schedule`. Have the server administrator create it and make your deployment account its owner. Keep the same path and `COMPOSE_PROJECT_NAME` across updates. The latter controls volume identity.

Transfer the bundle from your PC:

```sh
scp dist/milk-schedule-2026-10-05-1.tar.gz deploy@SERVER:/opt/milk-schedule/
ssh deploy@SERVER
cd /opt/milk-schedule
tar -xzf milk-schedule-2026-10-05-1.tar.gz
python3 deploy/bin/init-env.py
chmod 600 .env .env.local deploy/ops/.env.local deploy/observability/.env.local
```

There is no `apps/` directory in this bundle, and no build runs on the server. For private Docker Hub repositories, run `docker login --username xjensu` on the server using a read-only token.

## 4. Edit the environment files

### `.env`: release, network and container configuration

Open the file in an editor, for example `nano .env`.

| Setting | Meaning |
| --- | --- |
| `COMPOSE_PROJECT_NAME` | Keep `milk_schedule`, especially when reusing existing volumes. |
| `IMAGE_NAMESPACE` | Docker Hub account/registry prefix, normally `xjensu`. |
| `IMAGE_TAG` | Published release tag; the bundle's template contains its tag. |
| `ADMIN_INTERFACE` | Optional server interface, e.g. `enp1s0` or `wg0`. |
| `LAN_BIND_IP` | An IPv4 address actually assigned to the server. MinIO binds to it. Blank enables private LAN detection. |
| `ADMIN_LAN_CIDRS` | Trusted administrator client source networks, comma-separated. Blank uses the detected private LAN subnet. |
| `HTTP_BIND_IP`, `HTTP_PORT` | Website binding; defaults to `0.0.0.0:80`. |
| `WEB_REPLICAS` | Rails container count, default `2`. |
| `NOTICE_EXPORT_PUBLIC_ENDPOINT` | Optional object-store address reachable by administrators' browsers; otherwise `http://LAN_BIND_IP:9000`. |
| `POSTGRES_IMAGE`, `REDIS_IMAGE`, `MINIO_IMAGE`, `CONSUL_IMAGE` | Infrastructure versions, independent of your application tag. |

Example for a private network:

```dotenv
ADMIN_INTERFACE=enp1s0
LAN_BIND_IP=192.168.10.20
ADMIN_LAN_CIDRS=192.168.10.0/24
```

For a cloud server, prefer a private/VPN interface for administration. An explicit public `LAN_BIND_IP` is also supported, but you must set `ADMIN_LAN_CIDRS` to the trusted client source IPs/subnets. The client CIDRs need not contain the server address. Never set `0.0.0.0/0`. Do not copy the example addresses literally.

The current ingress serves HTTP and restricts login, administrator routes and `/ops/` by source address. It does not terminate TLS. Use a trusted encrypted network for account access until HTTPS is configured. Putting a reverse proxy/CDN in front requires deliberate forwarded-header and source-address policy changes; changing only `ASSUME_SSL` does not add HTTPS.

### `.env.local`: runtime credentials and application options

`init-env.py` fills blank required secrets with random values for a new installation. On an existing installation, retain your existing database name, user and all credentials. Cleanup does not rotate them.

- `POSTGRES_DB`, `POSTGRES_USER`, `POSTGRES_PASSWORD`: database identity. `DATABASE_URL` must match them.
- `REDIS_PASSWORD`: Redis authentication. `REDIS_URL` must match it.
- `MINIO_ACCESS_KEY`, `MINIO_SECRET_KEY`: application storage credentials. The template derives `MINIO_ROOT_USER` and `MINIO_ROOT_PASSWORD` from these for the MinIO server.
- `JWT_SECRET`, `SECRET_KEY_BASE`: signing secrets. Changing them invalidates authentication/session data.
- `SEED_ADMIN_EMAIL`, `SEED_ADMIN_PASSWORD`: initial administrator identity. Seeding does not reset an existing account's password.
- `CORS_ORIGINS`: the actual browser origin, such as `http://192.168.10.20`, without a trailing slash; multiple origins are comma-separated.
- Bucket names and runtime tuning can usually keep their template values.
- `ASSUME_SSL=false` and `FORCE_SSL=false` match the current HTTP ingress. `WEB_CONCURRENCY` controls worker processes inside each Rails container.

Compose expands `${VARIABLE}` references in this runtime template. Newly generated passwords are hexadecimal so the connection URLs remain valid. For an existing password with URL-reserved characters, retain or construct the correctly URL-encoded `DATABASE_URL`/`REDIS_URL`; do not simply insert raw characters into a URL. Single-quote literal dotenv values containing `$` to prevent accidental interpolation.

The runtime file intentionally retains fallback connection URLs because application settings require them. Consul supplies actual internal endpoints when discovery is enabled. Do not set container hosts to `localhost`.

### Other private files

`deploy/ops/.env.local` contains `OPS_CONSOLE_USERNAME` and `OPS_CONSOLE_PASSWORD`, separate from the application administrator. `deploy/observability/.env.local` contains Grafana credentials and is only used by the monitoring profile.

`deploy/generated/` contains host-specific network policy and Consul tokens. The preflight creates it with restricted permissions, keeps existing tokens and preserves the allocated discovery subnet. Back it up with Consul data. Do not copy it to a different server or delete it during an upgrade. Edit network inputs in `.env`, then rerun preflight; do not edit generated files manually.

`.env` is Compose input; `.env.local` is container runtime input. `deploy/bin/compose.sh` reads `.env` first and generated networking second. Shell environment variables take precedence, so clear old exported `IMAGE_TAG` or network overrides when using file values. Use `config --quiet`, not plain `config`, to avoid printing expanded secrets.

## 5. First deployment

After reviewing configuration, run:

```sh
./deploy/bin/deploy-discovery.sh
```

The script generates host policy, validates Compose, pulls images, starts infrastructure and discovery, runs `hanami db migrate`, starts the remaining services, installs the scoped ingress firewall and waits for Docker health checks. A failure stops the script. It does not run builds or seed data automatically.

For a new database, create the initial administrator and reference data:

```sh
./deploy/bin/db.sh seed
./deploy/bin/compose.sh ps
```

`db migrate` applies schema changes. For existing installations, back up before deployment and review release migrations first. The Excel worker applies its own migrations on startup. `db.sh` resolves the database URL through Consul before invoking Hanami, so the CLI and application use the same database identity. Hanami's separate `db prepare` command also runs seeds; routine deployment deliberately uses `migrate` instead.

Open `http://SERVER/` (include your port if not 80), then `/admin/` from an allowed client. Use the email and password from the server's private runtime file. Change the bootstrap password after first sign-in. `/ops/` uses its own credentials and provides only allowlisted database operations; it has no arbitrary shell or Docker socket.

Verify the actual release, not just container startup:

```sh
./deploy/bin/compose.sh images
./deploy/bin/compose.sh ps
curl --fail http://SERVER/readyz
```

The readiness URL is LAN-restricted. Check public schedules, administrator sign-in, a schedule edit/reload, an Excel import and a notice download. Health checks do not prove those complete workflows.

## 6. Update images and change settings

On the PC, build/push a new tag and create its matching bundle. On the server, back up, extract the new bundle over the existing installation, and edit the existing `.env` to select the new `IMAGE_TAG`. Extraction updates templates, not your private configuration or data. Compare the new `.env.example` with your runtime file and add any new keys deliberately.

```sh
cd /opt/milk-schedule
tar -xzf milk-schedule-2026-10-06-1.tar.gz
nano .env
./deploy/bin/deploy-discovery.sh
```

For configuration-only changes, rebuilding or pushing images is unnecessary:

```sh
nano .env.local
./deploy/bin/compose.sh config --quiet
./deploy/bin/compose.sh up -d --no-build
python3 deploy/bin/install-project-firewall.py
```

Compose recreates services whose environment changed. `docker compose restart` alone does not apply edited environment values. After changing network settings in `.env`, also run `python3 deploy/bin/prepare-discovery.py` before `up`. Reapply the project firewall after container replacement or host/firewall restarts.

Changing `POSTGRES_PASSWORD` in a file does not change the role password stored in an existing database. Rotate the database credential and dependent services together. Similarly, changing `POSTGRES_DB` does not rename an existing database. Grafana stores its administrator password in its persistent database after initialization; use Grafana's own password change/reset process for later rotation.

## 7. Backups and rollback

Persisted state includes PostgreSQL, MinIO objects, Redis files (`storage/redis_data`), Consul state/tokens and optional Grafana/Prometheus/Loki volumes. Preserve `.env`, `.env.local`, the two component credential files and `deploy/generated/` in a private backup. Keep previous release bundles and image tags.

Example PostgreSQL logical backup, without passwords in command arguments:

```sh
umask 077
mkdir -p backups
./deploy/bin/compose.sh exec -T db sh -c \
  'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Fc' \
  > "backups/postgres-$(date +%Y%m%d-%H%M%S).dump"
```

This backs up PostgreSQL only. Back up MinIO objects separately with the organization's storage backup procedure. For filesystem-level volume copies, stop the relevant writers/services or use storage-consistent snapshots; copying live database files is not a reliable backup. Keep backups outside the deployment directory and verify restoration on a separate installation.

For an application rollback, restore the previous bundle and select its original tag in `.env`, then pull/recreate compatible services. Do not automatically run old migrations over a newer schema. An image rollback does not undo migrations; confirm backward schema compatibility or perform a planned database restore.

```sh
./deploy/bin/compose.sh pull
./deploy/bin/compose.sh up -d --no-build
python3 deploy/bin/install-project-firewall.py
```

Use `compose.sh stop` to stop the application. `down` removes containers/networks but retains named volumes. Never use `down -v` or volume pruning as an update step. Keep the project name unchanged so Compose reuses the same volumes.

## 8. Monitoring and troubleshooting

See [monitoring instructions](observability/README.md) for the image-only optional overlay. Useful commands:

```sh
./deploy/bin/compose.sh ps -a
./deploy/bin/compose.sh logs --tail=100 api_geteway
./deploy/bin/compose.sh logs --tail=100 discovery_acl
./deploy/bin/compose.sh logs --tail=100 excel_processor
./deploy/bin/compose.sh exec nginx nginx -t
```

Review logs locally before sharing; never publish complete environment files, tokens or unredacted configuration.

| Symptom | Check |
| --- | --- |
| Image not found / pull denied | Tag exists for all seven images, correct namespace, Docker Hub permissions/login. |
| `exec format error` | Image architecture matches the server. |
| Missing bind-mount file | Extract the complete matching deployment bundle and run preflight. |
| Address detection fails | Set the assigned `LAN_BIND_IP` and correct interface/CIDRs in `.env`. |
| `/admin/` or `/ops/` returns 403 | Client source address belongs to the configured allowlist; no proxy is hiding it. |
| `/ops/` returns 401 | Its separate username/password; recreate the console after changes. |
| MinIO access denied | Root credentials and application credentials agree; retain the old values for existing data. |
| No healthy Consul endpoint | Check backing service, registrar, ACL initializer and tokens before restarting application workers. |
| Firewall helper fails | Docker iptables backend and `DOCKER-USER` chain; do not expose storage until rules apply. |
| New env value has no effect | Recreate the service, check shell overrides and Compose `environment:` entries. |

The deployment wrapper does not configure router rules, cloud security groups, DNS, TLS certificates or automatic firewall restoration at boot. Make the corresponding server arrangements and verify allowed/denied access from actual client networks.

## References

- [Docker Compose build and image tags](https://docs.docker.com/reference/compose-file/build/)
- [Compose interpolation and multiple environment files](https://docs.docker.com/compose/how-tos/environment-variables/variable-interpolation/)
- [Container environment precedence](https://docs.docker.com/compose/how-tos/environment-variables/envvars-precedence/)
