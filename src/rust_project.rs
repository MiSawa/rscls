use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use base_db::Edition;
use once_cell::sync::Lazy;
use paths::{AbsPath, AbsPathBuf};
use project_model::{ProjectWorkspace, Sysroot};
use serde::Serialize;

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct RustProject {
    pub sysroot: PathBuf,
    pub sysroot_src: PathBuf,
    pub crates: Vec<Crate>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Crate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub display_name: Option<String>,
    pub root_module: PathBuf,
    #[serde(serialize_with = "serialize_edition")]
    pub edition: Edition,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub version: Option<String>,
    pub deps: Vec<Dep>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub cfg: Vec<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub exclude: Vec<String>,

    pub is_proc_macro: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub proc_macro_dylib_path: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub repository: Option<String>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Dep {
    #[serde(rename = "crate")]
    pub krate: usize,
    pub name: String,
}

static CFG_KEYS_TO_IGNORE: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from_iter([
        "debug_assertions",
        "panic",
        "target_abi",
        "target_arch",
        "target_endian",
        "target_env",
        "target_family",
        "target_feature",
        "target_has_atomic",
        "target_has_atomic_equal_alignment",
        "target_has_atomic_load_store",
        "target_os",
        "target_pointer_width",
        "target_thread_local",
        "target_vendor",
        "unix",
        "windows",
    ])
});

fn serialize_edition<S: serde::Serializer>(
    edition: &Edition,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let s = match edition {
        Edition::Edition2015 => "2015",
        Edition::Edition2018 => "2018",
        Edition::Edition2021 => "2021",
    };
    serializer.serialize_str(s)
}

impl RustProject {
    pub fn fallback_project(sysroot: &Sysroot, source: AbsPathBuf) -> Self {
        let krate = Crate {
            display_name: None,
            root_module: source.into(),
            edition: Edition::CURRENT,
            version: None,
            deps: vec![],
            cfg: vec![],
            include: vec![],
            exclude: vec![],
            is_proc_macro: false,
            proc_macro_dylib_path: None,
            repository: None,
        };
        RustProject {
            sysroot: sysroot.root().to_owned().into(),
            sysroot_src: sysroot.src_root().to_owned().into(),
            crates: vec![krate],
        }
    }

    pub fn from_workspace(
        sysroot: &Sysroot,
        workspace: &ProjectWorkspace,
        mut translate_path: impl FnMut(&AbsPath) -> AbsPathBuf,
    ) -> Self {
        let mut path_to_id = HashMap::new();
        let mut id_to_path = HashMap::new();
        let (crate_graph, proc_macro_paths) = workspace.to_crate_graph(
            &mut |path| {
                let path = translate_path(path);
                let next_id = path_to_id.len() as u32;
                let id = path_to_id.entry(path.to_owned()).or_insert(next_id).clone();
                let ret = vfs::FileId(id);
                id_to_path
                    .entry(ret.clone())
                    .or_insert_with(|| path.to_owned());
                Some(ret)
            },
            &Default::default(),
        );

        let mut crate_ids = vec![];
        let mut crate_id_to_index = HashMap::new();
        for id in crate_graph.iter() {
            crate_ids.push(id);
            crate_id_to_index.insert(id, crate_id_to_index.len());
        }

        let crates = crate_ids
            .into_iter()
            .map(|id| {
                let data = &crate_graph[id];
                let display_name = data
                    .display_name
                    .as_ref()
                    .map(|name| name.canonical_name().to_owned());

                let root_module = id_to_path[&data.root_file_id].clone().into();
                let deps = data
                    .dependencies
                    .iter()
                    .map(|dep| Dep {
                        krate: crate_id_to_index[&dep.crate_id],
                        name: dep.name.to_string(),
                    })
                    .collect();
                let cfg: Vec<_> = data
                    .cfg_options
                    .get_cfg_keys()
                    .filter(|key| !CFG_KEYS_TO_IGNORE.contains(key.as_str()))
                    .flat_map(|key| {
                        // TODO: Escape?
                        data.cfg_options
                            .check(&cfg::CfgExpr::Atom(cfg::CfgAtom::Flag(key.clone())))
                            .filter(|x| *x)
                            .map(|_| format!("{key}"))
                            .into_iter()
                            .chain(
                                data.cfg_options
                                    .get_cfg_values(key)
                                    .map(move |value| format!(r#"{key}="{value}""#)),
                            )
                    })
                    .collect();
                let proc_macro_dylib_path = proc_macro_paths
                    .get(&id)
                    .and_then(|v| v.as_ref().ok().map(|v| v.1.clone().into()));
                let repository = match &data.origin {
                    base_db::CrateOrigin::Local { repo, .. }
                    | base_db::CrateOrigin::Library { repo, .. } => repo.clone(),
                    _ => None,
                };

                Crate {
                    display_name,
                    root_module,
                    edition: data.edition,
                    version: data.version.clone(),
                    deps,
                    cfg,
                    include: vec![],
                    exclude: vec![],
                    is_proc_macro: data.is_proc_macro,
                    proc_macro_dylib_path,
                    repository,
                }
            })
            .collect();

        RustProject {
            sysroot: sysroot.root().to_owned().into(),
            sysroot_src: sysroot.src_root().to_owned().into(),
            crates,
        }
    }
}
