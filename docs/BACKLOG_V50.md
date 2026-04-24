# Backlog V50 — Auto-Scaling Production Architecture (2026-04-24)

Scope: Day-1'den otomatik ölçeklenen, reklam verdiğin gün 100K kullanıcı gelse bile ayakta kalan bir mimari.

Felsefe: **Baştan doğru tasarla, 1 node'da başlat, config değişikliğiyle ölçekle.**

---

## Mimari Genel Bakış

```
İnternet → Cloudflare (CDN + DDoS + SSL)
              │
              ▼
         Traefik (Reverse Proxy + Auto-discovery)
              │
     ┌────────┼────────┐
     ▼        ▼        ▼
  poflow   poflow   poflow     ← Nomad auto-scales (min:1, max:N)
   api-1    api-2    api-N       CPU >%70 → yeni instance
     │        │        │
     ├────────┼────────┤
     ▼        ▼        ▼
   Redis ◄──────────────────── timer state, JWT cache, SSE pub/sub
     │
   NATS  ◄──────────────────── async events (audit, webhook, notification)
     │
  Postgres ◄────────────────── data (+ read replica when needed)
```

**Tek sunucuda başlar, aynı config ile 100 sunucuya ölçeklenir.**

---

## V50-1 [Critical / Infra] SQLite → PostgreSQL

SQLite tek-writer. Her mutation tek bir lock'tan geçiyor. Postgres binlerce eşzamanlı writer destekler.
sqlx zaten Postgres destekliyor — migration çoğunlukla query syntax farkları.

**Kod değişiklikleri:**
- `Cargo.toml`: sqlx features `sqlite` → `postgres`
- `last_insert_rowid()` → `RETURNING id`
- `IFNULL()` → `COALESCE()`
- `INTEGER PRIMARY KEY` → `SERIAL PRIMARY KEY` veya `BIGSERIAL`
- `datetime('now')` → `NOW()`
- `GROUP_CONCAT` → `STRING_AGG`
- Connection string: `sqlite:` → `postgres://`

**Yeni dosyalar:**
- `migrations/` klasörü — sqlx-cli ile yönetilen migration dosyaları
- `scripts/migrate-sqlite-to-pg.sh` — mevcut SQLite verilerini Postgres'e aktarma

**Kapasite planı:**
| Kullanıcı | Postgres Spec | Maliyet |
|---|---|---|
| 0-10K | 1 vCPU, 2GB (Hetzner €4/ay veya tek sunucuda) | ~$5 |
| 10K-100K | 2 vCPU, 4GB + 1 read replica | ~$40 |
| 100K-1M | 4 vCPU, 16GB + 2 read replica | ~$150 |

## V50-2 [Critical / Infra] Redis — State + Cache + Pub/Sub

Timer state, JWT cache ve SSE fan-out'u Redis'e taşı. API server'lar stateless olur → yatay ölçekleme mümkün.

**Kod değişiklikleri:**
- `Cargo.toml`: `redis = { version = "0.27", features = ["tokio-comp", "connection-manager"] }`
- `engine.rs`: `parking_lot::Mutex<HashMap<UserId, TimerState>>` → Redis hash (`HSET timer:{user_id}`)
- `routes/timer.rs`: SSE broadcast → Redis Pub/Sub subscribe
- `auth.rs`: JWT validation → Redis'te cache (`SET jwt:{hash} {user_id} EX 7200`)
- Token revocation: `DEL jwt:{hash}` (anında, DB sorgusu yok)

**Kapasite planı:**
| Kullanıcı | Redis Spec | Maliyet |
|---|---|---|
| 0-50K | 256MB (tek sunucuda) | $0 |
| 50K-500K | 1GB dedicated | ~$15 |
| 500K+ | Redis Cluster 3-node | ~$50 |

## V50-3 [Critical / Infra] NATS — Async Event Bus

Şu an her task update: `update → audit_log → webhook → notification → response`. Kullanıcı hepsini bekliyor.
NATS ile: `update → NATS publish → response`. Gerisini consumer'lar async halleder.

**Neden NATS (Kafka/RabbitMQ değil):**
- Tek binary, 10MB RAM, sıfır bağımlılık
- Saniyede 10M+ mesaj
- JetStream ile persistent queue (mesaj kaybı yok)
- Rust client: `async-nats` crate, production-ready

