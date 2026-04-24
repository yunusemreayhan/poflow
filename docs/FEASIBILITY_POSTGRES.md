# Fizibilite Analizi: SQLite → PostgreSQL Geçiş

**Tarih:** 2026-04-24
**Kapsam:** V50-1 backlog item'ı için gerçekçi iş yükü tahmini ve test stratejisi.

---

## TL;DR

**Geçiş yapılabilir ama önemsiz bir iş değil.** Kodbase'de 564 sqlx query var, 33 dosyaya dağılmış. 377 yerde `?` bind parameter, 22 yerde `last_insert_rowid()`, ve 1 tane FTS5 (full-text search) yok ki bu Postgres'te `tsvector`+GIN olarak yeniden yazılması gereken en zor parça.

**Tahmini süre:** 3-5 tam iş günü (tek kişi).
**Risk:** Orta — test suite güçlü (924 unit test + 887 e2e), regression'ları yakalar ama FTS5 testleri riskli.
**Sonuç:** Geçişe değer — backlog'daki ölçekleme hedefinin önkoşulu.

---

## Kodbase Envanteri

### SQLite bağımlılığı — gerçek sayılar

```
Ölçüm                                  Adet   Dosya
─────────────────────────────────────────────────────
Toplam sqlx query çağrısı              564    33
SqlitePool / sqlite: referansı          97    23
SQLite-specific syntax (IFNULL vb.)     63    23
`?` bind parametreleri                 377    47
last_insert_rowid() çağrısı             22    22
strftime() / julianday()                 6     1 (history.rs)
AUTOINCREMENT                           37     1 (db/mod.rs)
FTS5 (tasks_fts)                         4     1 (db/tasks.rs)
```

### En yoğun dosyalar (değişiklik gerekecek)

| Dosya | Query | SQLite syntax | Zorluk |
|---|---|---|---|
| `src/db/mod.rs` | 151 | 38 | **Yüksek** — schema, FTS5, migrasyon |
| `src/db/users.rs` | 35 | 1 | Düşük — sadece last_insert_rowid |
| `src/routes/history.rs` | 29 | 9 | **Yüksek** — strftime, julianday |
| `src/db/tasks.rs` | 20 | 4 | **Yüksek** — FTS5 MATCH sorguları |
| `src/db/rooms.rs` | 23 | 1 | Düşük |
| Diğer 28 dosya | ~300 | ~10 | Düşük-Orta |

### Engelleyici teknikler

1. **FTS5 (Full-Text Search)** — `tasks_fts` virtual table + 3 trigger.
   - Postgres karşılığı: `tsvector` sütunu + `GIN` index + trigger
   - Query syntax tamamen farklı: `MATCH ?` → `@@ to_tsquery($1)`
   - `snippet()` fonksiyonu yok, `ts_headline()` kullanılır
   - **~4 saat yeniden yazma + test**

2. **Bind parameter syntax** — sqlx her iki driver için farklı davranıyor
   - SQLite: `WHERE id = ?`
   - Postgres: `WHERE id = $1`
   - `sqlx::query()` bunu **otomatik dönüştürmez**. Manuel değişiklik lazım.
   - **~6 saat — script ile yarı-otomatik yapılabilir (sed + manual review)**

3. **`last_insert_rowid()` → `RETURNING id`**
   - 22 yer, hepsi `INSERT ... VALUES (...) RETURNING id` olmalı
   - Ayrıca `fetch_one(...).await?.get::<i64, _>("id")` pattern'ine geçilecek
   - **~2 saat**

4. **Tarih fonksiyonları** (`history.rs`)
   - `strftime('%Y-W%W', ...)` → `to_char(..., 'IYYY-IW')`
   - `strftime('%H', ...)` → `EXTRACT(HOUR FROM ...)`
   - `julianday(a) - julianday(b)` → `EXTRACT(EPOCH FROM (a - b)) / 86400`
   - **~2 saat, ama test coverage zayıf olabilir**

