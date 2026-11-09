//! NocoDB 设备档案读写。
//!
//! 服务器地址、API Token、表 ID 目前按需求直接写在代码里（内部使用），
//! 后续如要支持多环境，再挪到 `config.toml` 的 `[nocodb]` 段。
//!
//! 表结构是「一台设备一行」的扁平表，字段由 NocoDB 决定，前端通过
//! `nocodb_columns` 动态渲染表单，所以新增列不需要改代码。

use serde::Serialize;
use serde_json::{Map, Value};
use std::time::Duration;

/// NocoDB 服务地址
const BASE_URL: &str = "http://47.96.101.118:8080";
/// API Token（内部使用）
const TOKEN: &str = "nc_pat_tZ3CoCt0tSQPtZaIeygSOpzCW7BBcetWkI4IxTCa";
/// 设备信息表 ID
const DEVICES_TABLE: &str = "mibklfb9ybn96lx";
/// 激光器表 ID
const LASERS_TABLE: &str = "mmve483ctlcgjhu";
/// 电机表 ID
const MOTORS_TABLE: &str = "mexv8ok3duce7s0";
/// 设备 SN 所在列（用于查询与去重）
const SN_COLUMN: &str = "SN";
/// 请求超时
const TIMEOUT: Duration = Duration::from_secs(6);
/// 单次最多拉取记录数
const PAGE_SIZE: u32 = 200;

/// 系统列的类型，不参与表单渲染
const SYSTEM_UIDT: [&str; 10] = [
    "ID",
    "CreatedTime",
    "LastModifiedTime",
    "CreatedBy",
    "LastModifiedBy",
    "Order",
    "Deleted",
    "Formula",
    "Rollup",
    "Lookup",
];

/// 表字段（动态展示用）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NocoColumn {
    /// 列标题（也是记录里的 JSON key）
    pub title: String,
    /// NocoDB 字段类型，如 SingleLineText / LongText / Number / LinkToAnotherRecord
    pub uidt: String,
    /// 是否必填
    pub required: bool,
    /// 单选/多选的可选值
    pub options: Vec<String>,
    /// 关联字段指向的表 ID（仅 LinkToAnotherRecord 有值）
    pub related_table_id: Option<String>,
}

/// 逻辑表名 → NocoDB 表 ID
fn table_id(table: &str) -> Result<&'static str, String> {
    match table {
        "devices" => Ok(DEVICES_TABLE),
        "lasers" => Ok(LASERS_TABLE),
        "motors" => Ok(MOTORS_TABLE),
        other => Err(format!("未知的数据表: {other}")),
    }
}

/// NocoDB 自动生成的关联中间表列名，需要在结果里忽略
fn is_junction_column(title: &str) -> bool {
    title.contains("nc_m2m_")
}

/// 一条设备档案
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NocoRecord {
    /// NocoDB 行 ID
    pub id: u64,
    /// 各字段值（已剔除系统列）
    pub values: Map<String, Value>,
    /// 最后更新时间（NocoDB 自动维护）
    pub updated_at: Option<String>,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

/// 统一处理响应：非 2xx 时把服务端返回体带出来
async fn parse_response(resp: reqwest::Response, action: &str) -> Result<Value, String> {
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("{action} 读取响应失败: {e}"))?;

    if !status.is_success() {
        return Err(format!("{action} 失败（HTTP {}）: {}", status.as_u16(), text.trim()));
    }

    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| format!("{action} 响应解析失败: {e}"))
}

