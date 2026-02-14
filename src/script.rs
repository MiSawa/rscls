use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
};

use eyre::{ensure, eyre, Result, WrapErr as _};
use futures::future::JoinAll;
use path_absolutize::Absolutize as _;
use serde_json::{json, Value};
use tokio::{
    process::Command,
    spawn,
    sync::{Mutex, OnceCell},
};

use crate::event::EventSender;

struct Script {
    source: PathBuf,
    rust_script: Arc<PathBuf>,
    fallback_project: Value,
    project: RwLock<Arc<Option<PathBuf>>>,
    need_refresh: AtomicBool,
    refresh_lock: tokio::sync::Mutex<()>,
}
impl Script {
    async fn new(source: PathBuf, rust_script: Arc<PathBuf>) -> Self {
        let fallback_project = create_default_project(&source).await;
        Self {
            source,
            rust_script,
            fallback_project,
            project: RwLock::new(Arc::new(None)),
            need_refresh: AtomicBool::new(false),
            refresh_lock: Mutex::new(()),
        }
    }

    async fn project(&self) -> Value {
        let tmp = self.project.read().unwrap().clone();
        if let Some(manifest) = tmp.as_ref() {
            if tokio::fs::metadata(manifest).await.is_ok() {
                return serde_json::to_value(manifest).unwrap();
            }
        }
        self.fallback_project.clone()
    }

    async fn queue_refresh(self: &Arc<Self>, refreshed: impl FnOnce() + Send + 'static) {
        let this = self.clone();
        this.need_refresh.store(true, Ordering::SeqCst);
        this.do_refresh(refreshed).await
    }

    async fn do_refresh(self: Arc<Self>, refreshed: impl FnOnce()) {
        let _guard = self.refresh_lock.lock().await;
        if !self.need_refresh.swap(false, Ordering::SeqCst) {
            return;
        }
        let project_dir = match package_dir(self.rust_script.as_ref(), self.source.as_path()).await
        {
            Ok(project_dir) => project_dir,
            Err(e) => {
                tracing::error!(script = ?self.source, ?e, "failed to load script as a project");
                return;
            }
        };
        let new_project = Some(project_dir.join("Cargo.toml"));
        let mut project_write = self.project.write().unwrap();
        if project_write.as_ref() != &new_project {
            *project_write = new_project.into();
            tracing::info!(script = ?self.source, "reloaded project");
            refreshed();
        } else {
            tracing::info!(script = ?self.source, "no project diff found");
        }
    }
}

pub struct Scripts {
    event_sender: EventSender,
    rust_script: Arc<PathBuf>,
    scripts: BTreeMap<lsp_types::Url, Arc<Script>>,
}
impl Scripts {
    pub fn new(event_sender: EventSender, rust_script: PathBuf) -> Result<Self> {
        Ok(Self {
            event_sender,
            rust_script: rust_script.into(),
            scripts: BTreeMap::new(),
        })
    }

    pub async fn register(&mut self, uri: lsp_types::Url) {
        if let Ok(file) = uri.to_file_path() {
            if let std::collections::btree_map::Entry::Vacant(entry) = self.scripts.entry(uri) {
                let script =
                    entry.insert(Arc::new(Script::new(file, self.rust_script.clone()).await));
                let sender = self.event_sender.clone();
                sender.mark_need_reload();
                script
                    .queue_refresh(move || sender.mark_need_reload())
                    .await
            }
        }
    }

    pub fn deregister_if_registered(&mut self, uri: &lsp_types::Url) {
        if self.scripts.remove(uri).is_some() {
            self.event_sender.mark_need_reload();
        }
    }

    pub async fn queue_refresh(&self, uri: &lsp_types::Url) {
        if let Some(script) = self.scripts.get(uri) {
            let sender = self.event_sender.clone();
            script
                .queue_refresh(move || sender.mark_need_reload())
                .await
        }
    }

    pub async fn queue_refresh_all(&self) {
        // https://github.com/rust-lang/rust-clippy/issues/16012
        #[allow(clippy::redundant_iter_cloned)]
        self.scripts
            .values()
            .cloned()
            .map(|script| {
                let sender = self.event_sender.clone();
                spawn(async move {
                    script
                        .queue_refresh(move || sender.mark_need_reload())
                        .await
                })
            })
            .collect::<JoinAll<_>>()
            .await;
    }

    pub async fn projects(&self) -> Vec<Value> {
        self.scripts
            .values()
            .map(|script| script.project())
            .collect::<JoinAll<_>>()
            .await
    }
}

async fn create_default_project(source: &PathBuf) -> Value {
    static SYSROOT: OnceCell<Result<PathBuf>> = OnceCell::const_new();
    let sysroot = SYSROOT.get_or_init(default_sysroot).await;
    let mut value = json!({
        "crates": [{
            "root_module": source,
            "edition": "2021",
            "deps": [],
            "is_proc_macro": false,
        }]
    });
    if let Ok(Ok(sysroot)) = sysroot.as_ref().map(serde_json::to_value) {
        value
            .as_object_mut()
            .unwrap()
            .insert("sysroot".to_owned(), sysroot);
    }
    value
}

async fn package_dir(rust_script: impl AsRef<Path>, script: impl AsRef<Path>) -> Result<PathBuf> {
    let mut cmd = Command::new(rust_script.as_ref());
    cmd.arg("--package").arg(script.as_ref());
    run_and_parse_output_as_path(cmd).await
}

async fn default_sysroot() -> Result<PathBuf> {
    let mut cmd = Command::new("rustc");
    cmd.args(["--print", "sysroot"]).current_dir("/");
    run_and_parse_output_as_path(cmd).await
}

async fn run_and_parse_output_as_path(mut command: Command) -> Result<PathBuf> {
    let output = command
        .output()
        .await
        .wrap_err_with(|| eyre!("failed to run `{command:?}`"))?;
    ensure!(
        output.status.success(),
        "`{command:?}` terminated with a nonzero exit status {} with stderr {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8(output.stdout).wrap_err("got an invalid path")?;
    let path = PathBuf::from(output.trim_end());
    let path = path.absolutize().wrap_err("got an invalid abs path")?;
    Ok(path.to_path_buf())
}