5. **Schema tip değişiklikleri**
   - `INTEGER PRIMARY KEY AUTOINCREMENT` → `BIGSERIAL PRIMARY KEY`
   - `TEXT` tarih sütunları → `TIMESTAMPTZ` (veya TEXT olarak bırakılabilir ama önerilmez)
   - `REAL` → `DOUBLE PRECISION`
   - **~1 saat**

### Kolay olan şeyler

- `CREATE TABLE IF NOT EXISTS` pattern — ikisinde de çalışır
- Foreign key cascade — aynı syntax
- UNIQUE constraint'ler — aynı
- JOIN, GROUP BY, ORDER BY — aynı
- Çoğu route handler'ı zaten tip-agnostik

---

## Test Altyapısı — Mevcut Durum

### Unit/Integration Tests (924 test)

```rust
// Her test bunu yapıyor:
pub async fn app() -> axum::Router {
    let pool = db::connect_memory().await.unwrap();  // ← sqlite::memory:
    let engine = Arc::new(Engine::new(pool, config).await);
    build_router(engine).await
}
```

**Özellikler:**
- Her test **fresh DB** alıyor (izolasyon mükemmel)
- In-memory → diske yazma yok, çok hızlı
- 924 test toplam **~2-3 saniye** sürüyor (tahmin)
- Port çakışması yok, paralel çalışabilir
- CI'de de aynı şekilde çalışıyor

### E2E Tests (887 test)

```bash
# Her test dosyası ayrı daemon başlatıyor, random port, temp DB
./e2etests/run_e2e.sh
```

**Özellikler:**
- Her test dosyası için fresh SQLite file (`temp/poflow-{port}.db`)
- Gerçek HTTP + WebDriver ile GUI test
- **Süre: 15-20 dakika** (tahmin)

---

## Postgres ile Test Stratejisi — Karar Matrisi

### Seçenek 1: testcontainers-rs (ÖNERİLEN)

Her test için Docker/Podman ile Postgres konteyneri açar.

```rust
// Cargo.toml
[dev-dependencies]
testcontainers = "0.23"
testcontainers-modules = { version = "0.11", features = ["postgres"] }

// common/mod.rs
pub async fn connect_test() -> (PgPool, ContainerAsync<Postgres>) {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@localhost:{}/postgres", port);
    let pool = PgPoolOptions::new().connect(&url).await.unwrap();
    migrate(&pool).await.unwrap();
    (pool, container)
}
```

