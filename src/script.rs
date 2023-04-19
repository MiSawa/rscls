use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
};

use eyre::{ensure, eyre, Result, WrapErr as _};
use path_absolutize::Absolutize as _;
use project_model::Sysroot;
use serde::Serialize;
use serde_json::{Value, json};

use crate::event::EventSender;

#[derive(PartialEq, Eq)]
enum CachedProject {
    InMemory { project: Value },
    InFile { manifest: PathBuf },
}
impl Serialize for CachedProject {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            CachedProject::InMemory { project } => project.serialize(serializer),
            CachedProject::InFile { manifest } => manifest.serialize(serializer),
        }
    }
}

struct Script {
    source: PathBuf,
    rust_script: Arc<PathBuf>,
    project: RwLock<Arc<CachedProject>>,
    need_refresh: AtomicBool,
    refresh_lock: Mutex<()>,
}
impl Script {
    fn new(source: PathBuf, project: Value, rust_script: Arc<PathBuf>) -> Self {
        Self {
            source,
            rust_script,
            project: RwLock::new(Arc::new(CachedProject::InMemory { project })),
            need_refresh: AtomicBool::new(false),
            refresh_lock: Mutex::new(()),
        }
    }

    fn project(&self) -> Arc<CachedProject> {
        self.project.read().unwrap().clone()
    }

    fn queue_refresh(self: &Arc<Self>, refreshed: impl FnOnce() + Send + 'static) {
        let this = self.clone();
        this.need_refresh.store(true, Ordering::SeqCst);
        std::thread::spawn(move || this.do_refresh(refreshed));
    }

    fn do_refresh(self: Arc<Self>, refreshed: impl FnOnce()) {
        let _guard = self.refresh_lock.lock().unwrap();
        if !self.need_refresh.swap(false, Ordering::SeqCst) {
            return;
        }
        let ret = || -> Result<()> {
            let project_dir = package_dir(self.rust_script.as_ref(), self.source.as_path())?;
            let new_project = CachedProject::InFile {
                manifest: project_dir.join("Cargo.toml"),
            };
            let mut project_write = self.project.write().unwrap();
            if project_write.as_ref() != &new_project {
                *project_write = new_project.into();
                tracing::info!(script = ?self.source, "reloaded project");
                refreshed();
            } else {
                tracing::info!(script = ?self.source, "no project diff found");
            }
            Ok(())
        }();
        if let Err(e) = ret {
            tracing::error!(script = ?self.source, ?e, "failed to load script as a project");
        }
    }
}

pub struct Scripts {
    event_sender: EventSender,
    rust_script: Arc<PathBuf>,
    scripts: BTreeMap<lsp_types::Url, Arc<Script>>,
    fallback_sysroot: Sysroot,
}
impl Scripts {
    pub fn new(event_sender: EventSender, rust_script: PathBuf) -> Result<Self> {
        let fallback_sysroot =
            Sysroot::discover(Path::new("/").try_into().unwrap(), &Default::default())
                .map_err(|e| eyre!("unable to locate fallback sysroot: {e:?}"))?;
        Ok(Self {
            event_sender,
            rust_script: rust_script.into(),
            scripts: BTreeMap::new(),
            fallback_sysroot,
        })
    }

    pub fn register(&mut self, uri: lsp_types::Url) {
        if let Ok(file) = uri.to_file_path() {
            if let std::collections::btree_map::Entry::Vacant(entry) = self.scripts.entry(uri) {
                let project = create_default_project(&self.fallback_sysroot, &file);
                let script = entry.insert(Arc::new(Script::new(
                    file,
                    project,
                    self.rust_script.clone(),
                )));
                let sender = self.event_sender.clone();
                sender.mark_need_reload();
                script.queue_refresh(move || {
                    sender.mark_need_reload();
                })
            }
        }
    }

    pub fn deregister_if_registered(&mut self, uri: &lsp_types::Url) {
        if self.scripts.remove(uri).is_some() {
            self.event_sender.mark_need_reload();
        }
    }

    pub fn queue_refresh(&mut self, uri: &lsp_types::Url) {
        if let Some(script) = self.scripts.get_mut(uri) {
            let sender = self.event_sender.clone();
            script.queue_refresh(move || {
                sender.mark_need_reload();
            })
        }
    }

    pub fn projects(&self) -> Vec<Value> {
        self.scripts
            .values()
            .map(|script| serde_json::to_value(script.project().as_ref()).unwrap())
            .collect()
    }
}

fn package_dir(rust_script: impl AsRef<Path>, script: impl AsRef<Path>) -> Result<PathBuf> {
    let output = Command::new(rust_script.as_ref())
        .arg("--package")
        .arg(script.as_ref())
        .output()
        .wrap_err("failed to spawn rust-script")?;
    ensure!(
        output.status.success(),
        "rust-script process terminated with a nonzero exit code {} with stderr {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8(output.stdout).wrap_err("got an invalid path")?;
    let path = PathBuf::from(output.trim_end());
    let path = path.absolutize().wrap_err("got an invalid abs path")?;
    Ok(path.to_path_buf())
}

fn create_default_project(sysroot: &Sysroot, source: &PathBuf) -> Value {
    let root: PathBuf = sysroot.root().to_owned().into();
    let src_root: PathBuf = sysroot.src_root().to_owned().into();
    json!({
        "sysroot": root,
        "src_root": src_root,
        "crates": [{
            "root_module": source,
            "edition": "2021",
            "deps": [],
            "is_proc_macro": false,
        }]
    })
}
