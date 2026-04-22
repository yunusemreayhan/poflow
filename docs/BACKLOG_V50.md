# Backlog V50 — Cloud Scaling & Performance (2026-04-22)

Scope: Make Poflow production-ready for 100M registered users. Ranked by impact.

---

## V50-1 [Critical / Infra] Replace SQLite with PostgreSQL
**Impact:** 100x write throughput, unlocks horizontal scaling.

SQLite is single-writer — every mutation serializes through one lock. Postgres gives concurrent writers, connection pooling (PgBouncer), read replicas, and partitioning. sqlx already supports Postgres; migration is connection strings + minor query syntax (`last_insert_rowid()` → `RETURNING id`).

**Tasks:**
- [ ] Add `postgres` feature to sqlx dependency alongside `sqlite`
- [ ] Create Postgres migration scripts from existing SQLite schema
- [ ] Replace `last_insert_rowid()` calls with `RETURNING id`
- [ ] Replace `IFNULL` with `COALESCE` where used
- [ ] Add `DATABASE_URL` env var support for Postgres connection string
- [ ] Update Docker Compose with Postgres service
- [ ] Add PgBouncer config for connection pooling
- [ ] Write data migration script (SQLite → Postgres)
- [ ] Update docs/DEPLOYMENT.md and docs/ENV_VARS.md

## V50-2 [Critical / Infra] Add Redis for timer state + JWT cache + SSE fan-out
**Impact:** Makes API servers stateless → enables horizontal scaling.

Timer engine, SSE broadcast channels, and JWT validation are all in-process. This pins to a single server.

**Tasks:**
- [ ] Add `redis` crate dependency
- [ ] Move timer state from `parking_lot::Mutex` to Redis hashes with TTL
- [ ] Cache JWT validation results in Redis (avoid DB hit per request)
- [ ] Replace `tokio::sync::broadcast` SSE fan-out with Redis Pub/Sub
- [ ] Add `POFLOW_REDIS_URL` env var
- [ ] Add Redis to Docker Compose
- [ ] Update health check to verify Redis connectivity

## V50-3 [High / Perf] Add response compression
**Impact:** 5-10x bandwidth reduction, 5 minutes of work.

```rust
.layer(tower_http::compression::CompressionLayer::new())
```

**Tasks:**
- [ ] Add `compression` feature to `tower-http` dependency
- [ ] Add `CompressionLayer` to router middleware stack

## V50-4 [High / Infra] CDN for static assets
**Impact:** Eliminates 80%+ of server bandwidth, free tier available.

React bundle, CSS, sounds, icons all served from Rust server. Put Cloudflare/CloudFront in front.

**Tasks:**
- [ ] Add `Cache-Control` headers for static assets (1 year, immutable for hashed files)
- [ ] Add `Cache-Control: no-cache` for `index.html`
- [ ] Document Cloudflare setup in DEPLOYMENT.md
- [ ] Add `POFLOW_ASSET_URL` env var for CDN origin override in frontend

## V50-5 [High / Perf] Cursor-based pagination
**Impact:** Prevents catastrophic slowdown at scale.

Default 5000/page with offset pagination. `OFFSET 50000` scans and discards 50K rows.

**Tasks:**
- [ ] Add `cursor` query param support to task list endpoints
- [ ] Implement keyset pagination (`WHERE id > ? ORDER BY id LIMIT ?`)
- [ ] Add `next_cursor` field to paginated responses
- [ ] Keep offset pagination as fallback for backward compatibility
- [ ] Update `/api/tasks`, `/api/sprints`, `/api/audit`, `/api/notifications`

## V50-6 [High / Perf] Read/write splitting
**Impact:** 3-5x read throughput once Postgres is in place.

Route GET requests to read replicas, mutations to primary.

**Tasks:**
- [ ] Create separate read-only and read-write connection pools
- [ ] Route GET/HEAD handlers to read pool
- [ ] Route POST/PUT/DELETE handlers to write pool
- [ ] Add `POFLOW_READ_DATABASE_URL` env var for replica
- [ ] Handle replication lag for read-after-write consistency (sticky sessions or write-pool fallback)

## V50-7 [Medium / Observability] Add Prometheus metrics endpoint
**Impact:** Enables data-driven optimization.

**Tasks:**
- [ ] Add `metrics` and `metrics-exporter-prometheus` crates
- [ ] Expose `/metrics` endpoint (no auth required)
- [ ] Track: request latency histograms per endpoint, DB query duration, active SSE connections, timer count, cache hit/miss rate
- [ ] Add Grafana dashboard template to `docs/`

