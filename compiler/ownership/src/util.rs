//! Small shared helpers: a bit set and source-like names for places.

use std::collections::HashMap;
use tarn_ir::{Function, LocalKind, Place, Proj};
use tarn_types::{Ty, Typed};

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    pub fn new(n: usize) -> BitSet {
        BitSet { words: vec![0; n.div_ceil(64).max(1)] }
    }

    pub fn insert(&mut self, i: usize) -> bool {
        let (w, b) = (i / 64, 1u64 << (i % 64));
        let was = self.words[w] & b != 0;
        self.words[w] |= b;
        !was
    }

    pub fn remove(&mut self, i: usize) {
        self.words[i / 64] &= !(1u64 << (i % 64));
    }

    pub fn contains(&self, i: usize) -> bool {
        self.words[i / 64] & (1u64 << (i % 64)) != 0
    }

    /// `self |= other`; returns whether anything changed.
    pub fn union(&mut self, other: &BitSet) -> bool {
        let mut changed = false;
        for (a, b) in self.words.iter_mut().zip(&other.words) {
            let n = *a | *b;
            changed |= n != *a;
            *a = n;
        }
        changed
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(wi, w)| (0..64).filter(move |b| w & (1 << b) != 0).map(move |b| wi * 64 + b))
    }
}

/// Source-like name of a place: `x`, `p.first`, `r.name`, `arr[..]`.
/// Dereferences are not shown: Tarn has no `*` operator, the user wrote
/// `r.name` and auto-deref did the rest.
pub fn place_name(f: &Function, t: &Typed, p: &Place) -> String {
    let l = f.local(p.local);
    let mut name = match l.kind {
        LocalKind::User | LocalKind::Param => l.name.clone().unwrap_or_else(|| "value".into()),
        _ => "a temporary".into(),
    };
    let mut ty = l.ty.clone();
    let mut variant: Option<(Vec<Ty>, String)> = None;
    for proj in &p.proj {
        match proj {
            Proj::Deref => {
                ty = match ty {
                    Ty::Ref(_, inner) => *inner,
                    other => other,
                };
            }
            Proj::Index(_) => {
                name = format!("{name}[..]");
                ty = match ty {
                    Ty::Array(e, _) | Ty::Slice(e) => *e,
                    other => other,
                };
            }
            Proj::Downcast(v) => {
                if let Ty::Adt(e, args) = &ty
                    && let Some(def) = t.decls.enums.get(e)
                {
                    let map: HashMap<_, _> = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                    if let Some(var) = def.variants.get(*v as usize) {
                        variant = Some((var.fields.iter().map(|f| tarn_types::subst(f, &map)).collect(), var.name.clone()));
                        name = format!("{name}.{}", var.name);
                    }
                }
            }
            Proj::Field(i) => {
                if let Some((fields, _)) = variant.take() {
                    name = format!("{name}.{i}");
                    ty = fields.get(*i as usize).cloned().unwrap_or(Ty::Error);
                } else if let Ty::Adt(s, args) = &ty
                    && let Some(def) = t.decls.structs.get(s)
                {
                    let map: HashMap<_, _> = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                    let fld = &def.fields[*i as usize];
                    name = format!("{name}.{}", fld.name);
                    ty = tarn_types::subst(&fld.ty, &map);
                } else {
                    name = format!("{name}.{i}");
                }
            }
        }
    }
    name
}

/// Can a value of this type hold a reference (and therefore loans)?
/// Conservative for type parameters, functions/closures and opaque recovery
/// values. Unmodeled std APIs are rejected before IR (E3040).
pub fn may_hold_refs(t: &Ty) -> bool {
    match t {
        Ty::Ref(..) | Ty::Fn(..) | Ty::Param(_) | Ty::Any(_) | Ty::Opaque => true,
        Ty::Adt(_, args) => args.iter().any(may_hold_refs),
        Ty::Array(e, _) | Ty::Slice(e) => may_hold_refs(e),
        _ => false,
    }
}
