mod cli;
mod error;
mod models;
mod output;
mod steam;
mod store;

use clap::Parser;
use serde::Serialize;
use skillinstaller::rust_embed;
use skillinstaller::{
    InstallSkillArgs, install_interactive, load_embedded_skill, print_install_result,
};

use crate::cli::{
    AppArgs, Cli, Commands, DictSubcommands, OutputFormat, SearchArgs, UserOwnedArgs,
    UserSubcommands,
};
use crate::error::AppError;
use crate::models::{
    AppDetailsOut, DataSource, DictFindItem, DictItem, OwnedGame, SearchItem, TagFacet,
};
use crate::output::{build_pagination, clamp_limit, print_error, print_success};
use crate::store::{DictKind, LocalStore};

#[derive(Debug, Serialize)]
struct DictListData {
    items: Vec<DictItem>,
}

#[derive(Debug, Serialize)]
struct DictFindData {
    items: Vec<DictFindItem>,
}

#[derive(Debug, Serialize)]
struct SearchData {
    items: Vec<SearchItem>,
    facets: Option<FacetsData>,
}

#[derive(Debug, Serialize)]
struct FacetsData {
    tags: Vec<TagFacet>,
}

#[derive(Debug, Serialize)]
struct AppData {
    app: AppDetailsOut,
}

#[derive(Debug, Serialize)]
struct OwnedData {
    steamid: String,
    items: Vec<OwnedGame>,
}

#[derive(rust_embed::RustEmbed)]
#[folder = ".skill"]
struct SkillAssets;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let format = cli.resolved_format();

    let result = run(cli, format).await;
    if let Err(err) = result {
        print_error(format, err);
        std::process::exit(1);
    }
}

async fn run(cli: Cli, format: OutputFormat) -> Result<(), AppError> {
    let store = LocalStore::open()?;

    match cli.command {
        Commands::Tags(cmd) => handle_dict(format, &store, DictKind::Tags, cmd.action),
        Commands::Genres(cmd) => handle_dict(format, &store, DictKind::Genres, cmd.action),
        Commands::Categories(cmd) => handle_dict(format, &store, DictKind::Categories, cmd.action),
        Commands::Search(args) => handle_search(format, args).await,
        Commands::App(args) => handle_app(format, &store, args).await,
        Commands::User(cmd) => match cmd.action {
            UserSubcommands::Owned(args) => handle_user_owned(format, args).await,
        },
        Commands::InstallSkill(args) => handle_install_skill(args),
    }
}

fn handle_dict(
    format: OutputFormat,
    store: &LocalStore,
    kind: DictKind,
    action: DictSubcommands,
) -> Result<(), AppError> {
    store.ensure_seeded()?;

    match action {
        DictSubcommands::List(args) => {
            let limit = clamp_limit(args.limit);
            let offset = args.offset;
            let (items, total) = store.list_dict(kind, limit, offset)?;
            let pagination = build_pagination(limit, offset, items.len(), Some(total));
            let data = DictListData { items };

            print_success(
                format,
                data,
                Some(pagination),
                DataSource::LocalDb,
                false,
                |d| print_dict_list_human(kind, &d.items),
            );
            Ok(())
        }
        DictSubcommands::Find(args) => {
            if args.query.trim().is_empty() {
                return Err(AppError::InvalidArgument(
                    "query must not be empty".to_string(),
                ));
            }
            let limit = clamp_limit(args.paging.limit);
            let offset = args.paging.offset;
            let (items, total) = store.find_dict(kind, &args.query, limit, offset)?;
            let pagination = build_pagination(limit, offset, items.len(), Some(total));
            let data = DictFindData { items };

            print_success(
                format,
                data,
                Some(pagination),
                DataSource::LocalDb,
                false,
                |d| print_dict_find_human(kind, &args.query, &d.items),
            );
            Ok(())
        }
    }
}

async fn handle_search(format: OutputFormat, args: SearchArgs) -> Result<(), AppError> {
    let limit = clamp_limit(args.limit);
    let offset = args.offset;
    let tags = parse_tags_csv(&args.tags)?;

    let (items, facets) =
        steam::search_store(&tags, args.term.as_deref(), limit, offset, args.with_facets).await?;
    let original_len = items.len();
    let items = items.into_iter().take(limit).collect::<Vec<_>>();
    let mut pagination = build_pagination(limit, offset, items.len(), None);
    pagination.has_more = original_len > items.len() || pagination.has_more;

    let data = SearchData {
        items,
        facets: facets.map(|tags| FacetsData { tags }),
    };

    print_success(
        format,
        data,
        Some(pagination),
        DataSource::SteamStore,
        false,
        |d| print_search_human(&d.items, d.facets.as_ref()),
    );

    Ok(())
}

