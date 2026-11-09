//! NocoDB 设备档案读写。
//!
//! 服务器地址、API Token、表 ID 目前按需求直接写在代码里（内部使用），
//! 后续如要支持多环境，再挪到 `config.toml` 的 `[nocodb]` 段。
//!
//! 表结构是「一台设备一行」的扁平表，字段由 NocoDB 决定，前端通过
//! `nocodb_columns` 动态渲染表单，所以新增列不需要改代码。

use crate::config::AppConfig;
use serde::Serialize;
use serde_json::{Map, Value};
use std::time::Duration;

/// NocoDB 服务地址
const BASE_URL: &str = "http://47.96.101.118:8080";
/// API Token（内部使用）
const TOKEN: &str = "nc_pat_tZ3CoCt0tSQPtZaIeygSOpzCW7BBcetWkI4IxTCa";
/// 设备信息表 ID（仅用于识别 SN 过滤与默认查询）
const DEVICES_TABLE: &str = "mibklfb9ybn96lx";
/// 设备关联的配置表
const LASER_TABLE: &str = "mmve483ctlcgjhu";
const MOTOR_TABLE: &str = "mexv8ok3duce7s0";
const GAIN_TABLE: &str = "ms96qf581i1us8x";
const CAPTURE_TABLE: &str = "mlpb8n3r84vrhid";
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
    let url = format!("{BASE_URL}/api/v2/meta/tables/{table}");
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

