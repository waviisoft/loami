<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./brand/loami-lockup-dark.svg">
    <img src="./brand/loami-lockup.svg" alt="Loami" width="360">
  </picture>
</p>

# Introduction

**Loami** is _fertile ground for your backend_ — an embeddable backend substrate for early-stage
apps that provides a **document store, durable queue, realtime websocket backplane, and
background-job runner** over one self-clustering, churn-tolerant kernel whose only dependencies are
**compute + blob storage** (S3 / GCS / Azure Blob / Cloudflare R2).

> ⚠️ **Status: pre-alpha.** Loami is in early development. This site documents the project as it
> takes shape.

## Why

Every new app needs durable storage, background jobs, realtime updates across instances, and
fleet-wide session state almost immediately. Today that means Postgres + Redis + a queue + a
realtime provider + a worker library + sticky sessions — cost, ops, and a local-vs-prod gap, before
the first customer. Loami collapses that into a single embeddable library you run on nothing but
compute and a bucket.

## Principles

1. **Compute + blob, nothing else.** No required external services.
2. **Dev/prod parity by connection string** — `mem://` in CI, `file://` locally, `s3://` in prod.
3. **Graceful exit** — every facet's API mirrors its graduation target (Mongo / Kafka / Redis), so
   migrating off when you outgrow Loami is a first-class path.
4. **Embeddable & polyglot** — a library first (Rust core + bindings), a sidecar/CLI second.
5. **Lightweight & opt-in** — shared substrate, pluggable facets.
6. **Churn-tolerant by default** — assume spot instances vanish.

See the [Roadmap](./roadmap.md) for sequencing, and the
[API reference](https://waviisoft.github.io/loami/api/) for the crate docs.