async fn handle_app(
    format: OutputFormat,
    store: &LocalStore,
    args: AppArgs,
) -> Result<(), AppError> {
    let now = now_unix();
    let min_ts = now.saturating_sub(args.ttl_sec.max(0));

    let (raw_json, cached) = if let Some(cached_raw) = store.get_cached_app(args.appid, min_ts)? {
        (cached_raw, true)
    } else {
        let fresh = steam::fetch_appdetails_json(args.appid).await?;
        store.put_cached_app(args.appid, &fresh, now)?;
        (fresh, false)
    };

    let app = steam::normalize_appdetails(args.appid, &raw_json)?;
    let data = AppData { app };

    print_success(format, data, None, DataSource::SteamStore, cached, |d| {
        print_app_human(&d.app)
    });

    Ok(())
}

async fn handle_user_owned(format: OutputFormat, args: UserOwnedArgs) -> Result<(), AppError> {
    let api_key = std::env::var("STEAM_API_KEY").map_err(|_| {
        AppError::Unauthorized("STEAM_API_KEY is required for user owned".to_string())
    })?;

    let steamid = match (args.steamid.as_deref(), args.vanity.as_deref()) {
        (Some(id), None) => id.to_string(),
        (None, Some(vanity)) => steam::resolve_vanity(&api_key, vanity).await?,
        (Some(_), Some(_)) => {
            return Err(AppError::InvalidArgument(
                "provide only one of --steamid or --vanity".to_string(),
            ));
        }
        (None, None) => {
            return Err(AppError::InvalidArgument(
                "provide --steamid or --vanity".to_string(),
            ));
        }
    };

    let mut items = steam::get_owned_games(&api_key, &steamid).await?;
    items.sort_by(|a, b| b.playtime_forever_min.cmp(&a.playtime_forever_min));

    let limit = clamp_limit(args.limit);
    let offset = args.offset.min(items.len());
    let total = items.len();
    let paged = items
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();

    let data = OwnedData {
        steamid,
        items: paged,
    };

    let pagination = build_pagination(limit, offset, data.items.len(), Some(total));

    print_success(
        format,
        data,
        Some(pagination),
        DataSource::SteamWebapi,
        false,
        |d| print_owned_human(&d.steamid, &d.items),
    );

    Ok(())
}

fn handle_install_skill(args: InstallSkillArgs) -> Result<(), AppError> {
    let source = load_embedded_skill::<SkillAssets>();

    let result = install_interactive(source, &args)?;

    print_install_result(&result);

    Ok(())
}

fn parse_tags_csv(input: &str) -> Result<Vec<i64>, AppError> {
    let mut out = Vec::new();
    for raw in input.split(',') {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value = trimmed
            .parse::<i64>()
            .map_err(|_| AppError::InvalidArgument(format!("invalid tag id '{trimmed}'")))?;
        out.push(value);
    }
    if out.is_empty() {
        return Err(AppError::InvalidArgument(
            "--tags must include at least one numeric tag id".to_string(),
        ));
    }
    Ok(out)
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn kind_name(kind: DictKind) -> &'static str {
    match kind {
        DictKind::Tags => "tags",
        DictKind::Genres => "genres",
        DictKind::Categories => "categories",
    }
}

fn print_dict_list_human(kind: DictKind, items: &[DictItem]) {
    println!("{} ({})", kind_name(kind), items.len());
    for item in items {
        println!("{}\t{}", item.id, item.name);
    }
}

fn print_dict_find_human(kind: DictKind, query: &str, items: &[DictFindItem]) {
    println!("{} find '{}' ({})", kind_name(kind), query, items.len());
    for item in items {
        println!("{}\t{}\t{:.4}", item.id, item.name, item.rank);
    }
}

fn print_search_human(items: &[SearchItem], facets: Option<&FacetsData>) {
    println!("search results ({})", items.len());
    for item in items {
        if let Some(price) = &item.price {
            println!("{}\t{}\t{}", item.appid, item.name, price);
        } else {
            println!("{}\t{}", item.appid, item.name);
        }
    }

    if let Some(f) = facets {
        println!("\nrelated tag facets ({})", f.tags.len());
        for tag in &f.tags {
            println!("{}\t{}\tselected={}", tag.tagid, tag.count, tag.selected);
        }
    }
}

fn print_app_human(app: &AppDetailsOut) {
    println!("{} ({})", app.name, app.appid);
    if let Some(desc) = &app.short_description {
        println!("{}", desc);
    }
    println!(
        "genres: {}",
        app.genres
            .iter()
            .map(|g| g.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "categories: {}",
        app.categories
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    );
}

fn print_owned_human(steamid: &str, games: &[OwnedGame]) {
    println!("owned games for {} ({})", steamid, games.len());
    for game in games {
        println!(
            "{}\t{}\t{}m",
            game.appid,
            game.name.clone().unwrap_or_else(|| "Unknown".to_string()),
            game.playtime_forever_min
        );
    }
}
