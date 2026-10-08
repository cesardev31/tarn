//! Test discovery and generated ordinary source harnesses (ADR 0049).
use crate::{CheckResult, PackageSet, check_loaded, load_with_packages};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use tarn_ast::ItemKind;
use tarn_types::Ty;

pub const ENTRY: &str = "__tarn_test_dispatch";
#[derive(Clone, Debug)]
pub struct TestCase {
    pub name: String,
    pub index: usize,
}

/// Sorted sibling test roots, separate modules with ordinary visibility.
pub fn test_files(entry: &Path) -> Result<Vec<PathBuf>, String> {
    let parent = entry
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut files = Vec::new();
    for file in std::fs::read_dir(parent).map_err(|e| e.to_string())? {
        let path = file.map_err(|e| e.to_string())?.path();
        if path.is_file()
            && path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.ends_with("_test.tarn"))
            && path != entry
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

pub fn check_tests(entry: &Path) -> Result<(CheckResult, Vec<TestCase>), String> {
    check_tests_with_packages(entry, None)
}

/// `check_tests` for a project whose package dependencies the host verified.
pub fn check_tests_with_packages(entry: &Path, packages: Option<&PackageSet>) -> Result<(CheckResult, Vec<TestCase>), String> {
    let entry = entry
        .canonicalize()
        .map_err(|e| format!("cannot read `{}`: {e}", entry.display()))?;
    let mut roots = vec![entry.clone()];
    roots.extend(test_files(&entry)?);
    let mut text = BTreeMap::new();
    for path in &roots {
        text.insert(
            path.clone(),
            std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read `{}`: {e}", path.display()))?,
        );
    }
    let mut map = tarn_diagnostics::SourceMap::new();
    let file = map.add(entry.display().to_string(), text[&entry].clone());
    let parsed = tarn_parser::parse_file(file, map.file(file));
    // Generated imports must never complete an unterminated original token.
    if parsed
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == tarn_diagnostics::Severity::Error)
    {
        let mut result = crate::check(&entry)?;
        result.program.disk_sources.extend(roots);
        return Ok((result, Vec::new()));
    }
    let existing: std::collections::HashSet<_> = parsed
        .module
        .items
        .iter()
        .filter_map(|item| {
            if let ItemKind::Import(import) = &item.kind {
                Some(import.path.as_str())
            } else {
                None
            }
        })
        .collect();
    for path in roots.iter().skip(1) {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid test module name")?;
        if !stem.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || stem.as_bytes()[0].is_ascii_digit()
        {
            return Err(format!("invalid test module name `{stem}`"));
        }
        if !existing.contains(stem) {
            text.get_mut(&entry)
                .unwrap()
                .push_str(&format!("\nimport \"{stem}\"\n"));
        }
    }
    let overlays: HashMap<_, _> = text.iter().map(|(p, s)| (p.clone(), s.clone())).collect();
    let (mut initial_program, diagnostics) = load_with_packages(&entry, &overlays, packages)?;
    initial_program.disk_sources.extend(roots.iter().cloned());
    let initial = check_loaded((initial_program, diagnostics))?;
    if initial.has_errors() {
        return Ok((initial, Vec::new()));
    }
    let typed = initial.typed.as_ref().ok_or("missing test type tables")?;
    let resolved = initial
        .resolved
        .as_ref()
        .ok_or("missing test resolution tables")?;
    let generated_starts: HashMap<_, _> = text
        .iter()
        .map(|(path, source)| (path.clone(), source.len()))
        .collect();
    let mut cases = Vec::new();
    let mut dispatch = String::from("\nfn __tarn_test_dispatch(selected i32) {\n");
    for (module_index, (module_name, module)) in initial.program.modules.iter().enumerate() {
        let source_name = &initial.program.sources.file(module.file).name;
        let path = PathBuf::from(source_name);
        if !roots.contains(&path) {
            continue;
        }
        if module.items.iter().any(
            |item| matches!(&item.kind, ItemKind::Fn(f) if f.name.name.starts_with("__tarn_test_")),
        ) {
            return Err("function names beginning with __tarn_test_ are reserved for generated test harnesses".into());
        }
        for item in &module.items {
            let ItemKind::Fn(function) = &item.kind else {
                continue;
            };
            if function.owner.is_some() || !function.name.name.starts_with("test_") {
                continue;
            }
            let symbol = resolved.tables[module_index]
                .defs
                .get(&item.id)
                .or_else(|| resolved.tables[module_index].defs.get(&function.id))
                .ok_or("missing test symbol")?;
            let signature = &typed.decls.fns[symbol];
            let name = format!("{module_name}.{}", function.name.name);
            if function.is_async
                || function.is_unsafe
                || function.abi.is_some()
                || !signature.generics.is_empty()
                || !signature.params.is_empty()
                || function.receiver.is_some()
            {
                return Err(format!(
                    "test `{name}` must be a safe, synchronous, non-generic function with no parameters"
                ));
            }
            let index = cases.len();
            let body = if signature.ret == Ty::Void {
                format!("{}()\n", function.name.name)
            } else if let Ty::Adt(id, args) = &signature.ret {
                if Some(*id) != typed.decls.result || args.len() != 2 || args[0] != Ty::Void {
                    return Err(format!("test `{name}` must return void or Result<void, E>"));
                }
                let mut report = String::from("print(\"Err\")\n");
                match &args[1] {
                    Ty::Bool | Ty::Int(_) | Ty::Float(_) | Ty::Str => {
                        report.push_str("print(error)\n")
                    }
                    Ty::Adt(id, arguments) => {
                        report.push_str(&format!(
                            "print(\"{}\")\n",
                            resolved.symbols[id.0 as usize].name
                        ));
                        if let Some(def) = typed.decls.structs.get(id) {
                            let substitution: HashMap<_, _> = def
                                .generics
                                .iter()
                                .copied()
                                .zip(arguments.iter().cloned())
                                .collect();
                            for field in &def.fields {
                                if (field.is_pub || def.module.0 as usize == module_index)
                                    && matches!(
                                        tarn_types::subst(&field.ty, &substitution),
                                        Ty::Bool | Ty::Int(_) | Ty::Float(_) | Ty::Str
                                    )
                                {
                                    report.push_str(&format!(
                                        "print(\"{}:\")\nprint(error.{})\n",
                                        field.name, field.name
                                    ));
                                }
                            }
                        }
                        if let Some(def) = typed.decls.enums.get(id) {
                            let type_path = function
                                .ret
                                .as_ref()
                                .and_then(|ret| {
                                    if let tarn_ast::TypeKind::Path(path) = &ret.kind {
                                        path.args.get(1).and_then(|error| {
                                            if let tarn_ast::TypeKind::Path(path) = &error.kind {
                                                Some(
                                                    path.segments
                                                        .iter()
                                                        .map(|segment| segment.name.as_str())
                                                        .collect::<Vec<_>>()
                                                        .join("."),
                                                )
                                            } else {
                                                None
                                            }
                                        })
                                    } else {
                                        None
                                    }
                                })
                                .ok_or("enum error type must have a source path")?;
                            let substitution: HashMap<_, _> = def
                                .generics
                                .iter()
                                .copied()
                                .zip(arguments.iter().cloned())
                                .collect();
                            report.push_str("match error {\n");
                            for variant in &def.variants {
                                let bindings: Vec<_> = (0..variant.fields.len())
                                    .map(|index| format!("error_part_{index}"))
                                    .collect();
                                let pattern = if bindings.is_empty() {
                                    String::new()
                                } else {
                                    format!("({})", bindings.join(", "))
                                };
                                report.push_str(&format!(
                                    "{type_path}.{}{pattern} => {{\nprint(\"{}.{}\")\n",
                                    variant.name,
                                    resolved.symbols[id.0 as usize].name,
                                    variant.name
                                ));
                                for (binding, ty) in bindings.iter().zip(&variant.fields) {
                                    if matches!(
                                        tarn_types::subst(ty, &substitution),
                                        Ty::Bool | Ty::Int(_) | Ty::Float(_) | Ty::Str
                                    ) {
                                        report.push_str(&format!("print({binding})\n"));
                                    }
                                }
                                report.push_str("}\n");
                            }
                            report.push_str("}\n");
                        }
                    }
                    _ => report.push_str("print(\"non-scalar error value\")\n"),
                }
                format!(
                    "match {}() {{\n Ok(_) => {{}}\n Err(error) => {{\n{report}panic(\"test returned Err\")\n }}\n}}\n",
                    function.name.name
                )
            } else {
                return Err(format!("test `{name}` must return void or Result<void, E>"));
            };
            text.get_mut(&path)
                .unwrap()
                .push_str(&format!("\npub fn __tarn_test_{index}() {{\n{body}}}\n"));
            let call = if path == entry {
                format!("__tarn_test_{index}()")
            } else {
                format!("{module_name}.__tarn_test_{index}()")
            };
            dispatch.push_str(&format!("if selected == {index} {{\n{call}\nreturn\n}}\n"));
            cases.push(TestCase { name, index });
        }
    }
    dispatch.push_str("panic(\"invalid test index\")\n}\n");
    text.get_mut(&entry).unwrap().push_str(&dispatch);
    let overlays: HashMap<_, _> = text.into_iter().collect();
    let (mut program, diagnostics) = load_with_packages(&entry, &overlays, packages)?;
    // Overlay roots remain real watch dependencies, not embedded sources.
    program.disk_sources.extend(roots);
    program.disk_sources.sort();
    program.disk_sources.dedup();
    Ok((
        crate::check_loaded_with_bindings((program, diagnostics), &generated_starts)?,
        cases,
    ))
}

/// Compiler-generated scaffolding binds its prelude references explicitly.
/// Only appended source is affected; application shadowing retains normal semantics.
pub(crate) fn bind_generated_prelude(
    program: &crate::Program,
    resolved: &mut tarn_resolve::Resolved,
    starts: &HashMap<PathBuf, usize>,
) {
    let bindings: HashMap<_, _> = ["print", "panic", "i32", "Ok", "Err"]
        .into_iter()
        .filter_map(|name| {
            resolved
                .scope(tarn_resolve::ScopeId(0))
                .get(name)
                .map(|id| (name, id))
        })
        .collect();
    for table in &mut resolved.tables {
        for usage in table.uses.values_mut() {
            let source = program.sources.file(usage.span.file);
            if starts
                .get(&PathBuf::from(&source.name))
                .is_some_and(|start| usage.span.start as usize >= *start)
            {
                if let Some(symbol) = bindings.get(program.sources.snippet(usage.span)) {
                    usage.res = tarn_resolve::Res::Symbol(*symbol);
                }
            }
        }
    }
}
