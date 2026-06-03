# Roadmap

Loami is built in **wedges** — each a standalone-useful facet over a shared kernel. The order is
dependency-correct: later wedges persist their state (offsets, job state, dedup keys) in the
document store from wedge 1.

## 1. Document store _(current focus)_

Durable, schemaless JSON storage.

- **Phase 1 — single node, shippable:** Rust core; in-memory working set; pluggable backend by
  connection string (`mem://` / `file://` / `s3://`) via a storage abstraction; durable persistence
  to blob; snapshot-isolated reads; id lookup + simple field predicates + secondary indexes.
  Ships as "SQLite for JSON that persists to S3 and runs in CI" — with **none** of the distributed
  machinery.
- **Phase 2 — clustered:** SWIM membership, rendezvous (HRW) placement, RF≥2 replication, and
  conditional-write (CAS) fencing. Invisible until you run more than one instance.

## 2. Websocket + message routing

Realtime fan-out across instances without a separate backplane: a gossiped subscription registry
routes published messages only to nodes with interested subscribers.

## 3. Pub/sub for job handling

Durable queues + an embedded worker (`consume(queue, handler)`) with at-least-once delivery,
idempotent producers, retry/backoff, and dead-letter queues — for async work such as long-running
AI calls.

## What Loami is _not_

Not a scale-out OLTP database, not Kafka-at-scale, not an OLAP engine, not a managed BaaS. It is the
**0→1 substrate**: make starting trivial and graduating painless, then get out of the way.