/// 列出指定表的记录；设备信息表支持按 SN 模糊搜索
#[tauri::command]
pub async fn nocodb_list(table: String, query: Option<String>) -> Result<Vec<NocoRecord>, String> {
    let is_devices = table == DEVICES_TABLE;
    let url = format!("{BASE_URL}/api/v2/tables/{table}/records");
    let mut req = client()?
        .get(url)
        .header("xc-token", TOKEN)
        .query(&[("limit", PAGE_SIZE.to_string())]);

    if is_devices {
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
                    !is_devices
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

/// 从 NocoDB 映射出的设备参数
#[derive(Debug, Clone, Default, Serialize)]
pub struct RemoteDeviceParams {
    pub laser_type: Option<String>,
    pub laser_info: Option<String>,
    pub laser_current_ma: Option<u32>,
    pub laser_freq_hz: Option<u32>,
    pub motor_speed_rpm: Option<u32>,
    pub gain_pmt0: Option<f64>,
    pub gain_pmt1: Option<f64>,
    pub gain_pmt2: Option<f64>,
    pub gain_apd: Option<f64>,
    pub first_pos: Option<u32>,
    pub first_len: Option<u32>,
    pub second_pos: Option<u32>,
    pub second_len: Option<u32>,
    pub sum_value: Option<u32>,
    pub max_value: Option<u32>,
}

impl RemoteDeviceParams {
    /// 是否至少包含一个有效配置项
    pub fn has_data(&self) -> bool {
        self.laser_type.is_some()
            || self.laser_current_ma.is_some()
            || self.laser_freq_hz.is_some()
            || self.motor_speed_rpm.is_some()
            || self.gain_pmt0.is_some()
            || self.gain_apd.is_some()
            || self.first_pos.is_some()
            || self.first_len.is_some()
            || self.second_pos.is_some()
            || self.second_len.is_some()
            || self.sum_value.is_some()
            || self.max_value.is_some()
    }

    /// 与本地配置比较，判断服务器参数是否发生变化。
    ///
    /// 仅比较服务器实际提供的字段，未提供的字段视为不变化。
    pub fn differs_from(&self, cfg: &AppConfig) -> bool {
        self.laser_type
            .as_deref()
            .map(|v| v != cfg.radar.laser_type)
            .unwrap_or(false)
            || self
                .laser_info
                .as_deref()
                .map(|v| v != cfg.radar.laser_info)
                .unwrap_or(false)
            || self
                .laser_current_ma
                .map(|v| v != cfg.laser.current_ma)
                .unwrap_or(false)
            || self
                .laser_freq_hz
                .map(|v| v != cfg.laser.freq_hz)
                .unwrap_or(false)
            || self
                .motor_speed_rpm
                .map(|v| v != cfg.motor.speed_rpm)
                .unwrap_or(false)
            || self
                .gain_pmt0
                .map(|v| v != cfg.gain.pmt0)
                .unwrap_or(false)
            || self
                .gain_pmt1
                .map(|v| v != cfg.gain.pmt1)
                .unwrap_or(false)
            || self
                .gain_pmt2
                .map(|v| v != cfg.gain.pmt2)
                .unwrap_or(false)
            || self.gain_apd.map(|v| v != cfg.gain.apd).unwrap_or(false)
            || self
                .first_pos
                .map(|v| v != cfg.capture.first_pos)
                .unwrap_or(false)
            || self
                .first_len
                .map(|v| v != cfg.capture.first_len)
                .unwrap_or(false)
            || self
                .second_pos
                .map(|v| v != cfg.capture.second_pos)
                .unwrap_or(false)
            || self
                .second_len
                .map(|v| v != cfg.capture.second_len)
                .unwrap_or(false)
            || self
                .sum_value
                .map(|v| v != cfg.capture.sum_value)
                .unwrap_or(false)
            || self
                .max_value
                .map(|v| v != cfg.capture.max_value)
                .unwrap_or(false)
    }

    /// 覆盖到完整配置上，未提供的字段保持原值
    pub fn apply_to(&self, mut cfg: AppConfig) -> AppConfig {
        if let Some(v) = &self.laser_type {
            cfg.radar.laser_type = v.clone();
        }
        if let Some(v) = &self.laser_info {
            cfg.radar.laser_info = v.clone();
        }
        if let Some(v) = self.laser_current_ma {
            cfg.laser.current_ma = v;
        }
        if let Some(v) = self.laser_freq_hz {
            cfg.laser.freq_hz = v;
        }
        if let Some(v) = self.motor_speed_rpm {
            cfg.motor.speed_rpm = v;
        }
        if let Some(v) = self.gain_pmt0 {
            cfg.gain.pmt0 = v;
        }
        if let Some(v) = self.gain_pmt1 {
            cfg.gain.pmt1 = v;
        }
        if let Some(v) = self.gain_pmt2 {
            cfg.gain.pmt2 = v;
        }
        if let Some(v) = self.gain_apd {
            cfg.gain.apd = v;
        }
        if let Some(v) = self.first_pos {
            cfg.capture.first_pos = v;
        }
        if let Some(v) = self.first_len {
            cfg.capture.first_len = v;
        }
        if let Some(v) = self.second_pos {
            cfg.capture.second_pos = v;
        }
        if let Some(v) = self.second_len {
            cfg.capture.second_len = v;
        }
        if let Some(v) = self.sum_value {
            cfg.capture.sum_value = v;
        }
        if let Some(v) = self.max_value {
            cfg.capture.max_value = v;
        }
        cfg
    }
}

fn related_first<'a>(
    values: &'a Map<String, Value>,
    key: &str,
) -> Option<&'a Map<String, Value>> {
    values.get(key)?.as_array()?.first()?.as_object()
}

/// 关联字段里只返回了主显示列（如型号），需要按 Id 再查一次子表
fn related_id(values: &Map<String, Value>, key: &str) -> Option<u64> {
    related_first(values, key)?.get("Id")?.as_u64()
}

async fn fetch_related(table: &str, id: u64) -> Result<Option<Map<String, Value>>, String> {
    let records = nocodb_list(table.to_string(), None).await?;
    Ok(records
        .into_iter()
        .find(|r| r.id == id)
        .map(|r| r.values))
}

fn as_u32(obj: &Map<String, Value>, key: &str) -> Option<u32> {
    let v = obj.get(key)?;
    v.as_u64()
        .map(|n| n as u32)
        .or_else(|| v.as_f64().map(|n| n as u32))
}

fn as_f64(obj: &Map<String, Value>, key: &str) -> Option<f64> {
    let v = obj.get(key)?;
    v.as_f64().or_else(|| v.as_u64().map(|n| n as f64))
}

fn as_text(obj: &Map<String, Value>, key: &str) -> Option<String> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// NocoDB 中的激光器型号 → 上位机协议类型
pub fn map_laser_type(model: &str) -> String {
    if model.to_lowercase().contains("5k") {
        "laser_5kHz".to_string()
    } else {
        "laser_connet".to_string()
    }
}

