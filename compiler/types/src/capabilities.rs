//! Bounded structural cross-thread capability queries. Callable signatures are
//! deliberately insufficient evidence about erased capture environments.
use crate::{Decls, Ty, subst};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability { Transfer, Share }

/// Explicit trusted declaration evidence; absence grants no authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeCapabilities { pub transfer: bool, pub share: bool }

impl Decls {
    /// Resource containment, independent of initialization. Unknown generic
    /// substitutions may contain tasks; cycles conservatively retain loans.
    /// Conservative semantic reference containment for deferred borrowed task
    /// results. This is declaration inspection, not a second provenance analysis.
    pub fn may_contain_references(&self, ty: &Ty) -> bool {
        fn visit(d: &Decls, ty: &Ty, path: &mut Vec<Ty>, budget: &mut usize) -> bool {
            if *budget == 0 || path.len() >= 64 || path.contains(ty) { return true; }
            *budget -= 1;
            path.push(ty.clone());
            let answer = match ty {
                Ty::Ref(..) | Ty::Fn(..) | Ty::Param(_) | Ty::Any(_) | Ty::Opaque | Ty::Var(_) => true,
                Ty::Adt(s, _) if Some(*s) == d.mutex_guard => true,
                Ty::Array(t, _) | Ty::Slice(t) => visit(d, t, path, budget),
                Ty::Adt(s, args) if Some(*s) == d.task => args.iter().any(|t| visit(d, t, path, budget)),
                Ty::Adt(s, args) => {
                    if let Some(def) = d.structs.get(s) {
                        let map = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                        def.fields.iter().any(|f| visit(d, &subst(&f.ty, &map), path, budget))
                    } else if let Some(def) = d.enums.get(s) {
                        let map = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                        def.variants.iter().any(|v| v.fields.iter().any(|f| visit(d, &subst(f, &map), path, budget)))
                    } else { true }
                }
                _ => false,
            };
            path.pop();
            answer
        }
        visit(self, ty, &mut Vec::new(), &mut 4096)
    }

    pub fn contains_guard(&self, ty: &Ty) -> bool {
        self.contains_resource(ty, self.mutex_guard)
    }

    pub fn contains_task(&self, ty: &Ty) -> bool {
        self.contains_resource(ty, self.task)
    }

    fn contains_resource(&self, ty: &Ty, target: Option<tarn_resolve::SymbolId>) -> bool {
        fn visit(d: &Decls, ty: &Ty, path: &mut Vec<Ty>, budget: &mut usize, target: Option<tarn_resolve::SymbolId>) -> bool {
            if *budget == 0 || path.len() >= 64 || path.contains(ty) { return true; }
            *budget -= 1;
            path.push(ty.clone());
            let answer = match ty {
                Ty::Adt(s, _) if Some(*s) == target => true,
                Ty::Adt(s, args) => {
                    if let Some(def) = d.structs.get(s) {
                        let map = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                        def.fields.iter().any(|f| visit(d, &subst(&f.ty, &map), path, budget, target))
                    } else if let Some(def) = d.enums.get(s) {
                        let map = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                        def.variants.iter().any(|v| v.fields.iter().any(|f| visit(d, &subst(f, &map), path, budget, target)))
                    } else { false }
                }
                Ty::Array(t, _) | Ty::Slice(t) | Ty::Ref(_, t) => visit(d, t, path, budget, target),
                Ty::Param(_) | Ty::Opaque => true,
                _ => false,
            };
            path.pop();
            answer
        }
        visit(self, ty, &mut Vec::new(), &mut 4096, target)
    }

    pub fn capability(&self, ty: &Ty, capability: Capability) -> bool {
        self.capability_inner(ty, capability, &mut Vec::new(), &mut 4096)
    }

