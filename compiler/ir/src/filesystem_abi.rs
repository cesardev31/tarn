//! Phase 15B resource and syscall ABI, keyed to trusted resolved declarations.
use tarn_types::{IntTy, Ty, Typed};

pub(crate) fn verify(t: &Typed) -> Vec<String> {
    let d = &t.decls;
    if d.fs_intrinsics.is_empty() && d.fs_file.is_none() && d.fs_directory.is_none() {
        return vec![];
    }
    let invalid = || vec!["invalid filesystem declaration ABI".into()];
    let (Some(file), Some(directory)) = (d.fs_file, d.fs_directory) else {
        return invalid();
    };
    let i32_ty = Ty::Int(IntTy::I32);
    let usize_ty = Ty::Int(IntTy::Usize);
    for (id, field, ty) in [
        (file, "fd", i32_ty.clone()),
        (directory, "native", usize_ty.clone()),
    ] {
        let Some(def) = d.structs.get(&id) else {
            return invalid();
        };
        if def.is_copy
            || !def.generics.is_empty()
            || def.fields.len() != 1
            || def.fields[0].name != field
            || def.fields[0].ty != ty
            || def.fields[0].is_pub
            || !d
                .native_capabilities
                .get(&id)
                .is_some_and(|c| c.transfer && !c.share)
        {
            return invalid();
        }
    }
    let Some(raw) = d
        .net_intrinsics
        .get("io._now")
        .and_then(|id| d.fns.get(id))
        .map(|sig| sig.ret.clone())
    else {
        return invalid();
    };
    let path = Ty::Ref(false, Box::new(Ty::Str));
    let bytes = Ty::Slice(Box::new(Ty::Int(IntTy::U8)));
    let operations = [
        ("open", vec![path.clone(), i32_ty.clone()]),
        (
            "read",
            vec![i32_ty.clone(), Ty::Ref(true, Box::new(bytes.clone()))],
        ),
        (
            "write",
            vec![i32_ty.clone(), Ty::Ref(false, Box::new(bytes.clone()))],
        ),
        (
            "seek",
            vec![i32_ty.clone(), Ty::Int(IntTy::I64), i32_ty.clone()],
        ),
        ("metadata", vec![path.clone()]),
        ("file_metadata", vec![i32_ty.clone()]),
        ("sync", vec![i32_ty]),
        ("close", vec![Ty::Adt(file, vec![])]),
        ("mkdir", vec![path.clone()]),
        ("remove_file", vec![path.clone()]),
        ("remove_dir", vec![path.clone()]),
        ("rename", vec![path.clone(), path.clone()]),
        ("dir_open", vec![path]),
        ("dir_next", vec![usize_ty, Ty::Ref(true, Box::new(bytes))]),
    ];
    if d.fs_intrinsics.len() != operations.len() {
        return invalid();
    }
    for (operation, params) in operations {
        let Some(sig) = d
            .fs_intrinsics
            .get(&format!("fs._{operation}"))
            .and_then(|id| d.fns.get(id))
        else {
            return invalid();
        };
        if sig.abi.as_deref() != Some("intrinsic")
            || sig.receiver.is_some()
            || !sig.generics.is_empty()
            || sig.params != params
            || sig.ret != raw
        {
            return invalid();
        }
    }
    vec![]
}
