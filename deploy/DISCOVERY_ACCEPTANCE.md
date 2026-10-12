# Discovery deployment acceptance — 2026-10-04

Verified on the current Docker host using the generated LAN policy. Existing data and unrelated working-tree changes were preserved; no account insertion, password reset, schema reset, or new test files were used.

- Compose configuration, infrastructure Python/Ruby syntax, and running Nginx configuration passed validation.
- The healthy Consul catalog contained two `schedule-web` instances and one each of API, scheduler, operations console, PostgreSQL, Redis, MinIO, Excel processor, outbox relay, media sanitizer, notice exporter, generation worker, and Nginx. Scoped ACL provisioning was rerun successfully.
- Only project HTTP 80 and LAN-bound MinIO 9000 were published. Application and discovery ports were unpublished.
- Public browser rendering and LAN administrator sign-in worked. The administrator journey created a temporary workload, assigned a teacher, placed a lesson, edited its time slot, reloaded and read back the persisted edit. The temporary lesson and workload were deleted, and the original workload row count was restored.
- Sampled successful public requests reached both Rails upstreams. With one web instance stopped, repeated requests remained successful; both instances were subsequently restored and healthy.
- A Consul outage initially served cached public requests and subsequently returned 503; the sustained outage still returned 503 at 70 seconds. After Consul restarted, registration, public rendering, sign-in and readiness recovered.
- A non-LAN container reached public HTTP but received 403 for administrator, cabinet, operations, metrics, session and administrative API routes, including with forged forwarding headers. Internal callbacks were not exposed.
- Host-bound MinIO access from a non-LAN container was blocked; the LAN readiness request returned 200. Verification caught Docker's userland-proxy bypass of DOCKER-USER, and the installer now also protects the selected host address/port through INPUT.
- Nine empty authentication POST requests produced six application 422 responses and three Nginx 429 responses with Retry-After 60. No credentials were submitted in this limit check.
- A temporary denylist entry returned 403 and was then removed; the original denylist was restored.
- An invalid LAN interface override failed without widening the administrator policy. The preflight was rerun successfully without rotating service tokens.

Browser MCP navigation failed with NS_ERROR_PROXY_CONNECTION_REFUSED. Verification used the installed local Playwright Chromium with the host proxy bypassed; no browser dependencies were installed. Screenshots remained local temporary artifacts.

Not exercised: access from another physical network/router, dependency IP relocation under active transactions, full Excel import/export processing, and every scheduler/operations workflow. These are not claimed as runtime acceptance. Router forwarding and external firewall reachability must be verified on the deployment machine. This remains an HTTP-only, single-host deployment with a single registry and stateful dependencies.
