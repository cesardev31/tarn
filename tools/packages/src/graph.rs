//! Explicit deterministic resolution and strict locked-graph validation.
use crate::{
    Result,
    manifest::{Dependency, Manifest, Policy},
    read_text,
    store::{self, Release},
    write_atomic,
};
use semver::{Version, VersionReq};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
#[derive(Clone, Debug)]
pub struct Lock {
    pub manifest: String,
    pub packages: BTreeMap<String, Release>,
}
impl Lock {
    pub fn json(&self) -> Value {
        json!({"schema":1,"manifest":self.manifest,"packages":self.packages.values().map(|r|json!({"registry":r.registry,"release":r.json()})).collect::<Vec<_>>()})
    }
    pub fn read(root: &Path) -> Result<Self> {
        let value: Value =
            serde_json::from_str(&read_text(&root.join("tarn.lock"), 8 * 1024 * 1024)?)
                .map_err(|e| format!("invalid tarn.lock: {e}"))?;
        if value.as_object().is_none_or(|o| o.len() != 3) || value["schema"] != 1 {
            return Err(
                "unsupported tarn.lock schema; explicit regeneration/review required".into(),
            );
        }
        let manifest = value["manifest"]
            .as_str()
            .ok_or("lock needs manifest identity")?
            .to_owned();
        store::valid_hash(&manifest)?;
        let list = value["packages"]
            .as_array()
            .ok_or("lock packages must be an array")?;
        if list.len() > 128 {
            return Err("package graph limit exceeded".into());
        }
        let mut packages = BTreeMap::new();
        for item in list {
            if item.as_object().is_none_or(|o| o.len() != 2) {
                return Err("unsupported locked package fields".into());
            }
            let registry = item["registry"]
                .as_str()
                .ok_or("lock registry must be a string")?;
            if crate::source(registry, Path::new("."))? != registry {
                return Err("locked origin must be normalized".into());
            }
            let release = store::release(&item["release"], registry)?;
            if packages.insert(release.name.clone(), release).is_some() {
                return Err("lock contains duplicate package identities".into());
            }
        }
        Ok(Self { manifest, packages })
    }
    pub fn write(&self, root: &Path) -> Result<()> {
        write_atomic(
            &root.join("tarn.lock"),
            serde_json::to_string_pretty(&self.json())
                .unwrap()
                .as_bytes(),
        )
    }
    pub fn validate(
        &self,
        manifest: &Manifest,
        home: &Path,
        contents: bool,
    ) -> Result<BTreeMap<String, PathBuf>> {
        if self.manifest != manifest.fingerprint() {
            return Err("tarn.lock does not match tarn.toml; run tarn update explicitly".into());
        }
        let policy = manifest.effective_policy(home)?;
        self.validate_graph(manifest, &policy)?;
        let mut roots = BTreeMap::new();
        if contents {
            for (name, release) in &self.packages {
                roots.insert(name.clone(),store::verify(home,release).map_err(|e|format!("{e}; use tarn fetch for missing sources, never to accept a hash mismatch"))?);
            }
        }
        Ok(roots)
    }
    fn validate_graph(&self, manifest: &Manifest, policy: &Policy) -> Result<()> {
        let mut visited = BTreeSet::new();
        let mut active = BTreeSet::new();
        for (name, dep) in &manifest.dependencies {
            self.edge(name, dep, policy, &mut visited, &mut active)?;
        }
        if visited.len() != self.packages.len() {
            return Err("lock contains unreachable/undeclared packages; run tarn update".into());
        }
        Ok(())
    }
    fn edge(
        &self,
        name: &str,
        dependency: &Dependency,
        policy: &Policy,
        visited: &mut BTreeSet<String>,
        active: &mut BTreeSet<String>,
    ) -> Result<()> {
        let release = self
            .packages
            .get(name)
            .ok_or_else(|| format!("lock is missing dependency `{name}`"))?;
        if dependency.registry != release.registry
            || !VersionReq::parse(&dependency.requirement)
                .map_err(|e| e.to_string())?
                .matches(&release.version)
        {
            return Err(format!(
                "locked `{name}` violates a version/origin constraint"
            ));
        }
        release.policy(policy)?;
        if active.contains(name) {
            return Err(format!("dependency cycle through `{name}`"));
        }
        if visited.contains(name) {
            return Ok(());
        }
        active.insert(name.into());
        for (name, dep) in &release.dependencies {
            self.edge(name, dep, policy, visited, active)?;
        }
        active.remove(name);
        visited.insert(name.into());
        Ok(())
    }
}
struct Solver<'a> {
    manifest: &'a Manifest,
    policy: Policy,
    old: Option<&'a Lock>,
    freeze: Option<&'a str>,
    steps: usize,
    indices: BTreeMap<(String, String), Vec<Version>>,
    releases: BTreeMap<(String, String, Version), Release>,
    failure: String,
}
impl Solver<'_> {
    fn search(
        &mut self,
        chosen: BTreeMap<String, Release>,
    ) -> Result<Option<BTreeMap<String, Release>>> {
        self.steps += 1;
        if self.steps > 10000 || chosen.len() > 128 {
            return Err("deterministic resolution limit exceeded; simplify the graph".into());
        }
        let mut constraints: BTreeMap<String, Vec<(String, Dependency)>> = BTreeMap::new();
        for (name, dep) in &self.manifest.dependencies {
            constraints
                .entry(name.clone())
                .or_default()
                .push((self.manifest.name.clone(), dep.clone()));
        }
        for (owner, release) in &chosen {
            for (name, dep) in &release.dependencies {
                constraints
                    .entry(name.clone())
                    .or_default()
                    .push((owner.clone(), dep.clone()));
            }
        }
        for (name, requirements) in &constraints {
            let origins: BTreeSet<_> = requirements.iter().map(|(_, d)| &d.registry).collect();
            if origins.len() > 1
                || chosen.get(name).is_some_and(|r| {
                    requirements.iter().any(|(_, d)| {
                        d.registry != r.registry
                            || !VersionReq::parse(&d.requirement)
                                .unwrap()
                                .matches(&r.version)
                    })
                })
            {
                self.failure = format!(
                    "conflicting constraints for `{name}`: {}",
                    requirements
                        .iter()
                        .map(|(owner, d)| format!(
                            "{owner} requires {} from {}",
                            d.requirement, d.registry
                        ))
                        .collect::<Vec<_>>()
                        .join("; ")
                );
                return Ok(None);
            }
        }
        let Some((name, requirements)) = constraints
            .iter()
            .find(|(name, _)| !chosen.contains_key(*name))
        else {
            let candidate = Lock {
                manifest: self.manifest.fingerprint(),
                packages: chosen,
            };
            if let Err(error) = candidate.validate_graph(self.manifest, &self.policy) {
                self.failure = error;
                return Ok(None);
            }
            return Ok(Some(candidate.packages));
        };
        let registry = requirements[0].1.registry.clone();
        let key = (registry.clone(), name.clone());
        if !self.indices.contains_key(&key) {
            self.indices
                .insert(key.clone(), store::versions(&registry, name)?);
        }
        let mut versions = self.indices[&key].clone();
        if self.freeze.is_none() {
            if let Some(old) = self
                .old
                .and_then(|l| l.packages.get(name))
                .filter(|r| r.registry == registry)
            {
                if let Some(index) = versions.iter().position(|v| v == &old.version) {
                    let v = versions.remove(index);
                    versions.insert(0, v);
                }
            }
        }
        for version in versions {
            if requirements
                .iter()
                .any(|(_, d)| !VersionReq::parse(&d.requirement).unwrap().matches(&version))
            {
                continue;
            }
            if let (Some(target), Some(old)) =
                (self.freeze, self.old.and_then(|l| l.packages.get(name)))
            {
                if target != "*"
                    && target != name
                    && (old.version != version || old.registry != registry)
                {
                    continue;
                }
            }
            let key = (registry.clone(), name.clone(), version.clone());
            if !self.releases.contains_key(&key) {
                let release = store::load_release(&registry, name, &version)?;
                if let Some(old) = self
                    .old
                    .and_then(|l| l.packages.get(name))
                    .filter(|r| r.version == version && r.registry == registry)
                {
                    if old.json() != release.json() {
                        return Err(format!(
                            "immutable release {} {} changed; refusing replacement bytes/evidence",
                            name, version
                        ));
                    }
                }
                self.releases.insert(key.clone(), release);
            }
            let release = self.releases[&key].clone();
            if let Err(error) = release.policy(&self.policy) {
                self.failure = error;
                continue;
            }
            let mut next = chosen.clone();
            next.insert(name.clone(), release);
            if let Some(solution) = self.search(next)? {
                return Ok(Some(solution));
            }
        }
        if self.failure.is_empty() {
            self.failure = format!(
                "no version of `{name}` satisfies {}",
                requirements
                    .iter()
                    .map(|(n, d)| format!("{n}: {}", d.requirement))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        Ok(None)
    }
}
pub fn resolve(
    manifest: &Manifest,
    home: &Path,
    old: Option<&Lock>,
    update: Option<&str>,
) -> Result<Lock> {
    let mut solver = Solver {
        manifest,
        policy: manifest.effective_policy(home)?,
        old,
        freeze: update,
        steps: 0,
        indices: BTreeMap::new(),
        releases: BTreeMap::new(),
        failure: String::new(),
    };
    let packages=solver.search(BTreeMap::new())?.ok_or_else(||format!("cannot resolve one compatible version per package: {}; no manifest/lock changes were written",solver.failure))?;
    let lock = Lock {
        manifest: manifest.fingerprint(),
        packages,
    };
    lock.validate(manifest, home, false)?;
    for release in lock.packages.values() {
        store::fetch(home, release)?;
    }
    Ok(lock)
}
pub struct Context {
    pub manifest: Manifest,
    pub lock: Lock,
    pub roots: BTreeMap<String, PathBuf>,
    pub sources: BTreeMap<PathBuf, String>,
}
impl Context {
    pub fn load(entry: &Path) -> Result<Option<Self>> {
        let Some(root) = crate::manifest::find(entry) else {
            return Ok(None);
        };
        let manifest = Manifest::read(&root)?;
        let home = crate::home()?;
        // No lock is necessary for a project that has never declared packages.
        let lock = if manifest.dependencies.is_empty() && !root.join("tarn.lock").exists() {
            Lock {
                manifest: manifest.fingerprint(),
                packages: BTreeMap::new(),
            }
        } else {
            Lock::read(&root)
                .map_err(|e| format!("{e}; run tarn update or tarn fetch explicitly"))?
        };
        lock.validate(&manifest, &home, false)?;
        let mut roots = BTreeMap::new();
        let mut sources = BTreeMap::new();
        for (name, release) in &lock.packages {
            let (root, files) = store::verified_files(&home, release)?;
            for (path, bytes) in files {
                if path.ends_with(".tarn") {
                    sources.insert(
                        root.join(path),
                        String::from_utf8(bytes).map_err(|_| "invalid UTF-8 package source")?,
                    );
                }
            }
            roots.insert(name.clone(), root);
        }
        Ok(Some(Self {
            manifest,
            lock,
            roots,
            sources,
        }))
    }
    /// Verified sources and the import graph as plain compiler input.
    pub fn package_set(&self, home: &Path) -> tarn_driver::PackageSet {
        tarn_driver::PackageSet {
            direct: self.manifest.dependencies.keys().cloned().collect(),
            packages: self
                .lock
                .packages
                .iter()
                .map(|(name, release)| {
                    let package = tarn_driver::Package {
                        entry: self.roots[name].join(&release.entry),
                        dependencies: release.dependencies.keys().cloned().collect(),
                    };
                    (name.clone(), package)
                })
                .collect(),
            sources: self.sources.iter().map(|(path, text)| (path.clone(), text.clone())).collect(),
            inputs: vec![
                self.manifest.root.join("tarn.toml"),
                self.manifest.root.join("tarn.lock"),
                home.join("config.toml"),
            ],
        }
    }
    pub fn dependencies(&self, owner: Option<&str>) -> &BTreeMap<String, Dependency> {
        match owner {
            Some(name) => &self.lock.packages[name].dependencies,
            None => &self.manifest.dependencies,
        }
    }
    pub fn import(
        &self,
        owner: Option<&str>,
        path: &str,
    ) -> Result<Option<(String, PathBuf, String)>> {
        let (name, rest) = path.split_once('/').unwrap_or((path, ""));
        if !self.dependencies(owner).contains_key(name) {
            return Ok(None);
        }
        let release = &self.lock.packages[name];
        let base = &self.roots[name];
        let entry = base.join(&release.entry);
        let root = entry.parent().unwrap();
        let file = if rest.is_empty() {
            entry
        } else {
            crate::manifest::safe_path(rest)?;
            root.join(format!("{rest}.tarn"))
        };
        let relative = file
            .strip_prefix(base)
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("invalid package module path")?;
        if !release.files.contains_key(relative) {
            return Err(format!("package `{name}` does not contain module `{path}`"));
        }
        let module = if rest.is_empty() {
            format!("@package/{name}")
        } else {
            format!("@package/{name}/{rest}")
        };
        Ok(Some((module, file, name.into())))
    }
}

/// The verified package input for compiling `entry`, or None outside a
/// package project. Fails closed on a missing or inconsistent lock or cache.
pub fn package_set(entry: &Path) -> Result<Option<tarn_driver::PackageSet>> {
    match Context::load(entry)? {
        Some(context) => Ok(Some(context.package_set(&crate::home()?))),
        None => Ok(None),
    }
}