## V50-8 [Medium / Infra] Extract timer engine to separate worker
**Impact:** Decouples hottest loop from request handling.

1-second tick loop competes with API requests for CPU.

**Tasks:**
- [ ] Create `poflow-timer-worker` crate in workspace
- [ ] Worker reads/writes timer state from Redis
- [ ] Worker publishes tick events to Redis Pub/Sub
- [ ] API servers subscribe to timer events for SSE
- [ ] Add worker to Docker Compose as separate service

## V50-9 [Medium / Perf] Async event queue for audit + webhooks + notifications
**Impact:** Removes 3 synchronous writes from mutation hot path.

Every task update currently: update → audit_log → webhooks → notifications → respond.

**Tasks:**
- [ ] Add Redis Streams (or Postgres LISTEN/NOTIFY) as event bus
- [ ] Publish events from route handlers instead of synchronous writes
- [ ] Create background consumer for audit_log writes
- [ ] Create background consumer for webhook dispatch
- [ ] Create background consumer for notification creation
- [ ] Add dead-letter handling for failed webhook deliveries

## V50-10 [Medium / Perf] Database indexing overhaul
**Impact:** 2-10x faster queries on common paths.

Current 13 indexes cover basics. Need composite indexes for actual query patterns.

**Tasks:**
- [ ] Add `tasks(user_id, status, deleted_at)` — most common query
- [ ] Add `tasks(parent_id, sort_order)` — tree traversal
- [ ] Add `burn_log(sprint_id, cancelled_at, user_id)` — burn summary
- [ ] Add `sessions(user_id, started_at DESC)` — history
- [ ] Add `sprint_tasks(sprint_id, task_id)` — board rendering
- [ ] Add `audit_log(entity_type, entity_id, created_at DESC)` — audit queries
- [ ] Run EXPLAIN ANALYZE on top 20 slowest queries and add missing indexes

## V50-11 [Low / Infra] Graceful shutdown + zero-downtime deploys
**Impact:** Eliminates downtime during updates.

**Tasks:**
- [ ] Handle SIGTERM: stop accepting connections, drain in-flight (30s), close pools
- [ ] Document blue-green deploy strategy in DEPLOYMENT.md
- [ ] Add readiness probe endpoint (`/api/ready`) separate from health

## V50-12 [Low / API] API versioning
**Impact:** Future-proofs API contract.

**Tasks:**
- [ ] Add `/api/v1/` prefix to all current routes
- [ ] Keep `/api/` as alias for v1 (backward compat)
- [ ] Document versioning policy in API_CHANGELOG.md

---

## Cloud Cost Estimates

### GCP (cheapest for bursty traffic)
| Component | Service | Monthly |
|---|---|---|
| API servers | Cloud Run (4 × 1vCPU/2GB, scale-to-zero) | ~$80 |
| Database | Cloud SQL Postgres + 1 read replica | ~$130 |
| Cache | Memorystore Redis 1GB | ~$40 |
| CDN | Cloud CDN 1TB | ~$80 |
| LB | Cloud Load Balancing | ~$20 |
| **Total** | | **~$350/mo** |

### AWS
| Component | Service | Monthly |
|---|---|---|
| API servers | ECS Fargate (4 × 1vCPU/2GB) | ~$120 |
| Database | RDS Postgres t4g.medium + 1 replica | ~$140 |
| Cache | ElastiCache Redis t4g.small | ~$50 |
| CDN | CloudFront 1TB | ~$85 |
| LB | ALB | ~$25 |
| **Total** | | **~$425/mo** |

### Bootstrap (single server)
| Component | Service | Monthly |
|---|---|---|
| Everything | Hetzner CX32 (4vCPU/8GB) + Cloudflare free | ~$15 |

---

## Summary

| ID | Priority | Category | Description |
|----|----------|----------|-------------|
| V50-1 | Critical | Infra | SQLite → PostgreSQL |
| V50-2 | Critical | Infra | Redis for state + cache + pub/sub |
| V50-3 | High | Perf | Response compression |
| V50-4 | High | Infra | CDN for static assets |
| V50-5 | High | Perf | Cursor-based pagination |
| V50-6 | High | Perf | Read/write splitting |
| V50-7 | Medium | Observability | Prometheus metrics |
| V50-8 | Medium | Infra | Timer engine → separate worker |
| V50-9 | Medium | Perf | Async event queue |
| V50-10 | Medium | Perf | Index overhaul |
| V50-11 | Low | Infra | Graceful shutdown |
| V50-12 | Low | API | API versioning |

**Total: 12 items** — 0 fixed, 12 open
