# Protobuf evaluation report

Date: 2026-09-12  
Status: **defer wire-format adoption**

The repository contains an isolated candidate at [`schemas/protobuf/schedule_change.proto`](../schemas/protobuf/schedule_change.proto). It models a schedule-change envelope with stable numeric identifiers, an explicit change enum and an event timestamp. It is not connected to Redis, the Outbox relay, the Rails browser API or the existing Avro consumers.

The active event path remains the JSON envelope with an Avro binary payload. Protobuf cannot replace that path safely until the project has a pinned compiler/runtime for every producer and consumer, generated-code review, payload size/encode/decode measurements and compatibility exercises for old readers with new writers and vice versa. The local environment does not currently provide `protoc`, so no generated artifact or performance claim is recorded.

Adoption is deferred. If the experiment is resumed, use a versioned envelope that keeps the stable event ID independent of serialization, reserve removed field numbers, test unknown and missing fields, reject corrupt or oversized input, and deploy dual readers before changing producers. Preserve the Avro reader and queued-event rollback path until the migration is complete.
