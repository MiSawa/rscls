use std::{
    collections::BTreeMap,
    io::BufWriter,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
};

use eyre::{bail, ensure, eyre, Result, WrapErr as _};
use path_absolutize::Absolutize as _;
use paths::{AbsPath, AbsPathBuf};
use project_model::{CargoConfig, ProjectManifest, ProjectWorkspace, RustLibSource, Sysroot};
use serde::Serialize;
use serde_json::Value;
use tempfile::{NamedTempFile, TempDir};

use crate::{event::EventSender, rust_project::RustProject};

enum CachedProject {
    InMemory {
        project: RustProject,
    },
    InFile {
        project: RustProject,
        project_json: PathBuf,
        project_source: PathBuf,
    },
}
impl Serialize for CachedProject {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            CachedProject::InMemory { project } => project.serialize(serializer),
            CachedProject::InFile { project_json, .. } => project_json.serialize(serializer),
        }
    }
}

impl CachedProject {
    fn as_rust_project(&self) -> &RustProject {
        match self {
            CachedProject::InMemory { project } | CachedProject::InFile { project, .. } => project,
        }
    }
}

struct Script {
    source: AbsPathBuf,
    rust_script: Arc<PathBuf>,
    project: RwLock<Arc<CachedProject>>,
    need_refresh: AtomicBool,
}
impl Script {
    fn new(source: AbsPathBuf, project: RustProject, rust_script: Arc<PathBuf>) -> Self {
        Self {
            source: source.into(),
            rust_script: rust_script.into(),
            project: RwLock::new(Arc::new(CachedProject::InMemory { project })),
            need_refresh: AtomicBool::new(false),
        }
    }

    fn project(&self) -> Arc<CachedProject> {
        self.project.read().unwrap().clone()
    }

    pub fn project_path_to_script_path(&self, path: impl AsRef<Path>) -> Option<&Path> {
        let proj = self.project.read().unwrap();
        if let CachedProject::InFile { project_source, .. } = proj.as_ref() {
            if path.as_ref() == project_source {
                return Some(self.source.as_ref());
            }
        }
        None
    }

    fn refresh(self: &Arc<Self>, refreshed: impl FnOnce() + Send + 'static) {
        let this = self.clone();
        this.need_refresh.store(true, Ordering::SeqCst);
        std::thread::spawn(move || {
            if !this.need_refresh.swap(false, Ordering::SeqCst) {
                return;
            }
            let ret = || -> Result<()> {
                let project_dir = package_dir(this.rust_script.as_ref(), this.source.as_path())?;
                let sysroot = Sysroot::discover(&project_dir, &Default::default())
                    .map_err(|e| eyre!("unable to find sysroot: {e:?}"))?;
                let workspace = load_workspace(&sysroot, &project_dir)?;
                let mut project_source_file_name = this
                    .source
                    .file_stem()
                    .ok_or(eyre!("failed to obtain file stem"))?
                    .to_owned();
                project_source_file_name.push(".rs");
                let project_source = project_dir.join(project_source_file_name);
                let project = RustProject::from_workspace(&sysroot, &workspace, |path| {
                    if path == project_source.as_path() {
                        this.source.clone()
                    } else {
                        path.to_path_buf()
                    }
                });
                let mut project_write = this.project.write().unwrap();
                if project_write.as_rust_project() != &project {
                    // let project_temp_dir =
                    //     tempfile::tempdir_in(&project_dir).wrap_err("unable to create temp dir")?;
                    // let project_json = project_temp_dir.path().join("rust-project.json");
                    // let file = File::create(&project_json).wrap_err("unable to open temp file")?;
                    // serde_json::to_writer_pretty(BufWriter::new(&file), &project)
                    //     .wrap_err("failed to write to temp file")?;
                    // drop(file);

                    let project_json = project_dir.join("rust-project.json");
                    let temp = NamedTempFile::new_in(&project_dir)
                        .wrap_err("unable to create temp file")?;
                    serde_json::to_writer_pretty(BufWriter::new(&temp), &project)
                        .wrap_err("failed to write to temp file")?;
                    temp.persist(&project_json)
                        .wrap_err("failed to persist temp file")?;

                    *project_write = CachedProject::InFile {
                        project,
                        project_json: project_json.into(),
                        project_source: project_source.into(),
                    }
                    .into();
                    tracing::info!(script = ?this.source, "reloaded project");
                    refreshed();
                } else {
                    tracing::info!(script = ?this.source, "no project diff found");
                }
                Ok(())
            }();
            if let Err(e) = ret {
                tracing::error!(script = ?this.source, ?e, "failed to load script as a project");
            }
        });
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
            Sysroot::discover(&AbsPathBuf::try_from("/").unwrap(), &Default::default())
                .map_err(|e| eyre!("unable to locate fallback sysroot: {e:?}"))?;
        Ok(Self {
            event_sender,
            rust_script: rust_script.into(),
            scripts: BTreeMap::new(),
            fallback_sysroot,
        })
    }

    pub fn register(&mut self, uri: lsp_types::Url) {
        if let Ok(file) = uri
            .to_file_path()
            .and_then(|path| AbsPathBuf::try_from(path).map_err(|_| ()))
        {
            if let std::collections::btree_map::Entry::Vacant(entry) = self.scripts.entry(uri) {
                let project = RustProject::fallback_project(&self.fallback_sysroot, file.clone());
                let script = entry.insert(Arc::new(Script::new(
                    file,
                    project,
                    self.rust_script.clone(),
                )));
                let sender = self.event_sender.clone();
                sender.mark_need_reload();
                script.refresh(move || {
                    sender.mark_need_reload();
                })
            }
        }
    }

    pub fn deregister_if_registered(&mut self, uri: &lsp_types::Url) {
        if self.scripts.remove(&uri).is_some() {
            self.event_sender.mark_need_reload();
        }
    }

    pub fn saved(&mut self, uri: &lsp_types::Url) {
        if let Some(script) = self.scripts.get_mut(uri) {
            let sender = self.event_sender.clone();
            script.refresh(move || {
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

    pub fn project_path_to_script_path(&self, path: impl AsRef<Path>) -> Option<&Path> {
        self.scripts
            .values()
            .find_map(|script| script.project_path_to_script_path(&path))
    }
}

fn package_dir(rust_script: impl AsRef<Path>, script: impl AsRef<Path>) -> Result<AbsPathBuf> {
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
    Ok(AbsPathBuf::try_from(path.to_path_buf()).unwrap())
}

fn load_workspace(sysroot: &Sysroot, project_dir: impl AsRef<AbsPath>) -> Result<ProjectWorkspace> {
    let ProjectManifest::CargoToml(manifest_path) = ProjectManifest::from_manifest_file(project_dir.as_ref().join("Cargo.toml"))
        .map_err(|e| eyre!("unable to obtain manifest path: {e:?}"))? else {
            bail!("project manifest wasn't Cargo.toml");
        };
    let mut config = CargoConfig::default();
    config.sysroot = Some(RustLibSource::Path(sysroot.root().to_path_buf()));
    config.rustc_source = Some(RustLibSource::Discover);
    ProjectWorkspace::load(ProjectManifest::CargoToml(manifest_path), &config, &|_| ())
        .map_err(|e| eyre!("unable to load workspace: {e:?}"))
}
