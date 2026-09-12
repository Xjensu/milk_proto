# Graph recommendation evaluation

Date: 2026-09-12  
Status: **keep the PostgreSQL projection; defer a graph extension**

The current prototype is [`apps/schedule_web/docs/handoff/GRAPH_RECOMMENDATION_PROTOTYPE.sql`](../apps/schedule_web/docs/handoff/GRAPH_RECOMMENDATION_PROTOTYPE.sql). It derives explainable observed edges from subjects to teachers and rooms through existing workload and assignment tables, then ranks candidates by observation count. The query was accepted by the running PostgreSQL 15 instance with `EXPLAIN (COSTS OFF)`; the planner used the primary-key indexes for workload and curriculum joins.

This query is an advisory candidate lookup. It deliberately does not authorize a teacher or room, check availability, enforce group/slot conflicts or reserve a timetable cell. The scheduling API must perform those checks again at placement time. A stale or deleted edge must therefore fail safely rather than create a lesson.

SPARQL is an RDF query language and is not a graph database embedded in PostgreSQL. Apache AGE would add a PostgreSQL property-graph projection using Cypher, while RDF interoperability would require a separately evaluated RDF store or mapping. No extension is installed in the live database. Adoption is deferred until the same anonymized inputs can be compared against a graph projection for query latency, rebuild/update cost, explanation quality, authorization filtering and stale-edge behavior.
