use super::*;

impl Cx<'_, '_> {
    pub(super) fn networking(&mut self, name: &str, args: &[Val], dest: &Ty) -> Result<Option<Val>> {
        let Some(id) = self.t.decls.net_intrinsics.get(name) else {
            if name.starts_with("net._") {
                return Err(Error::bug("unregistered network intrinsic"));
            }
            return Ok(None);
        };
        let sig = &self.t.decls.fns[id];
        if sig.ret != *dest || args.len() != sig.params.len() || args.iter().zip(&sig.params).any(|(a, ty)| a.ty != *ty) {
            return Err(Error::bug("network intrinsic signature mismatch"));
        }
        if name == "net._poll_wait" {
            let Ty::Ref(true, slice) = &sig.params[1] else { return Err(Error::bug("readiness buffer ABI")); };
            let Ty::Slice(event) = slice.as_ref() else { return Err(Error::bug("readiness event ABI")); };
            let event_layout = layout::layout(self.t, event)?;
            if event_layout.size != 16 || event_layout.align != 8
                || event_layout.fields.iter().map(|(offset, _)| *offset).collect::<Vec<_>>() != [0, 8, 9, 10, 11] {
                return Err(Error::bug("readiness event layout mismatch"));
            }
        }
        let result_layout = layout::layout(self.t, dest)?;
        if result_layout.size != 40 || result_layout.align != 8 || result_layout.fields.iter().map(|(offset, _)| *offset).collect::<Vec<_>>() != [0, 4, 8, 16] {
            return Err(Error::bug("network result ABI mismatch"));
        }
        let slot = self.stack(result_layout.size, result_layout.align);
        let out = self.b.ins().stack_addr(types::I64, slot, 0);
        let mut values = vec![out];
        for arg in args {
            let value = arg.value.ok_or_else(|| Error::bug("network intrinsic value missing"))?;
            match &arg.ty {
                Ty::Ref(_, inner) if matches!(inner.as_ref(), Ty::Slice(_)) => {
                    values.push(self.b.ins().load(types::I64, cl::MemFlags::new(), value, 0));
                    values.push(self.b.ins().load(types::I64, cl::MemFlags::new(), value, 8));
                }
                Ty::Ref(_, inner) if **inner == Ty::Str => values.push(self.b.ins().load(types::I64, cl::MemFlags::new(), value, 0)),
                Ty::Adt(id, _) if Some(*id) == self.t.decls.net_poll => values.push(self.b.ins().load(types::I64, cl::MemFlags::new(), value, 0)),
                Ty::Adt(id, _) if self.t.decls.net_sockets.contains(id) => values.push(self.b.ins().load(types::I32, cl::MemFlags::new(), value, 0)),
                _ => values.push(value),
            }
        }
        let operation = name.strip_prefix("net._").ok_or_else(|| Error::bug("network intrinsic name"))?;
        let operation = if operation.starts_with("close_") && operation != "close_poll" { "close" } else { operation };
        self.runtime(&format!("tarn_rt_net_{operation}"), &values);
        Ok(Some(Val { value: Some(out), ty: dest.clone() }))
    }
}