/// 读取指定表的字段定义（忽略系统列与关联中间表列）
#[tauri::command]
pub async fn nocodb_columns(table: String) -> Result<Vec<NocoColumn>, String> {
    let table_id = table_id(&table)?;
    let url = format!("{BASE_URL}/api/v2/meta/tables/{table_id}");
    let resp = client()?
        .get(url)
        .header("xc-token", TOKEN)
        .send()
        .await
        .map_err(|e| format!("获取表结构失败: {e}"))?;
    let body = parse_response(resp, "获取表结构").await?;

    let mut columns = Vec::new();
    for col in body
        .get("columns")
        .and_then(|v| v.as_array())
        .map(|v| v.as_slice())
        .unwrap_or_default()
    {
        let uidt = col.get("uidt").and_then(|v| v.as_str()).unwrap_or("");
        if SYSTEM_UIDT.contains(&uidt) {
            continue;
        }
        let title = col.get("title").and_then(|v| v.as_str()).unwrap_or("");
        if title.is_empty() || is_junction_column(title) {
            continue;
        }
        let options = col
            .get("colOptions")
            .and_then(|v| v.get("options"))
            .and_then(|v| v.as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|o| o.get("title").and_then(|v| v.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let related_table_id = col
            .get("colOptions")
            .and_then(|v| v.get("fk_related_model_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        columns.push(NocoColumn {
            title: title.to_string(),
            uidt: uidt.to_string(),
            required: col.get("rqd").and_then(|v| v.as_bool()).unwrap_or(false),
            options,
            related_table_id,
        });
    }

    Ok(columns)
}

/// 把 NocoDB 返回的一行转换成 `NocoRecord`（剔除系统列）
fn to_record(value: &Value) -> Option<NocoRecord> {
    let obj = value.as_object()?;
    let id = obj.get("Id").and_then(|v| v.as_u64())?;
    let updated_at = obj
        .get("UpdatedAt")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut values = Map::new();
    for (k, v) in obj {
        if matches!(
            k.as_str(),
            "Id" | "CreatedAt" | "UpdatedAt" | "nc_created_by" | "nc_updated_by" | "nc_order"
                | "__nc_deleted"
        ) || is_junction_column(k)
        {
            continue;
        }
        values.insert(k.clone(), v.clone());
    }

    Some(NocoRecord {
        id,
        values,
        updated_at,
    })
}

/// 列出指定表的记录；devices 表支持按 SN 模糊搜索
#[tauri::command]
pub async fn nocodb_list(table: String, query: Option<String>) -> Result<Vec<NocoRecord>, String> {
    let table_id = table_id(&table)?;
    let url = format!("{BASE_URL}/api/v2/tables/{table_id}/records");
    let mut req = client()?
        .get(url)
        .header("xc-token", TOKEN)
        .query(&[("limit", PAGE_SIZE.to_string())]);

    if table == "devices" {
        if let Some(q) = query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            req = req.query(&[("where", format!("({SN_COLUMN},like,%{q}%)"))]);
        }
    }

    let resp = req.send().await.map_err(|e| format!("查询设备档案失败: {e}"))?;
    let body = parse_response(resp, "查询设备档案").await?;

    let records = body
        .get("list")
        .and_then(|v| v.as_array())
        .map(|list| {
            list.iter()
                .filter_map(to_record)
                // devices 表过滤掉手工新增的空行（SN 为空）
                .filter(|r| {
                    table != "devices"
                        || r.values
                            .get(SN_COLUMN)
                            .and_then(|v| v.as_str())
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实网络测试，默认忽略：`cargo test -- --ignored nocodb`
    #[tokio::test]
    #[ignore]
    async fn test_nocodb_readonly() {
        let columns = nocodb_columns("devices".into()).await.expect("读取字段失败");
        assert!(!columns.is_empty(), "表结构为空");
        eprintln!(
            "设备表字段: {:?}",
            columns.iter().map(|c| c.title.clone()).collect::<Vec<_>>()
        );
        assert!(
            columns.iter().any(|c| c.uidt == "LinkToAnotherRecord"),
            "设备表应有激光器/电机关联字段"
        );

        let lasers = nocodb_list("lasers".into(), None).await.expect("激光器列表失败");
        let motors = nocodb_list("motors".into(), None).await.expect("电机列表失败");
        eprintln!("激光器 {} 条 / 电机 {} 条", lasers.len(), motors.len());

        let list = nocodb_list("devices".into(), None).await.expect("列表失败");
        eprintln!("记录数: {}", list.len());
        assert!(!list.is_empty(), "nocodb_list 返回空列表");
    }
}