**Kod değişiklikleri:**
- `Cargo.toml`: `async-nats = "0.38"`
- Yeni modül: `src/events.rs` — event publish/subscribe
- Route handler'larda senkron audit/webhook/notification çağrıları → `nats.publish("poflow.events.task.updated", payload)`
- Yeni binary: `poflow-event-worker` — NATS'tan consume edip audit_log yazar, webhook dispatch eder, notification oluşturur

**Event tipleri:**
```
poflow.events.task.{created|updated|deleted}
poflow.events.sprint.{started|completed}
poflow.events.timer.{completed}
poflow.events.user.{registered|login}
```

## V50-4 [Critical / Infra] Podman Compose — Production Stack

Docker Compose drop-in replacement. Daemonless, rootless, tamamen ücretsiz. OCI-uyumlu.

**Dosya: `podman-compose.prod.yml`**
```yaml
services:
  postgres:
    image: docker.io/postgres:17-alpine
    environment:
      POSTGRES_DB: poflow
      POSTGRES_USER: poflow
      POSTGRES_PASSWORD_FILE: /run/secrets/pg_password
    volumes:
      - pg-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U poflow"]
      interval: 10s
      timeout: 5s
      retries: 5
    deploy:
      resources:
        limits: { cpus: "2", memory: "2G" }

  redis:
    image: docker.io/redis:7-alpine
    command: redis-server --maxmemory 256mb --maxmemory-policy allkeys-lru --appendonly yes
    volumes:
      - redis-data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s

  nats:
    image: docker.io/nats:2-alpine
    command: -js -sd /data  # JetStream enabled, persistent
    volumes:
      - nats-data:/data
    healthcheck:
      test: ["CMD", "nats-server", "--signal", "ldm"]  # lame duck mode check
      interval: 10s

  poflow-api:
    image: localhost/poflow:latest
    environment:
      DATABASE_URL: postgres://poflow:${PG_PASSWORD}@postgres:5432/poflow
      POFLOW_REDIS_URL: redis://redis:6379
      POFLOW_NATS_URL: nats://nats:4222
      POFLOW_BIND_ADDRESS: "0.0.0.0"
      POFLOW_GUI_DIR: /usr/share/poflow/gui
      POFLOW_LOG_JSON: "1"
      RUST_LOG: poflow_daemon=info
    depends_on:
      postgres: { condition: service_healthy }
      redis: { condition: service_healthy }
      nats: { condition: service_healthy }
    healthcheck:
      test: ["CMD", "wget", "-qO-", "http://localhost:9090/api/health"]
      interval: 15s
      timeout: 5s
      retries: 3
    deploy:
      replicas: 1  # Nomad overrides this in production

  poflow-timer:
    image: localhost/poflow:latest
    command: ["poflow-daemon", "--timer-worker-only"]
    environment:
      DATABASE_URL: postgres://poflow:${PG_PASSWORD}@postgres:5432/poflow
      POFLOW_REDIS_URL: redis://redis:6379
      POFLOW_NATS_URL: nats://nats:4222
    depends_on:
      postgres: { condition: service_healthy }
      redis: { condition: service_healthy }

  poflow-events:
    image: localhost/poflow:latest
    command: ["poflow-daemon", "--event-worker-only"]
    environment:
      DATABASE_URL: postgres://poflow:${PG_PASSWORD}@postgres:5432/poflow
      POFLOW_REDIS_URL: redis://redis:6379
      POFLOW_NATS_URL: nats://nats:4222
    depends_on:
      postgres: { condition: service_healthy }
      nats: { condition: service_healthy }

  traefik:
    image: docker.io/traefik:v3
    command:
      - --providers.docker=true
      - --providers.docker.exposedbydefault=false
      - --entrypoints.web.address=:80
      - --entrypoints.websecure.address=:443
      - --certificatesresolvers.le.acme.httpchallenge.entrypoint=web
      - --api.dashboard=true
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /run/podman/podman.sock:/var/run/docker.sock:ro
      - traefik-certs:/certs

volumes:
  pg-data:
  redis-data:
  nats-data:
  traefik-certs:
```

**Başlatma:**
```bash
# Tek komut ile tüm stack ayağa kalkar
podman-compose -f podman-compose.prod.yml up -d

# Ölçekleme (manual, Nomad olmadan):
podman-compose -f podman-compose.prod.yml up -d --scale poflow-api=3
```

## V50-5 [Critical / Infra] Nomad — Auto-Scaling Orkestrasyon

Kubernetes'ten 10x daha basit. Raw binary çalıştırabilir. Tek binary kurulum.

