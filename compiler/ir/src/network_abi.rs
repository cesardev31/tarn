//! Structural validation of private, trusted stdlib networking declarations.
use tarn_types::{IntTy, Ty, Typed};

pub(crate) fn verify(t: &Typed) -> Vec<String> {
    let d = &t.decls;
    if d.net_intrinsics.is_empty() && d.net_sockets.is_empty() && d.net_poll.is_none() && d.exec_owner.is_none() && d.exec_waker.is_none() && d.exec_state.is_none() {
        return Vec::new();
    }
    let invalid = || vec!["invalid network declaration ABI".to_string()];
    if d.net_sockets.len() != 4 || d.net_intrinsics.len() != 34 {
        return invalid();
    }
    // Async-body primitives (ADR 0037): exactly `_with_waker<R>(mut fn(&Waker) R) R`
    // and `_async_park()`, both private trusted intrinsics.
    let primitive = |id: Option<tarn_resolve::SymbolId>| id.and_then(|id| d.fns.get(&id)).filter(|sig| sig.abi.as_deref() == Some("intrinsic") && sig.receiver.is_none());
    let (Some(with_waker), Some(park)) = (primitive(d.exec_async_waker), primitive(d.exec_async_park)) else { return invalid() };
    let waker_ref = d.exec_waker.map(|w| Ty::Ref(false, Box::new(Ty::Adt(w, Vec::new()))));
    let [generic] = with_waker.generics.as_slice() else { return invalid() };
    if with_waker.ret != Ty::Param(*generic)
        || with_waker.params != vec![Ty::Fn(tarn_types::CallMode::Mutable, waker_ref.into_iter().collect(), Box::new(Ty::Param(*generic)))]
        || !park.generics.is_empty() || !park.params.is_empty() || park.ret != Ty::Void
    {
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
                "UnexpectedEof",
                "LimitExceeded",
            ]
    {
        return invalid();
    }
    if !error_def.fields[0].is_pub || error_def.fields[1..].iter().any(|f| f.ty != i32_ty || f.is_pub) {
        return invalid();
    }
    let Some(poll_id) = d.net_poll else { return invalid() };
    let Some(poll) = d.structs.get(&poll_id) else { return invalid() };
    if poll.is_copy || !poll.generics.is_empty() || poll.fields.len() != 1
        || poll.fields[0].name != "handle" || poll.fields[0].is_pub
        || poll.fields[0].ty != Ty::Int(IntTy::Usize)
        || !d.native_capabilities.get(&poll_id).is_some_and(|c| c.transfer && !c.share) { return invalid(); }
    let Some(wait) = d.net_intrinsics.get("net._poll_wait").and_then(|id| d.fns.get(id)) else { return invalid() };
    let Some(Ty::Ref(true, slice)) = wait.params.get(1) else { return invalid() };
    let Ty::Slice(event) = slice.as_ref() else { return invalid() };
    let Ty::Adt(event_id, event_args) = event.as_ref() else { return invalid() };
    let Some(event_def) = d.structs.get(event_id) else { return invalid() };
    if !event_args.is_empty() || !event_def.is_copy || !event_def.generics.is_empty() || event_def.fields.len() != 5
        || event_def.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>() != ["token", "readable", "writable", "error", "hangup"]
        || event_def.fields.iter().any(|f| !f.is_pub) || event_def.fields[1..].iter().any(|f| f.ty != Ty::Bool) { return invalid(); }
    let Ty::Adt(token_id, token_args) = &event_def.fields[0].ty else { return invalid() };
    let Some(token) = d.structs.get(token_id) else { return invalid() };
    if !token_args.is_empty() || !token.is_copy || !token.generics.is_empty() || token.fields.len() != 1
        || token.fields[0].name != "value" || token.fields[0].is_pub || token.fields[0].ty != Ty::Int(IntTy::U64) { return invalid(); }
    let events = Ty::Slice(event.clone());
    let Some(waker_id) = d.exec_waker else { return invalid() };
    let Some(state_id) = d.exec_state else { return invalid() };
    for id in [waker_id, state_id] {
        let Some(def) = d.structs.get(&id) else { return invalid() };
        if def.is_copy || !def.generics.is_empty() || def.fields.len() != 1
            || def.fields[0].name != "native" || def.fields[0].is_pub
            || def.fields[0].ty != Ty::Int(IntTy::Usize)
            || !d.native_capabilities.get(&id).is_some_and(|c| !c.transfer && !c.share) { return invalid(); }
    }
    let Some(new_waker) = d.net_intrinsics.get("net._waker_new").and_then(|id| d.fns.get(id)) else { return invalid() };
    let [Ty::Ref(false, owner)] = new_waker.params.as_slice() else { return invalid() };
    let Ty::Adt(owner_id, owner_args) = owner.as_ref() else { return invalid() };
    let Some(owner_def) = d.structs.get(owner_id) else { return invalid() };
    if Some(*owner_id) != d.exec_owner || !owner_args.is_empty() || owner_def.is_copy || !owner_def.generics.is_empty()
        || owner_def.fields.len() != 2 || owner_def.fields[0].name != "state" || owner_def.fields[1].name != "poll"
        || owner_def.fields.iter().any(|f| f.is_pub)
        || owner_def.fields[0].ty != Ty::Adt(state_id, vec![]) || owner_def.fields[1].ty != Ty::Adt(poll_id, vec![])
        || !d.native_capabilities.get(owner_id).is_some_and(|c| !c.transfer && !c.share)
        || new_waker.ret != Ty::Adt(waker_id, vec![]) || new_waker.abi.as_deref() != Some("intrinsic")
        || new_waker.receiver.is_some() || !new_waker.generics.is_empty()
        || new_waker.contract.parameters != [tarn_types::PassingMode::SharedBorrow]
        || new_waker.contract.result != tarn_types::ResultContract::Borrowed(vec![0]) { return invalid(); }
    let wake = Ty::Ref(false, Box::new(Ty::Adt(waker_id, vec![])));
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
        ("shutdown", vec![i32_ty.clone(), Ty::Int(IntTy::I32)]),
        ("nonblocking", vec![i32_ty.clone(), Ty::Bool]),
        ("mode", vec![i32_ty.clone()]),
        ("connected", vec![i32_ty.clone()]),
        ("now", vec![]),
        ("poll_new", vec![]),
        ("poll_ctl", vec![Ty::Int(IntTy::Usize), i32_ty.clone(), i32_ty.clone(), i32_ty.clone(), Ty::Int(IntTy::U64)]),
        ("poll_wait", vec![Ty::Int(IntTy::Usize), Ty::Ref(true, Box::new(events)), i32_ty.clone()]),
        ("close_poll", vec![Ty::Adt(poll_id, vec![])]),
        ("exec_new", vec![Ty::Int(IntTy::Usize)]),
        ("wake", vec![wake.clone()]),
        ("wake_link", vec![wake.clone(), wake.clone()]),
        ("wake_owner", vec![wake.clone(), Ty::Int(IntTy::Usize)]),
        ("wake_take", vec![wake.clone()]),
        ("wake_arm", vec![wake.clone(), i32_ty.clone(), i32_ty.clone()]),
        ("wake_clear", vec![wake]),
        ("exec_wait", vec![Ty::Int(IntTy::Usize), i32_ty.clone()]),
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
