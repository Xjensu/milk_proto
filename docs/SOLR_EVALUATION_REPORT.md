# Solr evaluation report

Date: 2026-09-12  
Status: **defer production adoption**

The isolated `search-experiment` profile was run against the six-document `schedule_experiment` fixture. The query set is version 1 and contains Russian teacher, subject and room searches, an abbreviation, punctuation, a typo, transliteration and an empty-result case. The benchmark uses the indexed `name_txt` field, 2 warm-up requests and 20 measured requests per query. The raw output is [`solr_benchmark_2026-09-12.json`](solr_benchmark_2026-09-12.json).

Known-item checks passed for the exact and partial teacher queries, subject morphology and abbreviation, room number and building queries, punctuation and the typo case. The transliteration query (`Ivanov`) returned no result, which is recorded as unsupported by this fixture. The empty query returned no result as required. Observed p95 latency was approximately 31–79 ms on this local run; these values are not a production capacity claim.

Solr remains an experiment because the evidence does not yet include a PostgreSQL comparison on the same dataset, authorization-filter checks, update/delete ordering, freshness measurement, outage fallback or a representative index size. PostgreSQL remains authoritative and administrator searches continue to use the existing relational path.

Before reconsidering adoption, run the same versioned query set against PostgreSQL, add authorization and mutation/freshness cases, and record p50/p95 latency, judged top-five usefulness, indexing cost and fallback behavior. Any future index must be a rebuildable projection fed by durable events; the browser must never choose a collection or bypass API authorization.
