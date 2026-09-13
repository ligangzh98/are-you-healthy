use anyhow::Context;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub assets: AssetsConfig,
    pub history: HistoryConfig,
    pub probe: ProbeConfig,
    pub scheduler: SchedulerConfig,
    pub http_client: HttpClientConfig,
    pub log: LogConfig,
    pub feishu: FeishuConfig,
    pub pushplus: PushplusConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FeishuConfig {
    pub webhook_url: String,
    pub enabled: bool,
    pub alert_cooldown_secs: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PushplusConfig {
    pub token: String,
    pub enabled: bool,
    pub alert_cooldown_secs: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AssetsConfig {
    pub dir: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub retention_days: u64,
    pub max_per_check: u64,
    pub cleanup_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ProbeConfig {
    pub capture_max_bytes: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SchedulerConfig {
    pub tick_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HttpClientConfig {
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LogConfig {
    pub level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            assets: AssetsConfig::default(),
            history: HistoryConfig::default(),
            probe: ProbeConfig::default(),
            scheduler: SchedulerConfig::default(),
            http_client: HttpClientConfig::default(),
            log: LogConfig::default(),
            feishu: FeishuConfig::default(),
            pushplus: PushplusConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 8080,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: "data/health.db".into(),
        }
    }
}

impl Default for AssetsConfig {
    fn default() -> Self {
        Self { dir: "assets".into() }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            retention_days: 30,
            max_per_check: 1000,
            cleanup_interval_secs: 3600,
        }
    }
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            capture_max_bytes: 8192,
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self { tick_secs: 5 }
    }
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self { timeout_secs: 15 }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
        }
    }
}

impl Default for FeishuConfig {
    fn default() -> Self {
        Self {
            webhook_url: String::new(),
            enabled: false,
            alert_cooldown_secs: 300,
        }
    }
}

impl Default for PushplusConfig {
    fn default() -> Self {
        Self {
            token: String::new(),
            enabled: false,
            alert_cooldown_secs: 300,
        }
    }
}

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read config {:?}", path))?;
        let cfg: AppConfig = toml::from_str(&text)
            .with_context(|| format!("parse config {:?}", path))?;
        Ok(cfg.normalized())
    }

    fn normalized(self) -> Self {
        let mut cfg = self;
        cfg.probe.capture_max_bytes = cfg.probe.capture_max_bytes.clamp(256, 1024 * 1024);
        cfg.scheduler.tick_secs = cfg.scheduler.tick_secs.max(1);
        cfg.history.cleanup_interval_secs = cfg.history.cleanup_interval_secs.max(60);
        cfg.http_client.timeout_secs = cfg.http_client.timeout_secs.max(1);
        cfg.feishu.alert_cooldown_secs = cfg.feishu.alert_cooldown_secs.max(60);
        cfg.pushplus.alert_cooldown_secs = cfg.pushplus.alert_cooldown_secs.max(60);
        cfg
    }

    pub fn database_path(&self) -> PathBuf {
        PathBuf::from(&self.database.path)
    }

    pub fn assets_dir(&self) -> PathBuf {
        PathBuf::from(&self.assets.dir)
    }
}

pub fn init(cfg: AppConfig) {
    CONFIG.set(cfg).expect("config initialized once");
}

pub fn get() -> &'static AppConfig {
    CONFIG.get().expect("config not initialized")
}

pub fn default_config_path() -> PathBuf {
    PathBuf::from("config.toml")
}