**Artılar:**
- Tam izolasyon (her test kendi DB'si)
- Production'daki Postgres ile birebir aynı behavior
- CI'de de çalışır (GitHub Actions Docker destekler)

**Eksiler:**
- Her test için konteyner başlatma: ~500ms-1s overhead
- 924 test × 1s = **~15 dakika** (şu an 2-3 saniye)
- Paralel çalıştırma RAM'i zorlar (her test = 50MB Postgres)
- Podman/Docker gereksinimi

**Azaltma:**
- Test pool: 4 Postgres konteyneri paylaşılır, her test `CREATE DATABASE test_{n}` yapar → **~3 dakika**

### Seçenek 2: Shared Postgres + Schema Reset

Tek bir Postgres instance, her test başında schema drop+create.

```rust
pub async fn connect_test() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    let pool = PgPoolOptions::new().connect(&url).await.unwrap();
    sqlx::query("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").execute(&pool).await.unwrap();
    migrate(&pool).await.unwrap();
    pool
}
```

**Artılar:**
- Konteyner başlatma yok, hızlı
- Toplam süre: **~30 saniye**

**Eksiler:**
- Paralel test çalıştırılamaz (schema paylaşımı)
- Developer her zaman bir Postgres çalıştırmak zorunda
- CI config daha karmaşık

### Seçenek 3: pg_tmp + testcontainers hibrit

Global bir Postgres konteyneri, her test `CREATE DATABASE` ile kendi DB'sini alır.

```rust
// Tek konteyner, çoklu DB
static CONTAINER: OnceCell<...> = ...;
pub async fn connect_test() -> PgPool {
    let container = CONTAINER.get_or_init(start_postgres).await;
    let db_name = format!("test_{}", uuid::Uuid::new_v4().simple());
    // CREATE DATABASE ... vs.
}
```

**Artılar:**
- Konteyner overhead 1 kere
- Paralel izolasyon korunur
- Toplam süre: **~1 dakika**

**Eksiler:**
- Setup kodu karmaşık
- CREATE DATABASE latency var (~100ms/test)

### Karar Matrisi

| Kriter | testcontainers (her test) | shared postgres | hibrit |
|---|---|---|---|
| Süre | 15 dk ❌ | 30 sn ✓ | 1 dk ✓ |
| İzolasyon | Mükemmel ✓ | Zayıf ❌ | İyi ✓ |
| CI Kolay | Orta | Zor | Orta |
| Paralel | Var | Yok ❌ | Var ✓ |
| Production-uyumlu | Mükemmel ✓ | İyi | İyi |
| Developer DX | Kolay | Orta | Kolay |

**Önerim: Hibrit (Seçenek 3).** 1 dakikada 924 test, tam izolasyon, production-benzer.

### E2E Tests İçin

E2E test'ler için `run_e2e.sh` güncellenecek:
- Postgres konteynerini test başında kaldır (tek seferlik)
- Her test için `CREATE DATABASE` + daemon'a uygun `DATABASE_URL`
- Test bittikten sonra `DROP DATABASE`
- Overhead: ~200ms/test × 887 = **~3 dakika ek süre**

### Aynı testleri koşabilir miyim?

**Evet, %95-98 oranında.** Test kodu SQLite'a bağımlı değil — sadece HTTP API'yi test ediyor. 924 test'in büyük çoğunluğu değişmeden çalışır. Değişmesi gerekebilecek test kategorileri:

- **FTS5 search testleri** (~5-10 test): Arama sonuç sıralaması farklı olabilir
- **Date formatting testleri** (~3-5 test): Postgres timestamp formatı farklı
- **Concurrent write testleri** (varsa): SQLite WAL vs Postgres MVCC davranışı farklı

Tahmini: **~15 test küçük ayarlama gerektirir**, gerisi aynen çalışır.

---

## Geçiş Sırası — Somut Plan

### Aşama 1: Hazırlık (4 saat)
- [ ] Yeni branch: `feat/postgres-migration`
- [ ] `Cargo.toml`: sqlx features `sqlite,chrono` → `postgres,chrono,runtime-tokio-rustls`
- [ ] `testcontainers` + `testcontainers-modules` dev-dependency
- [ ] `DATABASE_URL` env var parse (fallback: bugünkü SQLite path)

### Aşama 2: Schema çevirisi (4 saat)
- [ ] `db/mod.rs` → yeni schema (`BIGSERIAL`, `TIMESTAMPTZ`)
- [ ] FTS5 yerine `tsvector` + GIN index + trigger (db/tasks.rs)
- [ ] `migrations/001_initial.sql` — sqlx-cli uyumlu migration
- [ ] `connect_test()` hibrit test helper

### Aşama 3: Query dönüşümleri (8 saat)
- [ ] Script yaz: `?` → `$N` dönüşümü (yarı-otomatik)
- [ ] 22 yerde `last_insert_rowid()` → `RETURNING id`
- [ ] `IFNULL` → `COALESCE` (global find-replace)
- [ ] `history.rs` — `strftime`/`julianday` → Postgres eşdeğerleri
- [ ] FTS5 query'lerini `@@ to_tsquery()` ile değiştir

### Aşama 4: Test onarımı (6 saat)
- [ ] `common/mod.rs` → `connect_test()` Postgres versiyonu
- [ ] `cargo test -p poflow-daemon` — kırılan testleri düzelt
- [ ] Beklenen: ~15 test ayar gerektirir
- [ ] E2E test harness güncelle (`run_e2e.sh`)

### Aşama 5: Veri migrasyonu (3 saat)
- [ ] `scripts/migrate-sqlite-to-pg.sh` — pgloader kullanır
- [ ] Test: SQLite dump → Postgres restore → tüm testler geçmeli

### Aşama 6: CI + Docs (3 saat)
- [ ] `.github/workflows/ci.yml` — Postgres service container ekle
- [ ] `docs/DEPLOYMENT.md` güncelle
- [ ] `docs/ENV_VARS.md`: `DATABASE_URL` eklendi
- [ ] `docker-compose.yml` / `podman-compose.prod.yml` Postgres servisi

### Toplam Tahmin: **28 saat = 3.5 iş günü**

---

## Risk Değerlendirmesi

| Risk | Olasılık | Etki | Azaltma |
|---|---|---|---|
| FTS5 yeniden yazımı test coverage zayıf | Orta | Orta | E2E testlerde `/api/tasks/search` senaryolarını gözden geçir |
| Bind parametre dönüşümünde hata | Yüksek | Yüksek | Script ile değiştir, `cargo check` ile hemen yakalar |
| Test süresi 10x artması CI'yi yavaşlatır | Kesin | Düşük | Hibrit strateji ile 1dk'da kalır |
| pgloader SQLite→PG dönüşümünde veri kaybı | Düşük | Yüksek | Staging'de önce dene, row count kontrol |
| `TIMESTAMPTZ` timezone farkları | Orta | Orta | UTC'de kal, string parse'ı ayarla |
| Production'da rollback zorluğu | Orta | Yüksek | Feature flag ile çift-yazma dönemi |

---

## Fizibilite Sonucu

**Karar: DEVAM ET.**

- Kod değişikliği büyük ama **mekanik** — çoğu script'le yapılır
- Test suite mevcut, regressionu yakalar
- FTS5 tek gerçek zor parça, ama encapsulated (tek dosya)
- Test süresi kabul edilebilir (1 dakika)
- Ölçekleme hedefinin önkoşulu — kaçınılmaz

**Öneri:** 
1. Ayrı `feat/postgres-migration` branch'inde yap
2. Feature flag ile çift-driver desteği (sqlx `any` feature) geçici olarak tutulabilir
3. Staging'de 1 hafta gerçek trafikle test, sonra production

**Başlangıç noktası:** Yukarıdaki Aşama 1.

---

## Ek: SQLite-Postgres Syntax Eşleştirme Tablosu

| SQLite | Postgres |
|---|---|
| `INTEGER PRIMARY KEY AUTOINCREMENT` | `BIGSERIAL PRIMARY KEY` |
| `last_insert_rowid()` | `... RETURNING id` |
| `IFNULL(a, b)` | `COALESCE(a, b)` |
| `GROUP_CONCAT(col)` | `STRING_AGG(col, ',')` |
| `strftime('%Y-%m-%d', d)` | `to_char(d, 'YYYY-MM-DD')` |
| `strftime('%H', d)` | `EXTRACT(HOUR FROM d)` |
| `julianday(a) - julianday(b)` | `EXTRACT(EPOCH FROM (a-b))/86400` |
| `datetime('now')` | `NOW()` |
| `INSERT OR IGNORE` | `INSERT ... ON CONFLICT DO NOTHING` |
| `INSERT OR REPLACE` | `INSERT ... ON CONFLICT DO UPDATE SET ...` |
| `WHERE col LIKE ?` | `WHERE col ILIKE $1` (case-insensitive) |
| `ATTACH DATABASE` | schema kullanımı |
| `PRAGMA foreign_keys=ON` | (default on) |
| `VACUUM` | `VACUUM` (aynı) |
| `WHERE col MATCH ?` (FTS5) | `WHERE tsv @@ to_tsquery($1)` |
| `snippet(table, col, ...)` | `ts_headline(text, query, options)` |
| `rowid` | ID sütunu açıkça kullan |
| `?` bind | `$1`, `$2`, ... |
| `REAL` | `DOUBLE PRECISION` |
| `TEXT` (tarihler) | `TIMESTAMPTZ` |
