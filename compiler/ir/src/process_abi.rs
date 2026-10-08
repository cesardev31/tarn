//! Phase 15D completion/pipe resources and private syscall signatures.
use tarn_types::{IntTy, Ty, Typed};

pub(crate) fn verify(t: &Typed) -> Vec<String> {
    let d = &t.decls;
    if d.process_intrinsics.is_empty() && d.process_owner.is_none() && d.process_pipe.is_none() {
        return vec![];
    }
    let invalid = || vec!["invalid process declaration ABI".into()];
    let (Some(child), Some(pipe)) = (d.process_owner, d.process_pipe) else {
        return invalid();
    };
    let scalar = Ty::Int(IntTy::I32);
    for (id, field) in [(child, "pid"), (pipe, "fd")] {
        let Some(def) = d.structs.get(&id) else {
            return invalid();
        };
        if def.is_copy
            || !def.generics.is_empty()
            || def.fields.len() != 1
            || def.fields[0].name != field
            || def.fields[0].ty != scalar
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
    let text = Ty::Ref(false, Box::new(Ty::Str));
    let operations = [
        (
            "spawn",
            vec![
                text.clone(),
                Ty::Ref(false, Box::new(Ty::Slice(Box::new(Ty::Str)))),
                Ty::Ref(false, Box::new(Ty::Slice(Box::new(Ty::Str)))),
                text,
                Ty::Bool,
                Ty::Bool,
            ],
        ),
        ("wait", vec![Ty::Adt(child, vec![])]),
        ("close", vec![Ty::Adt(pipe, vec![])]),
        ("kill", vec![scalar.clone()]),
        (
            "read",
            vec![
                scalar,
                Ty::Ref(true, Box::new(Ty::Slice(Box::new(Ty::Int(IntTy::U8))))),
            ],
        ),
    ];
    if d.process_intrinsics.len() != operations.len() {
        return invalid();
    }
    for (operation, params) in operations {
        let Some(sig) = d
            .process_intrinsics
            .get(&format!("process._{operation}"))
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
