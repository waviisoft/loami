# Loami

**Fertile ground for your backend.**

> _SQLite's simplicity, for your entire backend._

> ⚠️ **Status: pre-alpha / design phase.** Loami is an early design exploration — there is no
> usable code yet. This README captures the vision and scope so the shape is clear before the
> first line of the engine is written. Expect everything here to change.

---

## The problem

Every new app needs the same things almost immediately: durable data storage, background jobs,
realtime updates across instances, and session state that survives a multi-instance fleet. Today that
means standing up and paying for a constellation of services — Postgres **+** Redis **+** a queue
(Kafka/SQS/RabbitMQ) **+** a realtime provider (Pusher/Ably) **+** a worker library **+** sticky-session
config — before you have a single customer. Each adds cost, ops burden, and a gap between local dev and
production. It's the reason the big cloud providers ship dozens of services — and it's overkill at 0→1.

## What it is

A single **embeddable library** (Rust core, polyglot bindings, plus a CLI/sidecar) that provides a
**document store, durable queue, realtime websocket backplane, and background-job runner** — over one
self-clustering, churn-tolerant kernel whose only dependencies are **compute + blob storage**
(S3 / GCS / Azure Blob / Cloudflare R2). Both are available on essentially every host, from AWS down to
Fly.io and Heroku. An in-memory hot tier provides speed; blob storage provides durability. Instances
discover each other and route ownership across the VPC, so a multi-instance fleet behaves like one
coherent backend with no external coordinator.

## Who it's for

- Solo devs, small teams, and early-stage startups building web/mobile apps at the **0→1 stage**.
- Teams that want **local-first dev/test ergonomics** and hate the local-vs-prod gap.
- Cost-sensitive teams running on cheap/churny compute (**spot instances**) who can't yet justify
  managed data infrastructure.
- **AI-app builders** who need async job handling (long-running model calls) and realtime token
  streaming to clients — without assembling a queue + websocket-provider stack.

## What it deliberately is **not**

- **Not a scale-out OLTP database** — no global consistency, no complex multi-document transactions,
  no SQL/joins. When you need those, graduate to Postgres.
- **Not Kafka-at-scale** — not for firehose throughput or event-sourcing-of-record. Graduate to
  Kafka / Redpanda.
- **Not an analytics/OLAP engine** — use DuckDB / ClickHouse.
- **Not a managed BaaS** — it's a library you embed and run on your own compute, not a vendor backend.
- **Not trying to be permanent** — success _includes_ the day you outgrow it and migrate off, and that
  migration being easy is a feature, not a failure.

## Why now

- Object storage finally has the missing primitives: **strong read-after-write consistency** (S3 2020+,
  GCS, R2) and **conditional writes / CAS** (S3 + R2, 2024) — enough to build correct single-writer
  fencing and atomic commits directly on a bucket.
- **Cloudflare R2's zero egress** makes constantly reading from the durable tier economical — egress
  cost historically killed blob-backed designs.
- **Mature building blocks** exist (Rust + WASM, OpenDAL, SlateDB, SWIM gossip libraries) — assemble
  rather than invent the hardest parts.
- **Local-first / CI-without-containers** is now an expectation, not a luxury.
- **AI apps** created fresh, acute demand for cheap async jobs + realtime streaming in otherwise-tiny
  apps — exactly this substrate's sweet spot.

## Design tenets (the non-negotiables)

1. **Compute + blob, nothing else.** No required external services, ever.
2. **Dev/prod parity by connection string.** `mem://` in CI, `file://` locally, `s3://` in prod — the
   same code path everywhere.
3. **Graceful exit.** Every facet's API mirrors its graduation target; migrating off is a first-class,
   documented path.
4. **Embeddable & polyglot.** A library first (Rust core + bindings), a sidecar/CLI second — not a
   server you must run.
5. **Lightweight & opt-in.** Shared substrate, pluggable facets; embed only what you use.
6. **Churn-tolerant by default.** Assume spot instances vanish; durability and failover are built in,
   not bolted on.

## Roadmap

1. **Document store** — durable, schemaless data. Single-node MVP first (`mem://` / `file://` / `s3://`),
   then a clustered tier.
2. **Websocket + message routing** — realtime fan-out across instances without a separate backplane.
3. **Pub/sub for job handling** — async workers (e.g. AI calls), at-least-once with idempotency.

## License

[MIT](./LICENSE) © 2026 WAVIISoft, LLC
