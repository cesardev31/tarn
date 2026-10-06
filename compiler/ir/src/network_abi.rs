//! Structural validation of private, trusted stdlib networking declarations.
use tarn_types::{IntTy, Ty, Typed};

pub(crate) fn verify(t: &Typed) -> Vec<String> {
    let d = &t.decls;
    if d.net_intrinsics.is_empty() && d.net_sockets.is_empty() {
        return Vec::new();
    }
    let invalid = || vec!["invalid network declaration ABI".to_string()];
    if d.net_sockets.len() != 3 || d.net_intrinsics.len() != 15 {
        return invalid();
    }
    let i32_ty = Ty::Int(IntTy::I32);
    let Some(raw) = d.net_intrinsics.get("net._resolve").and_then(|id| d.fns.get(id)).map(|sig| sig.ret.clone()) else { return invalid() };
    let Ty::Adt(raw_id, raw_args) = &raw else { return invalid() };
    let Some(raw_def) = d.structs.get(raw_id) else { return invalid() };
    if !raw_args.is_empty()
        || !raw_def.is_copy
        || !raw_def.generics.is_empty()
        || raw_def.fields.len() != 4
        || raw_def.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>() != ["domain", "code", "value", "address"]
    {
        return invalid();
    }
    let addr = raw_def.fields[3].ty.clone();
    let Ty::Adt(addr_id, addr_args) = &addr else { return invalid() };
    let Some(addr_def) = d.structs.get(addr_id) else { return invalid() };
    if !addr_args.is_empty()
        || !addr_def.generics.is_empty()
        || !addr_def.is_copy
        || addr_def.fields.len() != 1
        || addr_def.fields[0].name != "data"
        || addr_def.fields[0].is_pub
        || addr_def.fields[0].ty != Ty::Array(Box::new(Ty::Int(IntTy::U8)), 24)
    {
        return invalid();
    }
    let fields = [i32_ty.clone(), i32_ty.clone(), Ty::Int(IntTy::I64), addr.clone()];
    if raw_def.fields.iter().zip(fields).any(|(f, ty)| f.ty != ty || f.is_pub) {
        return invalid();
    }
    for id in &d.net_sockets {
        let Some(def) = d.structs.get(id) else { return invalid() };
        if def.is_copy
            || !def.generics.is_empty()
            || def.fields.len() != 1
            || def.fields[0].name != "fd"
            || def.fields[0].ty != i32_ty
            || def.fields[0].is_pub
            || !d.native_capabilities.get(id).is_some_and(|c| c.transfer && !c.share)
        {
            return invalid();
        }
    }
    let Some(error_id) = d.net_error else { return invalid() };
    let Some(error_def) = d.structs.get(&error_id) else { return invalid() };
    if !error_def.is_copy
        || !error_def.generics.is_empty()
        || error_def.fields.len() != 3
        || error_def.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>() != ["kind", "code", "domain"]
    {
        return invalid();
    }
    let Ty::Adt(kind_id, kind_args) = &error_def.fields[0].ty else { return invalid() };
    let Some(kind) = d.enums.get(kind_id) else { return invalid() };
    if !kind_args.is_empty()
        || !kind.is_copy
        || !kind.generics.is_empty()
        || kind.variants.iter().any(|v| !v.fields.is_empty())
        || kind.variants.iter().map(|v| v.name.as_str()).collect::<Vec<_>>()
            != [
                "AddressInUse",
                "ConnectionRefused",
                "ConnectionReset",
                "BrokenPipe",
                "TimedOut",
                "WouldBlock",
                "InvalidAddress",
                "DnsFailure",
                "OtherOs",
                "WriteZero",
            ]
    {
        return invalid();
    }
    if !error_def.fields[0].is_pub || error_def.fields[1..].iter().any(|f| f.ty != i32_ty || f.is_pub) {
        return invalid();
    }
    let bytes = Ty::Slice(Box::new(Ty::Int(IntTy::U8)));
    for (operation, params) in [
        ("resolve", vec![Ty::Ref(false, Box::new(Ty::Str))]),
        ("socket", vec![addr.clone(), Ty::Bool]),
        ("bind", vec![i32_ty.clone(), addr.clone()]),
        ("listen", vec![i32_ty.clone()]),
        ("accept", vec![i32_ty.clone()]),
        ("connect", vec![i32_ty.clone(), addr.clone()]),
        ("read", vec![i32_ty.clone(), Ty::Ref(true, Box::new(bytes.clone()))]),
        ("write", vec![i32_ty.clone(), Ty::Ref(false, Box::new(bytes.clone()))]),
        ("recv", vec![i32_ty.clone(), Ty::Ref(true, Box::new(bytes.clone()))]),
        ("send", vec![i32_ty.clone(), Ty::Ref(false, Box::new(bytes)), addr]),
        ("addr", vec![i32_ty.clone(), Ty::Bool]),
        ("shutdown", vec![i32_ty, Ty::Int(IntTy::I32)]),
        ("close_listener", vec![Ty::Adt(d.net_sockets[0], vec![])]),
        ("close_stream", vec![Ty::Adt(d.net_sockets[1], vec![])]),
        ("close_udp", vec![Ty::Adt(d.net_sockets[2], vec![])]),
    ] {
        let Some(sig) = d.net_intrinsics.get(&format!("net._{operation}")).and_then(|id| d.fns.get(id)) else { return invalid() };
        if sig.abi.as_deref() != Some("intrinsic") || sig.receiver.is_some() || !sig.generics.is_empty() || sig.params != params || sig.ret != raw {
            return invalid();
        }
    }
    Vec::new()
}
