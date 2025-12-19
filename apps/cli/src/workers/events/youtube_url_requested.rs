use std::{path::PathBuf, time::SystemTime};

use bratishka_core::events::Event;
use uuid::Uuid;

use crate::{pipeline_old::ensure_model, provider::Provider, workers::events::EventHeader};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct JobSpec {
    pub url: String,
    pub force: bool,
    pub provider: Provider,
    pub requested_report_lang: Option<String>,

    // pure derived values
    pub root_cache_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub model_path: PathBuf,
}

impl JobSpec {
    pub async fn from_cli(cli: crate::Cli) -> anyhow::Result<Self> {
        let provider: Provider = cli.provider.into();

        let root_cache_dir = crate::cache::get_root_cache_dir();
        let cache_dir = crate::cache::get_cache_dir(&cli.url);
        std::fs::create_dir_all(&cache_dir)?;
        let model_path = ensure_model(&root_cache_dir).await?;

        Ok(Self {
            url: cli.url,
            force: cli.force,
            provider,
            requested_report_lang: cli.lang,
            root_cache_dir,
            cache_dir,
            model_path,
        })
    }
}

#[derive(serde::Serialize)]
pub struct YoutubeUrlRequested {
    pub header: EventHeader,
    pub job: JobSpec,
}

impl YoutubeUrlRequested {
    pub const EVENT_TYPE: &'static str = "youtube.url_requested";

    pub fn new(job: JobSpec) -> Self {
        let event_id = Uuid::new_v4();
        Self {
            header: EventHeader {
                event_id,
                parent_ids: Vec::new(),
                timestamp: SystemTime::now(),
            },
            job,
        }
    }
}

impl Event for YoutubeUrlRequested {
    fn event_id(&self) -> Uuid {
        self.header.event_id
    }

    fn parent_ids(&self) -> &[Uuid] {
        &self.header.parent_ids
    }

    fn event_type(&self) -> &'static str {
        Self::EVENT_TYPE
    }

    fn timestamp(&self) -> SystemTime {
        self.header.timestamp
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }
}
