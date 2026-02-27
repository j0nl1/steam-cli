use regex::Regex;
use scraper::{Html, Selector};
use serde_json::Value;
use url::Url;

use crate::error::AppError;
use crate::models::{AppDetailsOut, DictItem, OwnedGame, SearchItem, TagFacet};

pub async fn search_store(
    tags: &[i64],
    term: Option<&str>,
    limit: usize,
    offset: usize,
    with_facets: bool,
) -> Result<(Vec<SearchItem>, Option<Vec<TagFacet>>), AppError> {
    let mut url = Url::parse("https://store.steampowered.com/search/results")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("force_infinite", "1");
        qp.append_pair(
            "tags",
            &tags
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
        qp.append_pair("supportedlang", "english");
        qp.append_pair("ndl", "1");
        qp.append_pair("start", &offset.to_string());
        qp.append_pair("count", &limit.to_string());
        if let Some(t) = term {
            qp.append_pair("term", t);
        }
    }

    let html_text = reqwest::Client::new().get(url).send().await?.text().await?;
    parse_search_html(&html_text, tags, with_facets)
}

pub fn parse_search_html(
    html_text: &str,
    selected_tags: &[i64],
    with_facets: bool,
) -> Result<(Vec<SearchItem>, Option<Vec<TagFacet>>), AppError> {
    let document = Html::parse_document(html_text);
    let row_sel = Selector::parse("a.search_result_row")
        .map_err(|e| AppError::Internal(format!("selector parse: {e}")))?;
    let title_sel = Selector::parse("span.title")
        .map_err(|e| AppError::Internal(format!("selector parse: {e}")))?;
    let price_sel = Selector::parse("div.discount_final_price, div.search_price")
        .map_err(|e| AppError::Internal(format!("selector parse: {e}")))?;

    let mut items = Vec::new();
    for row in document.select(&row_sel) {
        let Some(appid_raw) = row.value().attr("data-ds-appid") else {
            continue;
        };
        let Ok(appid) = appid_raw.parse::<i64>() else {
            continue;
        };

        let name = row
            .select(&title_sel)
            .next()
            .map(|n| n.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Unknown".to_string());

        let price = row
            .select(&price_sel)
            .next()
            .map(|n| {
                n.text()
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|s| !s.is_empty());

        items.push(SearchItem { appid, name, price });
    }

    if items.is_empty() {
        return Err(AppError::UpstreamSchema(
            "no search_result_row entries found in store response".to_string(),
        ));
    }

    let facets = if with_facets {
        Some(parse_tag_facets(html_text, selected_tags)?)
    } else {
        None
    };

    Ok((items, facets))
}

fn parse_tag_facets(html_text: &str, selected_tags: &[i64]) -> Result<Vec<TagFacet>, AppError> {
    let re = Regex::new(r"PopulateTagFacetData\(\s*(\[[^\)]*\])\s*,\s*(\[[^\)]*\])")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let caps = re.captures(html_text).ok_or_else(|| {
        AppError::UpstreamSchema("facets block not found in search HTML".to_string())
    })?;

    let raw_pairs = caps
        .get(1)
        .ok_or_else(|| AppError::UpstreamSchema("facet pairs missing".to_string()))?
        .as_str();

    let parsed: Vec<Vec<Value>> = serde_json::from_str(raw_pairs)
        .map_err(|e| AppError::UpstreamSchema(format!("facet parse failed: {e}")))?;

    let out = parsed
        .into_iter()
        .filter_map(|pair| {
            if pair.len() != 2 {
                return None;
            }
            let tagid = value_to_i64(&pair[0])?;
            let count = value_to_i64(&pair[1])?;
            Some(TagFacet {
                tagid,
                count,
                selected: selected_tags.contains(&tagid),
            })
        })
        .collect::<Vec<_>>();

    Ok(out)
}

fn value_to_i64(v: &Value) -> Option<i64> {
    if let Some(i) = v.as_i64() {
        return Some(i);
    }
    if let Some(s) = v.as_str() {
        return s.parse::<i64>().ok();
    }
    None
}

pub async fn fetch_appdetails_json(appid: i64) -> Result<String, AppError> {
    let url = format!("https://store.steampowered.com/api/appdetails?appids={appid}&l=english");
    let text = reqwest::Client::new().get(url).send().await?.text().await?;
    Ok(text)
}

pub fn normalize_appdetails(appid: i64, raw_json: &str) -> Result<AppDetailsOut, AppError> {
    let root: Value =
        serde_json::from_str(raw_json).map_err(|e| AppError::UpstreamSchema(e.to_string()))?;
    let obj = root
        .get(appid.to_string())
        .ok_or_else(|| AppError::UpstreamSchema("appid key missing in appdetails".to_string()))?;

    let success = obj
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !success {
        return Err(AppError::NotFound(format!("appid {appid} not found")));
    }

    let data = obj
        .get("data")
        .ok_or_else(|| AppError::UpstreamSchema("appdetails data missing".to_string()))?;

    let name = data
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let categories = parse_id_description_list(data.get("categories"));
    let genres = parse_id_description_list(data.get("genres"));

    let out = AppDetailsOut {
        appid,
        name,
        short_description: data
            .get("short_description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        categories,
        genres,
        supported_languages: data
            .get("supported_languages")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        platforms: data.get("platforms").cloned().unwrap_or(Value::Null),
        release_date: data
            .get("release_date")
            .and_then(|v| v.get("date"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        price_overview: data.get("price_overview").cloned(),
    };

    Ok(out)
}

fn parse_id_description_list(value: Option<&Value>) -> Vec<DictItem> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_i64()?.to_string();
            let name = item.get("description")?.as_str()?.to_string();
            Some(DictItem { id, name })
        })
        .collect()
}

pub async fn resolve_vanity(api_key: &str, vanity: &str) -> Result<String, AppError> {
    let mut url = Url::parse("https://api.steampowered.com/ISteamUser/ResolveVanityURL/v1/")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("key", api_key);
        qp.append_pair("vanityurl", vanity);
    }

    let json: Value = reqwest::Client::new().get(url).send().await?.json().await?;
    let response = json
        .get("response")
        .ok_or_else(|| AppError::UpstreamSchema("resolve vanity response missing".to_string()))?;

    if response.get("success").and_then(|v| v.as_i64()) != Some(1) {
        return Err(AppError::NotFound(format!("vanity '{vanity}' not found")));
    }

    let steamid = response
        .get("steamid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::UpstreamSchema("steamid missing in vanity response".to_string())
        })?;

    Ok(steamid.to_string())
}

pub async fn get_owned_games(api_key: &str, steamid: &str) -> Result<Vec<OwnedGame>, AppError> {
    let mut url = Url::parse("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("key", api_key);
        qp.append_pair("steamid", steamid);
        qp.append_pair("include_appinfo", "1");
        qp.append_pair("include_played_free_games", "1");
        qp.append_pair("format", "json");
    }

    let json: Value = reqwest::Client::new().get(url).send().await?.json().await?;
    let games = json
        .get("response")
        .and_then(|r| r.get("games"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::UpstreamSchema("owned games array missing".to_string()))?;

    let mut out = Vec::with_capacity(games.len());
    for g in games {
        let appid = g.get("appid").and_then(|v| v.as_i64()).unwrap_or_default();
        if appid == 0 {
            continue;
        }

        out.push(OwnedGame {
            appid,
            name: g
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            playtime_forever_min: g
                .get("playtime_forever")
                .and_then(|v| v.as_i64())
                .unwrap_or_default(),
            playtime_2weeks_min: g
                .get("playtime_2weeks")
                .and_then(|v| v.as_i64())
                .unwrap_or_default(),
        });
    }

    Ok(out)
}
