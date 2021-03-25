use crate::{vanity, Phrase};
use clap::Clap;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::RwLock;
use tokio::time::error::Elapsed;
use tokio::time::{timeout, Duration, Instant};

#[derive(Clap)]
pub struct VanityOpts {
    #[clap(subcommand)]
    secret_type: VanitySecretType,
}

impl VanityOpts {
    pub async fn handle(&self) -> anyhow::Result<()> {
        let (secret_type, opts) = match &self.secret_type {
            VanitySecretType::Phrase(phrase) => {
                let secret_type = vanity::SecretType::Phrase {
                    language: phrase.phrase_opts.language.language.to_owned(),
                    words: phrase.phrase_opts.words.0,
                };
                (secret_type, &phrase.common_opts)
            }
            VanitySecretType::Seed(seed) => (vanity::SecretType::Seed, &seed.common_opts),
            VanitySecretType::Private(private) => {
                (vanity::SecretType::Private, &private.common_opts)
            }
        };

        let matches = if opts.start {
            vanity::Match::start(&opts.matching)
        } else if opts.end {
            vanity::Match::end(&opts.matching)
        } else if opts.regex {
            vanity::Match::regex(&opts.matching)?
        } else {
            vanity::Match::start_or_end(&opts.matching)
        };

        let vanity = vanity::Vanity::new(secret_type, matches);
        let (mut rx, counter) = vanity.start().await?;
        let started = Instant::now();
        let mut last_log = Instant::now();
        loop {
            match timeout(Duration::from_millis(100), rx.recv()).await {
                Ok(Some(result)) => {
                    println!("{},{:?}", result.address.to_string(), result.secret);
                    last_log = log(started, last_log, counter.clone()).await;
                }
                // Channel closed
                Ok(None) => {
                    break;
                }
                // Timeout
                Err(_) => {
                    last_log = log(started, last_log, counter.clone()).await;
                }
            }
        }
        Ok(())
    }
}

async fn log(started: Instant, last_log: Instant, counter: Arc<RwLock<usize>>) -> Instant {
    let c = *counter.read().await;
    let now = Instant::now();
    let since_last_log = now.duration_since(last_log);
    if since_last_log < Duration::from_secs(1) {
        last_log
    } else {
        let total_taken = Instant::now().duration_since(started);
        let rate = (c as f64) / total_taken.as_secs_f64();
        eprintln!("Attempted: {}, Rate: {:?} attempts/s", c, rate);
        now
    }
}

#[derive(Clap)]
enum VanitySecretType {
    Phrase(PhraseOpts),
    Seed(SeedOpts),
    Private(PrivateOpts),
}

#[derive(Clap)]
struct PhraseOpts {
    #[clap(flatten)]
    pub phrase_opts: super::phrase::New,

    #[clap(flatten)]
    pub common_opts: CommonOpts,
}

#[derive(Clap)]
struct SeedOpts {
    #[clap(flatten)]
    pub common_opts: CommonOpts,
}

#[derive(Clap)]
struct PrivateOpts {
    #[clap(flatten)]
    pub common_opts: CommonOpts,
}

#[derive(Clap)]
struct CommonOpts {
    /// Match on this string. By default will match the start and end.
    matching: String,

    /// Match on start only. Default is start and end.
    #[clap(short, long, group = "match")]
    start: bool,

    /// Match on end only. Default is start and end.
    #[clap(short, long, group = "match")]
    end: bool,

    /// Match on a regular expression instead.
    #[clap(short, long, group = "match")]
    regex: bool,

    // TODO
    /// Number of parallel tasks to use. Default: Your logical processors minus one, or at least 1.
    #[clap(short, long)]
    tasks: Option<usize>,

    /// Stop after finding this many matches.
    #[clap(short, long, default_value = "1")]
    limit: usize,

    /// Quit after this many attempts
    #[clap(short, long)]
    quit: Option<usize>,
}
