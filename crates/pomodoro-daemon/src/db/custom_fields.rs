use super::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct CustomField {
    pub id: i64,
    pub name: String,
    pub field_type: String, // "text", "number", "select", "date", "user"
    pub options: Option<String>, // JSON array for select type
    pub required: bool,
    pub sort_order: i64,
    pub created_by: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct TaskCustomValue {
    pub id: i64,
    pub task_id: i64,
    pub field_id: i64,
    pub value: Option<String>,
}

pub async fn list_custom_fields(pool: &Pool) -> Result<Vec<CustomField>> {
    Ok(sqlx::query_as::<_, CustomField>("SELECT * FROM custom_fields ORDER BY sort_order, id")
        .fetch_all(pool).await?)
}

pub async fn create_custom_field(pool: &Pool, name: &str, field_type: &str, options: Option<&str>, required: bool, sort_order: i64, created_by: i64) -> Result<CustomField> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO custom_fields (name, field_type, options, required, sort_order, created_by, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(name).bind(field_type).bind(options).bind(required).bind(sort_order).bind(created_by).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(CustomField { id, name: name.to_string(), field_type: field_type.to_string(), options: options.map(|s| s.to_string()), required, sort_order, created_by, created_at: now })
}

pub async fn update_custom_field(pool: &Pool, id: i64, name: &str, field_type: &str, options: Option<&str>, required: bool, sort_order: i64) -> Result<CustomField> {
    let result = sqlx::query("UPDATE custom_fields SET name = ?, field_type = ?, options = ?, required = ?, sort_order = ? WHERE id = ?")
        .bind(name).bind(field_type).bind(options).bind(required).bind(sort_order).bind(id)
        .execute(pool).await?;
    if result.rows_affected() == 0 { return Err(anyhow::anyhow!("not found")); }
    Ok(sqlx::query_as::<_, CustomField>("SELECT * FROM custom_fields WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn delete_custom_field(pool: &Pool, id: i64) -> Result<()> {
    let result = sqlx::query("DELETE FROM custom_fields WHERE id = ?").bind(id).execute(pool).await?;
    if result.rows_affected() == 0 { return Err(anyhow::anyhow!("not found")); }
    Ok(())
}

pub async fn set_task_field_value(pool: &Pool, task_id: i64, field_id: i64, value: Option<&str>) -> Result<()> {
    sqlx::query("INSERT INTO task_custom_values (task_id, field_id, value) VALUES (?, ?, ?) ON CONFLICT(task_id, field_id) DO UPDATE SET value = excluded.value")
        .bind(task_id).bind(field_id).bind(value).execute(pool).await?;
    Ok(())
}

pub async fn get_task_field_values(pool: &Pool, task_id: i64) -> Result<Vec<TaskFieldValue>> {
    let rows: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT cv.field_id, cf.name, cf.field_type, cv.value \
         FROM task_custom_values cv JOIN custom_fields cf ON cv.field_id = cf.id \
         WHERE cv.task_id = ? ORDER BY cf.sort_order, cf.id")
        .bind(task_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(fid, fname, ftype, val)| TaskFieldValue { field_id: fid, field_name: fname, field_type: ftype, value: val }).collect())
}

pub async fn get_task_field_values_batch(pool: &Pool, task_ids: &[i64]) -> Result<Vec<(i64, TaskFieldValue)>> {
    if task_ids.is_empty() { return Ok(vec![]); }
    let ph = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT cv.task_id, cv.field_id, cf.name as field_name, cf.field_type, cv.value \
         FROM task_custom_values cv JOIN custom_fields cf ON cv.field_id = cf.id \
         WHERE cv.task_id IN ({}) ORDER BY cv.task_id, cf.sort_order, cf.id", ph);
    let mut q = sqlx::query_as::<_, (i64, i64, String, String, Option<String>)>(&sql);
    for id in task_ids { q = q.bind(id); }
    let rows = q.fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(tid, fid, fname, ftype, val)| {
        (tid, TaskFieldValue { field_id: fid, field_name: fname, field_type: ftype, value: val })
    }).collect())
}

pub async fn delete_task_field_value(pool: &Pool, task_id: i64, field_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM task_custom_values WHERE task_id = ? AND field_id = ?")
        .bind(task_id).bind(field_id).execute(pool).await?;
    Ok(())
}
