use super::*;

impl Cx<'_, '_> {
    /// Raw pointer intrinsics of the trusted `ffi` module (ADR 0047). Pointers
    /// are plain 64-bit addresses; none of these operations access memory.
    pub(super) fn ffi(&mut self, name: &str, args: &[Val], dest: &Ty) -> Result<Option<Val>> {
        let Some(operation) = name.strip_prefix("ffi.") else { return Ok(None) };
        let word = |v: &Val| v.value.ok_or_else(|| Error::bug("ffi operand"));
        // `of(&data)` with an inferred unsized T would yield the address of a
        // temporary (pointer, length) pair, not of the data.
        if let ("of" | "of_mut", [a]) = (operation, args)
            && matches!(&a.ty, Ty::Ref(_, inner) if matches!(inner.as_ref(), Ty::Slice(_) | Ty::Any(_)))
        {
            return Err(Error::unsupported("ffi.of on a slice or dynamic reference; use ffi.slice"));
        }
        let value = match (operation, args, dest) {
            ("null", [], Ty::Ptr(true, _)) => self.b.ins().iconst(types::I64, 0),
            ("of", [a], Ty::Ptr(false, _)) if matches!(a.ty, Ty::Ref(false, _)) => word(a)?,
            ("of_mut", [a], Ty::Ptr(true, _)) if matches!(a.ty, Ty::Ref(true, _)) => word(a)?,
            ("slice", [a], Ty::Ptr(false, _)) | ("slice_mut", [a], Ty::Ptr(true, _))
                if matches!(&a.ty, Ty::Ref(_, s) if matches!(s.as_ref(), Ty::Slice(_))) =>
            {
                let fat = word(a)?;
                self.b.ins().load(types::I64, cl::MemFlags::new(), fat, 0)
            }
            ("to_const", [a], Ty::Ptr(false, _)) if matches!(a.ty, Ty::Ptr(true, _)) => word(a)?,
            ("address", [a], Ty::Int(IntTy::Usize)) if matches!(a.ty, Ty::Ptr(..)) => word(a)?,
            ("from_address", [a], Ty::Ptr(true, _)) if a.ty == Ty::Int(IntTy::Usize) => word(a)?,
            _ => return Err(Error::bug(format!("ffi intrinsic {name} shape"))),
        };
        Ok(Some(Val { value: Some(value), ty: dest.clone() }))
    }
}
