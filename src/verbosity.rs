pub use clap_verbosity_flag::{ErrorLevel, LogLevel, WarnLevel};

#[derive(clap::Args, Debug, Clone)]
#[group(skip)]
pub struct Verbosity<L: LogLevel = ErrorLevel> {
    #[command(flatten)]
    inner: clap_verbosity_flag::Verbosity<L>,
}

impl<L: LogLevel> Verbosity<L> {
    pub fn level_filter(&self) -> tracing_subscriber::filter::LevelFilter {
        convert_filter(self.inner.log_level_filter())
    }
}

fn convert_filter(filter: log::LevelFilter) -> tracing_subscriber::filter::LevelFilter {
    match filter {
        log::LevelFilter::Off => tracing_subscriber::filter::LevelFilter::OFF,
        log::LevelFilter::Error => tracing_subscriber::filter::LevelFilter::ERROR,
        log::LevelFilter::Warn => tracing_subscriber::filter::LevelFilter::WARN,
        log::LevelFilter::Info => tracing_subscriber::filter::LevelFilter::INFO,
        log::LevelFilter::Debug => tracing_subscriber::filter::LevelFilter::DEBUG,
        log::LevelFilter::Trace => tracing_subscriber::filter::LevelFilter::TRACE,
    }
}