/// 按 SN 从 NocoDB 读取设备参数。
///
/// 网络不可用时返回 Err；网络正常但找不到该 SN 时返回 Ok(None)。
pub async fn fetch_device_params(sn: &str) -> Result<Option<RemoteDeviceParams>, String> {
    let records = nocodb_list(DEVICES_TABLE.to_string(), Some(sn.to_string())).await?;
    let Some(record) = records.into_iter().find(|r| {
        r.values.get(SN_COLUMN).and_then(|v| v.as_str()).map(str::trim) == Some(sn.trim())
    }) else {
        return Ok(None);
    };

    let mut params = RemoteDeviceParams::default();

    if let Some(id) = related_id(&record.values, "激光器") {
        if let Some(laser) = fetch_related(LASER_TABLE, id).await? {
            if let Some(model) = as_text(&laser, "型号") {
                params.laser_type = Some(map_laser_type(&model));
                params.laser_info = Some(model);
            }
            params.laser_current_ma = as_u32(&laser, "电流");
            params.laser_freq_hz = as_u32(&laser, "频率");
        }
    }

    if let Some(id) = related_id(&record.values, "电机") {
        if let Some(motor) = fetch_related(MOTOR_TABLE, id).await? {
            params.motor_speed_rpm = as_u32(&motor, "转速");
        }
    }

    if let Some(id) = related_id(&record.values, "增益") {
        if let Some(gain) = fetch_related(GAIN_TABLE, id).await? {
            // 列名为 PMT0/1/2 时直接对应；否则兼容 PMT1/2/3
            if gain.contains_key("PMT0") {
                params.gain_pmt0 = as_f64(&gain, "PMT0");
                params.gain_pmt1 = as_f64(&gain, "PMT1");
                params.gain_pmt2 = as_f64(&gain, "PMT2");
            } else {
                params.gain_pmt0 = as_f64(&gain, "PMT1");
                params.gain_pmt1 = as_f64(&gain, "PMT2");
                params.gain_pmt2 = as_f64(&gain, "PMT3");
            }
            params.gain_apd = as_f64(&gain, "APD");
        }
    }

    if let Some(id) = related_id(&record.values, "采集参数") {
        if let Some(cap) = fetch_related(CAPTURE_TABLE, id).await? {
            params.first_pos = as_u32(&cap, "第一段位置");
            params.first_len = as_u32(&cap, "第一段长度");
            params.second_pos = as_u32(&cap, "第二段位置");
            params.second_len = as_u32(&cap, "第二段长度");
            params.sum_value = as_u32(&cap, "和阈值");
            params.max_value = as_u32(&cap, "值阈值");
        }
    }

    Ok(Some(params))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实网络测试，默认忽略：`cargo test -- --ignored nocodb`
    #[tokio::test]
    #[ignore]
    async fn test_nocodb_readonly() {
        let columns = nocodb_columns(DEVICES_TABLE.into())
            .await
            .expect("读取字段失败");
        assert!(!columns.is_empty(), "表结构为空");
        eprintln!(
            "设备表字段: {:?}",
            columns.iter().map(|c| c.title.clone()).collect::<Vec<_>>()
        );
        assert!(
            columns.iter().any(|c| c.uidt == "LinkToAnotherRecord"),
            "设备表应有激光器/电机关联字段"
        );

        // 逐个加载关联表，验证多关联（激光器/电机/增益/采集参数）
        for col in columns.iter().filter(|c| c.uidt == "LinkToAnotherRecord") {
            let Some(rel) = col.related_table_id.as_deref() else {
                continue;
            };
            let rows = nocodb_list(rel.to_string(), None)
                .await
                .unwrap_or_else(|e| panic!("读取关联表 {rel} 失败: {e}"));
            let _cols = nocodb_columns(rel.to_string()).await.expect("读取关联表字段失败");
            eprintln!("关联字段「{}」 → {} 条记录", col.title, rows.len());
        }

        let list = nocodb_list(DEVICES_TABLE.into(), None).await.expect("列表失败");
        eprintln!("记录数: {}", list.len());
        assert!(!list.is_empty(), "nocodb_list 返回空列表");
    }




}