**Neden Nomad (K8s değil):**
- Tek binary, 5 dakikada kurulur (K8s: saatler/günler)
- Raw binary orkestre eder (Docker image'a bile gerek yok)
- HashiCorp Nomad Community Edition tamamen ücretsiz ve açık kaynak
- Poflow gibi az sayıda servis için ideal (K8s 50+ mikroservis için tasarlandı)
- Built-in service discovery (Consul'a gerek yok)

**Dosya: `deploy/poflow-api.nomad.hcl`**
```hcl
job "poflow-api" {
  datacenters = ["dc1"]
  type        = "service"

  group "api" {
    count = 1  # başlangıç instance sayısı

    scaling {
      enabled = true
      min     = 1
      max     = 20

      policy {
        # CPU %70'i geçince yeni instance
        check "cpu" {
          source = "nomad-apm"
          query  = "avg_cpu"
          strategy "target-value" {
            target = 70
          }
        }
        # Aktif bağlantı sayısı 1000'i geçince
        check "connections" {
          source = "prometheus"
          query  = "poflow_active_connections"
          strategy "target-value" {
            target = 1000
          }
        }
      }
    }

    network {
      port "http" { to = 9090 }
    }

    service {
      name = "poflow-api"
      port = "http"
      tags = ["traefik.enable=true"]

      check {
        type     = "http"
        path     = "/api/health"
        interval = "10s"
        timeout  = "3s"
      }
    }

    task "api" {
      driver = "docker"  # veya "raw_exec" ile direkt binary

      config {
        image = "poflow:latest"
        ports = ["http"]
      }

      env {
        DATABASE_URL       = "postgres://poflow:pass@postgres.service.consul:5432/poflow"
        POFLOW_REDIS_URL   = "redis://redis.service.consul:6379"
        POFLOW_NATS_URL    = "nats://nats.service.consul:4222"
        POFLOW_LOG_JSON    = "1"
        POFLOW_BIND_ADDRESS = "0.0.0.0"
      }

      resources {
        cpu    = 500   # MHz
        memory = 512   # MB
      }
    }
  }
}
```

**Ölçekleme senaryoları:**
```
Gece 03:00  → 50 online   → 1 API instance  → maliyet: $15/ay
Sabah 09:00 → 5K online   → 2 API instance  → Nomad otomatik kaldırdı
Reklam günü → 100K signup  → 10 API instance → Nomad 60 saniyede ölçekledi
Reklam bitti → 2K online   → 1 API instance  → Nomad fazlaları kapattı
```

## V50-6 [High / Perf] Response Compression

```rust
// lib.rs — router'a ekle
.layer(tower_http::compression::CompressionLayer::new())
```

`Cargo.toml`:
```toml
tower-http = { version = "0.6", features = ["cors", "trace", "fs", "compression-gzip", "compression-br"] }
```

5 dakikalık iş, 5-10x bandwidth azalması.

## V50-7 [High / Perf] Cursor-Based Pagination

Offset pagination `OFFSET 50000` → 50K satır tarar ve atar. Cursor pagination O(1).

**Değişiklik:**
```rust
// Eski: ?page=100&per_page=50
// Yeni: ?cursor=eyJpZCI6NTAwMH0&limit=50

// Query:
// WHERE id > $cursor ORDER BY id ASC LIMIT $limit
```

Response'a `next_cursor` eklenir. Eski `?page=` parametresi backward compat için kalır.

## V50-8 [High / Perf] Read/Write Splitting

Postgres'te read replica eklendiğinde:
- GET → read replica (trafiğin %95'i)
- POST/PUT/DELETE → primary

```rust
pub struct DbPools {
    pub write: PgPool,  // primary
    pub read: PgPool,   // replica (yoksa primary'ye fallback)
}
```

`POFLOW_READ_DATABASE_URL` env var'ı set edilmezse, read pool = write pool. Sıfır config ile çalışır, replica ekleyince otomatik kullanır.

## V50-9 [High / Infra] Dockerfile Güncelleme

Mevcut Dockerfile'da hatalar var (`pojidora-daemon`, `POMODORO_*` env vars). Düzelt + multi-stage optimize et.

```dockerfile
## Build: Rust backend
FROM docker.io/rust:1.86-bookworm AS backend
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release -p poflow-daemon

## Build: Frontend
FROM docker.io/node:22-bookworm-slim AS frontend
WORKDIR /build/gui
COPY gui/package.json gui/package-lock.json ./
RUN npm ci --ignore-scripts
COPY gui/ .
RUN npm run build

## Runtime: minimal
FROM docker.io/debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libpq5 wget && \
    rm -rf /var/lib/apt/lists/*
COPY --from=backend /build/target/release/poflow-daemon /usr/bin/poflow-daemon
COPY --from=frontend /build/gui/dist /usr/share/poflow/gui

ENV POFLOW_GUI_DIR=/usr/share/poflow/gui \
    POFLOW_BIND_ADDRESS=0.0.0.0 \
    RUST_LOG=poflow_daemon=info

RUN useradd -r -s /bin/false appuser && mkdir -p /data && chown appuser:appuser /data
EXPOSE 9090
VOLUME /data
HEALTHCHECK --interval=15s --timeout=5s --retries=3 \
    CMD wget -qO- http://localhost:9090/api/health || exit 1
USER appuser
CMD ["poflow-daemon"]
```

Değişiklikler: `pojidora` → `poflow`, `libsqlite3-0` → `libpq5`, env var isimleri düzeltildi.

## V50-10 [Medium / Observability] Prometheus Metrics + Grafana

```rust
// Cargo.toml
metrics = "0.24"
metrics-exporter-prometheus = "0.16"

// main.rs
let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
let handle = builder.install_recorder().unwrap();
// /metrics endpoint'i handle.render() döner
```

**Metrikler:**
- `poflow_http_requests_total{method, path, status}` — counter
- `poflow_http_request_duration_seconds{method, path}` — histogram
- `poflow_db_query_duration_seconds{query}` — histogram
- `poflow_active_sse_connections` — gauge
- `poflow_active_timers` — gauge
- `poflow_redis_cache_hits_total` / `poflow_redis_cache_misses_total`

Nomad auto-scaling bu metrikleri kullanarak ölçekleme kararı verir.

## V50-11 [Medium / Perf] Database Index Overhaul

```sql
-- En sık sorgulanan yollar
CREATE INDEX idx_tasks_user_status ON tasks(user_id, status, deleted_at);
CREATE INDEX idx_tasks_parent_sort ON tasks(parent_id, sort_order);
CREATE INDEX idx_burn_log_sprint ON burn_log(sprint_id, cancelled_at, user_id);
CREATE INDEX idx_sessions_user_time ON sessions(user_id, started_at DESC);
CREATE INDEX idx_sprint_tasks_composite ON sprint_tasks(sprint_id, task_id);
CREATE INDEX idx_audit_entity ON audit_log(entity_type, entity_id, created_at DESC);
CREATE INDEX idx_tasks_project_status ON tasks(project, status) WHERE deleted_at IS NULL;
```

## V50-12 [Medium / Infra] Graceful Shutdown

```rust
// main.rs
let shutdown = async {
    tokio::signal::ctrl_c().await.ok();
    tracing::info!("Shutting down gracefully...");
};
axum::serve(listener, app)
    .with_graceful_shutdown(shutdown)
    .await?;
// Drain süresi: 30 saniye (in-flight request'ler tamamlanır)
```

Ayrı `/api/ready` endpoint'i — Nomad/Traefik bunu kullanarak yeni instance hazır olana kadar trafik yönlendirmez.

## V50-13 [Low / API] API Versioning

Tüm route'lara `/api/v1/` prefix'i ekle. `/api/` → `/api/v1/` redirect. Breaking change geldiğinde `/api/v2/` açılır, v1 çalışmaya devam eder.

---

## Maliyet Tablosu — Kullanıcı Sayısına Göre Otomatik Ölçekleme

| Kullanıcı | API Nodes | Postgres | Redis | NATS | Sunucu | Aylık Maliyet |
|---|---|---|---|---|---|---|
| 0-1K | 1 | Aynı sunucu | Aynı sunucu | Aynı sunucu | 1× Hetzner CX22 | **€5** |
| 1K-10K | 1-2 | Aynı sunucu | Aynı sunucu | Aynı sunucu | 1× Hetzner CX32 | **€9** |
| 10K-50K | 2-4 | Dedicated 2vCPU | Dedicated 1GB | Aynı sunucu | 3× Hetzner | **€30** |
| 50K-200K | 4-8 | 4vCPU + replica | 2GB | Dedicated | 5× Hetzner | **€80** |
| 200K-1M | 8-20 | 8vCPU + 2 replica | Cluster | Cluster | Cloud (Hetzner/AWS) | **€300** |

**Önemli:** Tüm bu geçişler **kod değişikliği gerektirmez**. Nomad job spec'inde `count` ve `resources` değişir, yeni sunucu eklenir, Nomad otomatik dağıtır.

## Freemium İş Modeli Desteği

| Tier | Task Limiti | Fiyat | Teknik |
|---|---|---|---|
| Free | 50 task, 2 proje | $0 | `users.tier = 'free'`, API'de limit check |
| Pro | Sınırsız | $5/ay | `users.tier = 'pro'` |
| Team | Sınırsız + team features | $8/kullanıcı/ay | `users.tier = 'team'` |

Limit enforcement API middleware'de: tier check → 403 "Upgrade to Pro".

---

## Uygulama Sırası

| Sıra | ID | Ne | Neden Önce |
|---|---|---|---|
| 1 | V50-6 | Response compression | 5 dk iş, anında etki |
| 2 | V50-9 | Dockerfile düzelt | Mevcut hatalar var, deploy'u engelliyor |
| 3 | V50-1 | SQLite → Postgres | Her şeyin temeli |
| 4 | V50-2 | Redis ekle | Stateless API → ölçekleme mümkün |
| 5 | V50-3 | NATS ekle | Async events → response time düşer |
| 6 | V50-4 | Podman Compose stack | Tüm servisleri tek komutla ayağa kaldır |
| 7 | V50-5 | Nomad job specs | Auto-scaling aktif |
| 8 | V50-10 | Prometheus metrics | Ölçekleme kararları veri-tabanlı olur |
| 9 | V50-7 | Cursor pagination | Büyük veri setlerinde çökmez |
| 10 | V50-8 | Read/write split | Replica ekleyince otomatik kullanır |
| 11 | V50-11 | Index overhaul | Query performansı |
| 12 | V50-12 | Graceful shutdown | Zero-downtime deploy |
| 13 | V50-13 | API versioning | Gelecek-güvenli |

---

## Araç Seçim Gerekçeleri

| Katman | Seçim | Neden Bu (Alternatif Değil) |
|---|---|---|
| Konteyner | **Podman** | Daemonless, rootless, Docker-uyumlu, tamamen ücretsiz |
| Orkestrasyon | **Nomad** | Tek binary, 5dk kurulum, raw binary desteği (K8s overkill) |
| Reverse Proxy | **Traefik** | Auto-discovery, Let's Encrypt otomatik, Nomad/Docker entegrasyonu |
| Event Bus | **NATS** | Tek binary, 10MB RAM, JetStream persistent, Rust client mükemmel |
| Cache/State | **Redis** | Endüstri standardı, pub/sub + cache + state tek araçta |
| Database | **PostgreSQL** | Endüstri standardı, read replica, partitioning, sqlx desteği |
| CDN | **Cloudflare** | Ücretsiz tier, DDoS koruması, SSL, global edge |
| Monitoring | **Prometheus + Grafana** | Açık kaynak, Nomad entegrasyonu, auto-scaling metrikleri |

**Kullanılmayan ve neden:**
- **Kubernetes**: 5 servislik bir stack için overkill. Nomad aynı işi 1/10 karmaşıklıkla yapar.
- **Consul**: Nomad'ın built-in service discovery'si yeterli. 50+ servis olunca düşünülür.
- **Istio/Linkerd**: Service mesh, servisler arası mTLS gerektiğinde. Şimdilik gereksiz.
- **Kafka**: Milyarlarca event/gün olunca. NATS JetStream 100M kullanıcıya kadar yeter.
- **RabbitMQ**: NATS daha hafif, daha hızlı, daha az operasyonel yük.

---

## Summary

| ID | Priority | Category | Description |
|----|----------|----------|-------------|
| V50-1 | Critical | Infra | SQLite → PostgreSQL |
| V50-2 | Critical | Infra | Redis — state + cache + pub/sub |
| V50-3 | Critical | Infra | NATS — async event bus |
| V50-4 | Critical | Infra | Podman Compose production stack |
| V50-5 | Critical | Infra | Nomad auto-scaling orkestrasyon |
| V50-6 | High | Perf | Response compression (gzip/brotli) |
| V50-7 | High | Perf | Cursor-based pagination |
| V50-8 | High | Perf | Read/write splitting |
| V50-9 | High | Infra | Dockerfile düzeltme + optimizasyon |
| V50-10 | Medium | Observability | Prometheus metrics + Grafana |
| V50-11 | Medium | Perf | Database index overhaul |
| V50-12 | Medium | Infra | Graceful shutdown + zero-downtime |
| V50-13 | Low | API | API versioning |

**Total: 13 items** — 0 fixed, 13 open
