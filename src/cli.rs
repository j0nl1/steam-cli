use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use skillinstaller::InstallSkillArgs;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormatArg {
    Human,
    Json,
}

#[derive(Debug, Parser)]
#[command(
    name = "steam-cli",
    version,
    about = "Steam CLI local for search/detail/user signals"
)]
pub struct Cli {
    #[arg(long, global = true, value_enum, default_value_t = OutputFormatArg::Human)]
    pub format: OutputFormatArg,
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Tags(DictCommand),
    Genres(DictCommand),
    Categories(DictCommand),
    Search(SearchArgs),
    App(AppArgs),
    User(UserCommand),
    InstallSkill(InstallSkillArgs),
}

#[derive(Debug, Args)]
pub struct DictPagingArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
}

#[derive(Debug, Subcommand)]
pub enum DictSubcommands {
    List(DictPagingArgs),
    Find(FindArgs),
}

#[derive(Debug, Args)]
pub struct FindArgs {
    pub query: String,
    #[command(flatten)]
    pub paging: DictPagingArgs,
}

#[derive(Debug, Args)]
pub struct DictCommand {
    #[command(subcommand)]
    pub action: DictSubcommands,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[arg(long)]
    pub tags: String,
    #[arg(long)]
    pub term: Option<String>,
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
    #[arg(long, default_value_t = false)]
    pub with_facets: bool,
}

#[derive(Debug, Args)]
pub struct AppArgs {
    pub appid: i64,
    #[arg(long, default_value_t = 86_400)]
    pub ttl_sec: i64,
}

#[derive(Debug, Args)]
pub struct UserOwnedArgs {
    #[arg(long)]
    pub steamid: Option<String>,
    #[arg(long)]
    pub vanity: Option<String>,
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
}

#[derive(Debug, Subcommand)]
pub enum UserSubcommands {
    Owned(UserOwnedArgs),
}

#[derive(Debug, Args)]
pub struct UserCommand {
    #[command(subcommand)]
    pub action: UserSubcommands,
}

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Human,
    Json,
}

impl Cli {
    pub fn resolved_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            match self.format {
                OutputFormatArg::Human => OutputFormat::Human,
                OutputFormatArg::Json => OutputFormat::Json,
            }
        }
    }
}
