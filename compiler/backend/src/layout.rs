//! The single Linux x86_64 layout authority for native code and runtime ABI.
use crate::{Error, Result};
use tarn_types::{Ty, Typed};

#[derive(Clone, Debug)]
pub struct Layout {
    pub size: u32,
    pub align: u32,
    pub fields: Vec<(u32, Ty)>,
    pub variants: Vec<Vec<(u32, Ty)>>,
}
fn align(n: u32, a: u32) -> Result<u32> {
    n.checked_add(a - 1).map(|x| x & !(a - 1)).ok_or_else(|| Error::unsupported("layout exceeds 32-bit object size"))
}
pub fn layout(t: &Typed, ty: &Ty) -> Result<Layout> {
    let l = layout_inner(t, ty, &mut Vec::new())?;
    // Correctness-first explicit copies/drop glue are bounded, not silently
    // expanded into millions of instructions for huge fixed-size objects.
    if l.size > 65536 {
        return Err(Error::unsupported("native value size exceeds v0 limit of 64 KiB"));
    }
    Ok(l)
}
fn layout_inner(t: &Typed, ty: &Ty, seen: &mut Vec<Ty>) -> Result<Layout> {
    if seen.len() >= 64 {
        return Err(Error::unsupported("native aggregate nesting exceeds 64 levels"));
    }
    if seen.contains(ty) {
        return Err(Error::unsupported("recursive by-value layout"));
    }
    let scalar = |size| Layout { size, align: size.max(1), fields: Vec::new(), variants: Vec::new() };
    Ok(match ty {
        Ty::Void | Ty::Never => scalar(0),
        Ty::Bool => scalar(1),
        Ty::Int(i) => scalar((crate::codegen::int_bits(*i) / 8) as u32),
        Ty::Float(tarn_types::FloatTy::F32) => scalar(4),
        Ty::Float(_) => scalar(8),
        Ty::Str => scalar(8),
        Ty::Ref(_, inner) => {
            if matches!(inner.as_ref(), Ty::Slice(_) | Ty::Any(_)) {
                Layout { size: 16, align: 8, fields: Vec::new(), variants: Vec::new() }
            } else {
                scalar(8)
            }
        }
        Ty::Fn(..) => Layout { size: 16, align: 8, fields: Vec::new(), variants: Vec::new() },
        Ty::Array(elem, n) => {
            if *n > 4096 {
                return Err(Error::unsupported("fixed arrays exceed v0 limit of 4096 elements"));
            }
            let el = layout_inner(t, elem, seen)?;
            let size = u64::from(el.size).checked_mul(*n).and_then(|n| u32::try_from(n).ok()).ok_or_else(|| Error::unsupported("array layout too large"))?;
            Layout { size, align: el.align, fields: Vec::new(), variants: Vec::new() }
        }
        Ty::Adt(s, args) => {
            seen.push(ty.clone());
            let result = if let Some(def) = t.decls.structs.get(s) {
                let map = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                let ts: Vec<_> = def.fields.iter().map(|f| tarn_types::subst(&f.ty, &map)).collect();
                record(t, &ts, seen)?
            } else if let Some(def) = t.decls.enums.get(s) {
                let map = def.generics.iter().copied().zip(args.iter().cloned()).collect();
                let mut variants = Vec::new();
                let mut size = 4;
                let mut alignment = 4;
                for v in &def.variants {
                    let ts: Vec<_> = v.fields.iter().map(|ty| tarn_types::subst(ty, &map)).collect();
                    let payload = record(t, &ts, seen)?;
                    alignment = alignment.max(payload.align);
                    variants.push(payload);
                }
                // u32 declaration-index tag, followed by an aligned payload union.
                let offset = align(4, alignment)?;
                let fields = variants
                    .into_iter()
                    .map(|v| {
                        size = size.max(offset + v.size);
                        v.fields.into_iter().map(|(o, ty)| (offset + o, ty)).collect()
                    })
                    .collect();
                Layout { size: align(size, alignment)?, align: alignment, fields: Vec::new(), variants: fields }
            } else {
                return Err(Error::unsupported("opaque/prelude resource layout"));
            };
            seen.pop();
            result
        }
        other => return Err(Error::unsupported(format!("type {other:?}"))),
    })
}
fn record(t: &Typed, ts: &[Ty], seen: &mut Vec<Ty>) -> Result<Layout> {
    let mut fields = Vec::new();
    let mut size = 0u32;
    let mut alignment = 1;
    for ty in ts {
        let l = layout_inner(t, ty, seen)?;
        size = align(size, l.align)?;
        fields.push((size, ty.clone()));
        size = size.checked_add(l.size).ok_or_else(|| Error::unsupported("struct layout too large"))?;
        alignment = alignment.max(l.align);
    }
    Ok(Layout { size: align(size, alignment)?, align: alignment, fields, variants: Vec::new() })
}
