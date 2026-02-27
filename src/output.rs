use serde::Serialize;

use crate::cli::OutputFormat;
use crate::error::AppError;
use crate::models::{DataSource, Envelope, ErrorBody, Meta, Pagination};

pub fn clamp_limit(limit: usize) -> usize {
    limit.clamp(1, 100)
}

pub fn build_pagination(
    limit: usize,
    offset: usize,
    returned: usize,
    total: Option<usize>,
) -> Pagination {
    let has_more = match total {
        Some(t) => offset.saturating_add(returned) < t,
        None => returned == limit,
    };
    Pagination {
        limit,
        offset,
        returned,
        has_more,
        total,
    }
}

pub fn print_success<T: Serialize>(
    format: OutputFormat,
    data: T,
    pagination: Option<Pagination>,
    source: DataSource,
    cached: bool,
    human: impl FnOnce(&T),
) {
    match format {
        OutputFormat::Human => human(&data),
        OutputFormat::Json => {
            let envelope = Envelope {
                ok: true,
                data: Some(data),
                pagination,
                meta: Meta {
                    version: "1.0.0",
                    source,
                    cached,
                },
                error: None,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| "{}".to_string())
            );
        }
    }
}

pub fn print_error(format: OutputFormat, error: AppError) {
    match format {
        OutputFormat::Human => {
            eprintln!("Error [{}]: {}", error.code(), error);
        }
        OutputFormat::Json => {
            let envelope: Envelope<serde_json::Value> = Envelope {
                ok: false,
                data: None,
                pagination: None,
                meta: Meta {
                    version: "1.0.0",
                    source: DataSource::Internal,
                    cached: false,
                },
                error: Some(ErrorBody {
                    code: error.code(),
                    message: error.to_string(),
                }),
            };
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| "{}".to_string())
            );
        }
    }
}
