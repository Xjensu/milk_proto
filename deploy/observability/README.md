# Local observability overlay

This overlay provides the local monitoring path selected in the platform roadmap:

`Rails /metrics → Prometheus → Grafana`

and

`Docker JSON logs or Fluent Forward → Fluentd → Loki → Grafana`.

Grafana credentials are supplied through the uncommitted `deploy/observability/.env.local` file. Create it from the committed template before the first start:

```powershell
Copy-Item deploy/observability/.env.example deploy/observability/.env.local
```

Replace `GF_SECURITY_ADMIN_PASSWORD` with a long random value and keep the file private. The local Grafana username is `admin`; Prometheus and Loki expose local operator endpoints without a separate login in this development overlay.

Start it from the repository root after the application services are available:

```powershell
docker compose -f docker-compose.yml -f deploy/observability/docker-compose.observability.yml --profile observability up -d --build
```

Open Grafana at `http://localhost:3001`, Prometheus at `http://localhost:9090`, and Loki at `http://localhost:3100/ready`. Grafana provisions the Prometheus and Loki data sources and the `Milk Schedule · Operations` dashboard automatically. If Grafana was already initialized, changing the environment variable does not rotate its stored password automatically; run `docker exec milk_schedule_grafana grafana cli admin reset-admin-password "$env:GF_SECURITY_ADMIN_PASSWORD"` after loading the same value into your shell, or use Grafana's password-reset procedure.

Prometheus also loads the committed alert rules in `prometheus/alerts.yml`. `ScheduleWebDown` fires after two minutes without a Rails scrape; `ScheduleWebCircuitBreakerOpen` fires after five minutes with any upstream breaker open. Review the alert details in Prometheus (`/alerts`) and the corresponding breaker key in Grafana before retrying an administrator mutation.

The Prometheus target is the internal Compose address `schedule_web:3000/metrics`. The dashboard uses the bounded request counters, duration sums/counts and circuit-breaker gauges already exposed by Rails; it does not use request IDs, URLs or user data as metric labels.

Fluentd reads Docker JSON-file logs from the mounted Docker container directory and also accepts Fluent Forward traffic on port `24224`. On Docker Desktop, verify that the Docker data directory can be mounted by the engine. If that mount is unavailable in a deployment environment, route the service logger to Fluent Forward instead of weakening application logging or exposing the Docker socket.

Stop only the overlay with:

```powershell
docker compose -f docker-compose.yml -f deploy/observability/docker-compose.observability.yml --profile observability down
```

The named `prometheus-data`, `loki-data` and `grafana-data` volumes preserve history and dashboard state. Remove them only as an intentional local reset.

The overlay clears inherited HTTP proxy variables inside Prometheus, Loki, Fluentd and Grafana because these containers communicate over the Docker network. The current image tags are pinned to the locally verified Prometheus 3.3.0 and Grafana 11.6.0 images; update them deliberately when the deployment registry is reachable and repeat the health checks.
