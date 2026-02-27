use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    LocalDb,
    SteamStore,
    SteamWebapi,
    Internal,
}

#[derive(Debug, Serialize)]
pub struct Pagination {
    pub limit: usize,
    pub offset: usize,
    pub returned: usize,
    pub has_more: bool,
    pub total: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub version: &'static str,
    pub source: DataSource,
    pub cached: bool,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct Envelope<T: Serialize> {
    pub ok: bool,
    pub data: Option<T>,
    pub pagination: Option<Pagination>,
    pub meta: Meta,
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DictItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DictFindItem {
    pub id: String,
    pub name: String,
    pub rank: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchItem {
    pub appid: i64,
    pub name: String,
    pub price: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagFacet {
    pub tagid: i64,
    pub count: i64,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppDetailsOut {
    pub appid: i64,
    pub name: String,
    pub short_description: Option<String>,
    pub categories: Vec<DictItem>,
    pub genres: Vec<DictItem>,
    pub supported_languages: Option<String>,
    pub platforms: serde_json::Value,
    pub release_date: Option<String>,
    pub price_overview: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OwnedGame {
    pub appid: i64,
    pub name: Option<String>,
    pub playtime_forever_min: i64,
    pub playtime_2weeks_min: i64,
}
