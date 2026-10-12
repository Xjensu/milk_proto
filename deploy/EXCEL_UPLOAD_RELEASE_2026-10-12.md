# Excel upload release — 2026-10-12-1

Local verification on the published images: the actual ExcelImportsGateway succeeded in 6.20 seconds (longer than the normal five-second deadline). Rails multipart upload of a real XLSX returned HTTP 201 after ClamAV scanning and MinIO storage. Redis/outbox delivery reached the Excel worker; the diagnostic workbook intentionally used an unknown header and was recorded as failed with the expected unsupported-structure explanation. No curriculum data was imported. The file and storage object were removed afterward. This verifies upload and worker delivery, not a complete valid business import.

Authenticated production check on table.bsut.by: login redirected successfully, `/admin/excel_imports` returned 200, and the same upload returned HTTP 422 with the generic service-unavailable message after 5.34 seconds. A subsequent read found the diagnostic file in upload history, and its delete action returned 303. Thus the reported failure did not mean the upload was never committed. Do not repeatedly upload the same business workbook after this error without checking history.

The published Rails source allows 120 seconds for this single-attempt upload call; the normal upstream deadline defaults to 5 seconds. The production timing is consistent with an older image using the shorter deadline. Without production logs/image inspection, the exact exception remains unconfirmed. The API source also explicitly requires Shellwords before standalone ClamAV checks. No credential or cookie is included in this document.

## Existing server update

Run in the existing deployment directory on the Linux server. Preserve `.env.local`, `deploy/ops/.env.local`, data volumes and the existing Compose project name. Do not run `down -v`, regenerate secrets or seed during an ordinary upgrade. Take the normal database/volume backup before migration.

Fetch the infrastructure release branch or commit published with this release. For the `milk_proto` checkout:

```sh
git fetch origin codex/admin-workbench-2026-10-03
git merge --ff-only origin/codex/admin-workbench-2026-10-03
```

If your checkout uses a different remote name, use that name. A refused fast-forward requires reviewing local differences; do not force-reset server configuration. A deployment created from an archive can instead extract the matching new release archive over the deployment files; it contains no `.env.local` or private generated credentials.

Set `IMAGE_TAG=2026-10-12-1` in the existing `.env.local` without altering secrets. Update existing API_IMAGE/WEB_IMAGE overrides too; these override IMAGE_TAG:

```sh
python3 - <<'PY'
from pathlib import Path
import re
p = Path('.env.local')
s = p.read_text()
s, count = re.subn(r'^IMAGE_TAG=.*$', 'IMAGE_TAG=2026-10-12-1', s, flags=re.M)
if not count:
    s += '\nIMAGE_TAG=2026-10-12-1\n'
# Explicit per-service overrides take precedence over IMAGE_TAG.
for key, image in [('API_IMAGE', 'table-api'), ('WEB_IMAGE', 'table-bsut-by')]:
    s = re.sub(rf'^{key}=.*$', f'{key}=xjensu/{image}:2026-10-12-1', s, flags=re.M)
p.write_text(s)
PY
python3 deploy/bin/prepare-discovery.py
./deploy/bin/compose.sh config --quiet
./deploy/bin/compose.sh pull
./deploy/bin/compose.sh up -d --no-build \
  db redis minio consul discovery_acl \
  postgres_registrar redis_registrar minio_registrar minio_init scheduler
./deploy/bin/db.sh migrate
./deploy/bin/compose.sh up -d --no-build
python3 deploy/bin/install-project-firewall.py
./deploy/bin/compose.sh up -d --no-build --wait --wait-timeout 180
./deploy/bin/compose.sh images
./deploy/bin/compose.sh ps
```

Check that Rails, API, Excel worker and associated custom services show tag `2026-10-12-1`. The stock edge image enforces the repository LAN policy. If production uses a custom ingress that permits external administration, preserve/port its configuration before replacing edge. For a focused upload update, keep the existing ingress/infrastructure and recreate only the application services as described below. HTTPS publishing is not configured by this release. The preflight step matters after network changes: stale generated addresses can prevent MinIO binding or discovery.

ClamAV signatures must be available on production:

```sh
./deploy/bin/compose.sh logs --tail=80 clamav_signatures
./deploy/bin/compose.sh exec api_geteway clamscan --version
./deploy/bin/compose.sh logs --since=10m api_geteway schedule_web excel_processor minio_init
```

Inspect logs locally; do not share expanded Compose configuration or secrets. Keep virus scanning enabled. The scanner's first invocation may take longer than five seconds. IIS/ARR or another outer proxy must allow the complete upload request to finish; align its timeout above the application upload deadline and allow the intended multipart body size (API file limit is 20 MiB, ingress allows multipart overhead).

Finally, sign in and upload one supported Excel workbook. Expect “Файл отправлен на обработку.” and a processing/history update. Read back the result before retrying after any timeout. An unsupported spreadsheet schema is a processing error, distinct from an upload-service failure. If the new release still returns the generic error, record the exact upload time and correlate Rails `upstream_failure` events, API exceptions, ClamAV, discovery and MinIO logs; production server access is required to fix that underlying condition.

## Published source revisions

- API: `f84452e12fc4b584b6154922c42157f4cc56c92f`, branch `feature/date-resolved-schedule`.
- Excel worker: `d9bc3e0a2e17e13534bfa929cc93f8a08abfc1d9`, branch `main`.
- Legacy frontend: `e42bf76595336897621fe70f3bf2f3b867cd30c0`, branch `main`; optional and not the current Rails admin interface.
- Rails: local commit `92ff8791f0a26ad61662c163c07f20d4c2b2fd5b`; no owning Git remote was configured at publication time. Its Docker image is published; Git publication awaits the repository URL.

Browser plugin/skill was unavailable; Playwright MCP navigation failed with `NS_ERROR_PROXY_CONNECTION_REFUSED`. Production authentication/upload was therefore verified through direct HTTP with CSRF and an in-memory session, not a rendered browser journey. No screenshot/console or full UI acceptance is claimed. Seven normal image builds passed; changed API Ruby files, all Rails app/config Ruby files, and Python source syntax passed. The legacy frontend is checked separately by its container build.

Local diagnostic artifacts and private runtime files are kept out of Git publication. Existing broad application changes were published at the owner's request; the focused Excel check is not full regression acceptance for every unrelated feature.

## Focused update preserving the current ingress

If your server has customised Nginx/ARR or externally accessible administration, prefer this bounded application update after fetching the deployment files and setting the image tag/overrides above. Reuse existing generated discovery settings while the server network remains unchanged:

```sh
./deploy/bin/compose.sh config --quiet
./deploy/bin/compose.sh pull schedule_web api_geteway excel_processor \
  outbox_relay media_sanitizer notice_export_worker schedule_generation_worker ops_console
./deploy/bin/db.sh migrate
./deploy/bin/compose.sh up -d --no-build --no-deps --wait --wait-timeout 180 \
  schedule_web api_geteway excel_processor \
  outbox_relay media_sanitizer notice_export_worker schedule_generation_worker ops_console
./deploy/bin/compose.sh images
./deploy/bin/compose.sh ps
```

This keeps the currently running edge, database, Redis and MinIO containers. If discovery/network settings are stale, run preflight and reconcile them first; the full procedure earlier covers that case. Do not replace a custom ingress configuration blindly. Existing dependency services must be healthy before the bounded update.