    fn capability_inner(&self, ty: &Ty, cap: Capability, path: &mut Vec<(Ty, Capability)>, budget: &mut usize) -> bool {
        if *budget == 0 || path.len() >= 64 || path.iter().any(|(t, c)| t == ty && *c == cap) { return false; }
        *budget -= 1;
        path.push((ty.clone(), cap));
        let answer = match ty {
            Ty::Bool | Ty::Int(_) | Ty::Float(_) | Ty::Str | Ty::Void | Ty::Never | Ty::Error => true,
            Ty::Param(p) => {
                let marker = match cap { Capability::Transfer => self.transfer, Capability::Share => self.share };
                marker.is_some_and(|s| self.bounds.get(p).is_some_and(|b| b.contains(&s)))
            }
            Ty::Ref(false, t) => self.capability_inner(t, Capability::Share, path, budget),
            Ty::Ref(true, t) => cap == Capability::Transfer && self.capability_inner(t, Capability::Transfer, path, budget),
            Ty::Array(t, _) | Ty::Slice(t) => self.capability_inner(t, cap, path, budget),
            Ty::Adt(s, _) if Some(*s) == self.mutex_guard => false,
            Ty::Adt(s, args) if Some(*s) == self.mutex => args.len() == 1 && self.capability_inner(&args[0], Capability::Transfer, path, budget),
            Ty::Adt(s, _) if self.atomics.contains_key(s) => true,
            Ty::Adt(s, args) if Some(*s) == self.task => cap == Capability::Transfer && args.len() == 1 && self.capability_inner(&args[0], cap, path, budget),
            Ty::Adt(s, args) => {
                if let Some(contract) = self.native_capabilities.get(s) {
                    match cap { Capability::Transfer => contract.transfer, Capability::Share => contract.share }
                } else if let Some(d) = self.structs.get(s) {
                    let map: HashMap<_, _> = d.generics.iter().copied().zip(args.iter().cloned()).collect();
                    args.len() == d.generics.len() && d.fields.iter().all(|f| self.capability_inner(&subst(&f.ty, &map), cap, path, budget))
                } else if let Some(d) = self.enums.get(s) {
                    let map: HashMap<_, _> = d.generics.iter().copied().zip(args.iter().cloned()).collect();
                    args.len() == d.generics.len() && d.variants.iter().all(|v| v.fields.iter().all(|t| self.capability_inner(&subst(t, &map), cap, path, budget)))
                } else { false }
            }
            Ty::Any(s) => self.native_capabilities.get(s).is_some_and(|c| match cap { Capability::Transfer => c.transfer, Capability::Share => c.share }),
            Ty::Fn(..) | Ty::Opaque | Ty::Var(_) => false,
        };
        path.pop();
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IntTy, CallMode};
    #[test]
    fn references_and_erasure_require_evidence() {
        let d = Decls::default();
        let integer = Ty::Int(IntTy::I32);
        assert!(d.capability(&Ty::Ref(false, Box::new(integer.clone())), Capability::Transfer));
        assert!(d.capability(&Ty::Ref(true, Box::new(integer.clone())), Capability::Transfer));
        assert!(!d.capability(&Ty::Ref(true, Box::new(integer)), Capability::Share));
        assert!(!d.capability(&Ty::Fn(CallMode::Shared, vec![], Box::new(Ty::Void)), Capability::Transfer));
        assert!(!d.capability(&Ty::Opaque, Capability::Transfer));
    }
    #[test]
    fn structural_queries_substitute_and_bound_recursive_declarations() {
        use crate::{FieldDef, ParamId, StructDef};
        use tarn_resolve::{ModuleId, SymbolId};
        let symbol = SymbolId(100);
        let parameter = ParamId(101);
        let mut d = Decls::default();
        d.structs.insert(symbol, StructDef { module: ModuleId(0), generics: vec![parameter],
            fields: vec![FieldDef { name: "value".into(), ty: Ty::Param(parameter), is_pub: true }], is_copy: false });
        assert!(d.capability(&Ty::Adt(symbol, vec![Ty::Str]), Capability::Transfer));
        assert!(!d.capability(&Ty::Adt(symbol, vec![Ty::Opaque]), Capability::Transfer));
        d.structs.get_mut(&symbol).unwrap().fields[0].ty = Ty::Adt(symbol, vec![Ty::Param(parameter)]);
        assert!(!d.capability(&Ty::Adt(symbol, vec![Ty::Str]), Capability::Share));
        d.structs.get_mut(&symbol).unwrap().fields[0].ty = Ty::Adt(symbol, vec![Ty::Array(Box::new(Ty::Param(parameter)), 1)]);
        assert!(!d.capability(&Ty::Adt(symbol, vec![Ty::Str]), Capability::Transfer));
    }

    #[test]
    fn native_and_dynamic_authority_is_explicit_and_independent() {
        use tarn_resolve::SymbolId;
        let symbol = SymbolId(100);
        let mut d = Decls::default();
        assert!(!d.capability(&Ty::Adt(symbol, vec![]), Capability::Transfer));
        assert!(!d.capability(&Ty::Any(symbol), Capability::Share));
        d.native_capabilities.insert(symbol, NativeCapabilities { transfer: true, share: false });
        assert!(d.capability(&Ty::Adt(symbol, vec![]), Capability::Transfer));
        assert!(!d.capability(&Ty::Adt(symbol, vec![]), Capability::Share));
        assert!(!d.capability(&Ty::Ref(false, Box::new(Ty::Any(symbol))), Capability::Transfer));
        d.native_capabilities.get_mut(&symbol).unwrap().share = true;
        assert!(d.capability(&Ty::Ref(false, Box::new(Ty::Any(symbol))), Capability::Transfer));
    }

    #[test]
    fn synchronization_authority_is_distinct_from_copy_and_payload_share() {
        use tarn_resolve::SymbolId;
        let (mutex, guard, atomic, payload) = (SymbolId(100), SymbolId(101), SymbolId(102), SymbolId(103));
        let mut d = Decls { mutex: Some(mutex), mutex_guard: Some(guard), ..Decls::default() };
        d.atomics.insert(atomic, Ty::Bool);
        d.native_capabilities.insert(payload, NativeCapabilities { transfer: true, share: false });
        let payload = Ty::Adt(payload, vec![]);
        assert!(!d.capability(&payload, Capability::Share));
        let synchronized = Ty::Adt(mutex, vec![payload.clone()]);
        assert!(d.capability(&synchronized, Capability::Transfer));
        assert!(d.capability(&synchronized, Capability::Share));
        assert!(!d.is_copy(&synchronized));
        let guard = Ty::Adt(guard, vec![payload]);
        assert!(!d.capability(&guard, Capability::Transfer));
        assert!(!d.capability(&guard, Capability::Share));
        assert!(!d.is_copy(&guard));
        assert!(d.may_contain_references(&guard));
        assert!(d.contains_guard(&Ty::Array(Box::new(guard), 2)));
        let atomic = Ty::Adt(atomic, vec![]);
        assert!(d.capability(&atomic, Capability::Transfer));
        assert!(d.capability(&atomic, Capability::Share));
        assert!(!d.is_copy(&atomic));
        assert!(!d.capability(&Ty::Adt(mutex, vec![Ty::Opaque]), Capability::Share));
        let parameter = crate::ParamId(104);
        d.transfer = Some(SymbolId(105));
        d.share = Some(SymbolId(106));
        d.bounds.insert(parameter, vec![d.share.unwrap()]);
        let generic = Ty::Adt(mutex, vec![Ty::Param(parameter)]);
        assert!(!d.capability(&generic, Capability::Share));
        d.bounds.insert(parameter, vec![d.transfer.unwrap()]);
        assert!(d.capability(&generic, Capability::Share));
    }

}
