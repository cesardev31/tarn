//! Cranelift-specific lowering. Scalars use frontend SSA variables unless
//! address-taken; aggregate values use canonical-layout stack memory.
use crate::{Error, Result, layout};
use cranelift_codegen::{
    ir::{
        self as cl, InstBuilder,
        condcodes::{FloatCC, IntCC},
        types,
    },
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::collections::{HashMap, HashSet};
use tarn_ir::{self as ir, post_drop as post, *};
use tarn_types::{FloatTy, IntTy, Ty, Typed};

pub fn int_bits(t: IntTy) -> u16 {
    match t {
        IntTy::I8 | IntTy::U8 => 8,
        IntTy::I16 | IntTy::U16 => 16,
        IntTy::I32 | IntTy::U32 => 32,
        _ => 64,
    }
}
fn scalar(ty: &Ty) -> Option<cl::Type> {
    Some(match ty {
        Ty::Bool => types::I8,
        Ty::Int(i) => match int_bits(*i) {
            8 => types::I8,
            16 => types::I16,
            32 => types::I32,
            _ => types::I64,
        },
        Ty::Float(FloatTy::F32) => types::F32,
        Ty::Float(_) => types::F64,
        Ty::Ref(..) | Ty::Str => types::I64,
        _ => return None,
    })
}
fn abi_type(t: &Typed, ty: &Ty) -> Result<Option<cl::Type>> {
    let l = layout::layout(t, ty)?;
    Ok(if l.size == 0 { None } else { Some(scalar(ty).unwrap_or(types::I64)) })
}
fn signature(module: &ObjectModule, t: &Typed, f: &ir::Function) -> Result<cl::Signature> {
    let mut sig = module.make_signature();
    if layout::layout(t, &f.ret)?.size > 0 && scalar(&f.ret).is_none() {
        sig.params.push(cl::AbiParam::new(types::I64));
    }
    for p in f.params() {
        if let Some(ty) = abi_type(t, &f.local(p).ty)? {
            sig.params.push(cl::AbiParam::new(ty));
        }
    }
    if let Some(ty) = abi_type(t, &f.ret)?
        && scalar(&f.ret).is_some()
    {
        sig.returns.push(cl::AbiParam::new(ty));
    }
    Ok(sig)
}

pub fn emit(p: &post::Program, t: &Typed) -> Result<Vec<u8>> {
    let main = p.functions.iter().find(|f| f.decl.name == "main").ok_or_else(|| Error::unsupported("program requires fn main()"))?;
    if main.decl.param_count != 0 || main.decl.ret != Ty::Void {
        return Err(Error::unsupported("entry must be fn main() with no return value"));
    }
    // Only reachable functions are code-generated. Unsupported unused stdlib
    // declarations and generic helpers do not prevent scalar executables.
    let mut reachable = HashSet::new();
    let mut queue = vec![main.decl.id];
    while let Some(id) = queue.pop() {
        if !reachable.insert(id) {
            continue;
        }
        let f = p.functions.get(id.0 as usize).ok_or_else(|| Error::bug("missing direct function"))?;
        if f.blocks.is_empty() {
            return Err(Error::unsupported(format!("function {} has no native body", f.decl.name)));
        }
        if !f.decl.generics.is_empty() {
            return Err(Error::unsupported(format!("generic function {} (monomorphization)", f.decl.name)));
        }
        for b in &f.blocks {
            if let Terminator::Call { callee: Callee::Fn(c, args), .. } = &b.term {
                if !args.is_empty() {
                    return Err(Error::unsupported("generic direct call"));
                }
                queue.push(*c);
            }
        }
    }
    let mut settings = settings::builder();
    settings.set("opt_level", "none").map_err(|e| Error::bug(e.to_string()))?;
    settings.set("is_pic", "false").map_err(|e| Error::bug(e.to_string()))?;
    let isa = cranelift_native::builder().map_err(|e| Error::unsupported(e.to_string()))?.finish(settings::Flags::new(settings)).map_err(|e| Error::bug(e.to_string()))?;
    let builder = ObjectBuilder::new(isa, "tarn", cranelift_module::default_libcall_names()).map_err(|e| Error::bug(e.to_string()))?;
    let mut module = ObjectModule::new(builder);
    let mut ids = HashMap::new();
    let mut ordered: Vec<_> = reachable.into_iter().collect();
    ordered.sort();
    for id in &ordered {
        let f = &p.functions[id.0 as usize].decl;
        let sig = signature(&module, t, f)?;
        let fid = module.declare_function(&format!("tarn_fn_{}", id.0), Linkage::Local, &sig).map_err(|e| Error::bug(e.to_string()))?;
        ids.insert(*id, fid);
    }
    let mut runtime = HashMap::new();
    for (name, params, returns) in [
        ("tarn_rt_rem_f32", vec![types::F32, types::F32], vec![types::F32]),
        ("tarn_rt_rem_f64", vec![types::F64, types::F64], vec![types::F64]),
        ("tarn_rt_string", vec![types::I64, types::I64], vec![types::I64]),
        ("tarn_rt_drop_string", vec![types::I64], vec![]),
        ("tarn_rt_print_i64", vec![types::I64], vec![]),
        ("tarn_rt_print_u64", vec![types::I64], vec![]),
        ("tarn_rt_print_f64", vec![types::F64], vec![]),
        ("tarn_rt_print_bool", vec![types::I8], vec![]),
        ("tarn_rt_print_string", vec![types::I64], vec![]),
        ("tarn_rt_panic", vec![types::I64], vec![]),
        ("tarn_rt_fault", vec![], vec![]),
    ] {
        let mut sig = module.make_signature();
        sig.params = params.into_iter().map(cl::AbiParam::new).collect();
        sig.returns = returns.into_iter().map(cl::AbiParam::new).collect();
        let id = module.declare_function(name, Linkage::Import, &sig).map_err(|e| Error::bug(e.to_string()))?;
        runtime.insert(name, id);
    }
    for id in ordered {
        let f = &p.functions[id.0 as usize];
        let mut ctx = module.make_context();
        ctx.func.signature = signature(&module, t, &f.decl)?;
        let mut fb = FunctionBuilderContext::new();
        {
            let b = FunctionBuilder::new(&mut ctx.func, &mut fb);
            let mut cx = Cx { b, module: &mut module, p, t, f, ids: &ids, runtime: &runtime, locals: Vec::new(), flags: Vec::new(), blocks: Vec::new(), sret: None };
            cx.function()?;
            cx.b.finalize();
        }
        cranelift_codegen::verify_function(&ctx.func, module.isa()).map_err(|e| Error::bug(format!("{}: {e}", f.decl.name)))?;
        module.define_function(ids[&id], &mut ctx).map_err(|e| Error::bug(format!("{}: {e}", f.decl.name)))?;
    }
    // libc startup calls the C main shim; internal Tarn main is a void function.
    let mut sig = module.make_signature();
    sig.returns.push(cl::AbiParam::new(types::I32));
    let entry = module.declare_function("main", Linkage::Export, &sig).map_err(|e| Error::bug(e.to_string()))?;
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let mut fb = FunctionBuilderContext::new();
    {
        let mut b = FunctionBuilder::new(&mut ctx.func, &mut fb);
        let block = b.create_block();
        b.switch_to_block(block);
        let target = module.declare_func_in_func(ids[&main.decl.id], b.func);
        b.ins().call(target, &[]);
        let zero = b.ins().iconst(types::I32, 0);
        b.ins().return_(&[zero]);
        b.seal_all_blocks();
        b.finalize();
    }
    module.define_function(entry, &mut ctx).map_err(|e| Error::bug(e.to_string()))?;
    module.finish().emit().map_err(|e| Error::bug(e.to_string()))
}
#[derive(Clone, Copy)]
enum Slot {
    Ssa(Variable),
    Stack(cl::StackSlot),
    Empty,
}
#[derive(Clone, Copy)]
enum Flag {
    Bit(Variable),
    Bits(cl::StackSlot, u64),
}
#[derive(Clone)]
struct Val {
    value: Option<cl::Value>,
    ty: Ty,
}
struct Cx<'a, 'b> {
    b: FunctionBuilder<'a>,
    module: &'b mut ObjectModule,
    p: &'b post::Program,
    t: &'b Typed,
    f: &'b post::Function,
    ids: &'b HashMap<FunctionId, FuncId>,
    runtime: &'b HashMap<&'static str, FuncId>,
    locals: Vec<Slot>,
    flags: Vec<Flag>,
    blocks: Vec<cl::Block>,
    sret: Option<cl::Value>,
}
impl Cx<'_, '_> {
    fn function(&mut self) -> Result<()> {
        let mut addressed = HashSet::new();
        for block in &self.f.blocks {
            for s in &block.stmts {
                if let post::Op::Plain(StatementKind::Assign(_, Rvalue::Ref(_, p))) = &s.op {
                    if !matches!(p.proj.first(), Some(Proj::Deref)) {
                        addressed.insert(p.local);
                    }
                }
            }
        }
        for (i, l) in self.f.decl.locals.iter().enumerate() {
            let lyt = layout::layout(self.t, &l.ty)?;
            let slot = if lyt.size == 0 {
                Slot::Empty
            } else if let Some(ty) = scalar(&l.ty).filter(|_| !addressed.contains(&LocalId(i as u32))) {
                Slot::Ssa(self.b.declare_var(ty))
            } else {
                Slot::Stack(self.stack(lyt.size, lyt.align))
            };
            self.locals.push(slot);
        }
        for flag in &self.f.flags {
            let slot = match flag {
                post::FlagKind::Value(_) => Flag::Bit(self.b.declare_var(types::I8)),
                post::FlagKind::Elements(_, n) => Flag::Bits(self.stack(u32::try_from(*n).map_err(|_| Error::unsupported("bitmap too large"))?.max(1), 1), *n),
            };
            self.flags.push(slot);
        }
        self.blocks = self.f.blocks.iter().map(|_| self.b.create_block()).collect();
        let entry = self.blocks[0];
        self.b.append_block_params_for_function_params(entry);
        self.b.switch_to_block(entry);
        let params = self.b.block_params(entry).to_vec();
        let mut pi = 0;
        if layout::layout(self.t, &self.f.decl.ret)?.size > 0 && scalar(&self.f.decl.ret).is_none() {
            self.sret = Some(params[pi]);
            pi += 1;
        }
        for local in self.f.decl.params() {
            let ty = self.f.decl.local(local).ty.clone();
            if abi_type(self.t, &ty)?.is_some() {
                self.write(&Place::local(local), Val { value: Some(params[pi]), ty })?;
                pi += 1;
            }
        }
        // Non-SSA uninitialized locals have no reads on valid source paths.
        // Give SSA slots an arbitrary zero entry definition so a dead/uninit
        // edge through a guarded drop does not require an undefined SSA use.
        for i in 0..self.locals.len() {
            if i > 0 && i <= self.f.decl.param_count as usize {
                continue;
            }
            if let Slot::Ssa(var) = self.locals[i] {
                let ty = scalar(&self.f.decl.locals[i].ty).unwrap();
                let v = self.zero(ty);
                self.b.def_var(var, v);
            }
        }
        for (bi, block) in self.f.blocks.iter().enumerate() {
            if bi != 0 {
                self.b.switch_to_block(self.blocks[bi]);
            }
            for stmt in &block.stmts {
                self.statement(&stmt.op)?;
            }
            self.term(&block.term)?;
        }
        self.b.seal_all_blocks();
        // The outer caller consumes/finalizes the builder after lowering.
        Ok(())
    }
    fn stack(&mut self, size: u32, alignment: u32) -> cl::StackSlot {
        self.b.create_sized_stack_slot(cl::StackSlotData::new(cl::StackSlotKind::ExplicitSlot, size.max(1), alignment.trailing_zeros() as u8))
    }
    fn zero(&mut self, ty: cl::Type) -> cl::Value {
        if ty == types::F32 {
            self.b.ins().f32const(0.0)
        } else if ty == types::F64 {
            self.b.ins().f64const(0.0)
        } else {
            self.b.ins().iconst(ty, 0)
        }
    }
    fn runtime(&mut self, name: &'static str, args: &[cl::Value]) -> Vec<cl::Value> {
        let target = self.module.declare_func_in_func(self.runtime[name], self.b.func);
        let call = self.b.ins().call(target, args);
        self.b.inst_results(call).to_vec()
    }
    fn fault_if(&mut self, condition: cl::Value) {
        let fail = self.b.create_block();
        let next = self.b.create_block();
        self.b.ins().brif(condition, fail, &[], next, &[]);
        self.b.switch_to_block(fail);
        self.runtime("tarn_rt_fault", &[]);
        self.b.ins().trap(cl::TrapCode::user(1).unwrap());
        self.b.switch_to_block(next);
    }
    fn place_ty(&self, p: &Place) -> Result<Ty> {
        post::place_ty(&self.f.decl, self.t, p).ok_or_else(|| Error::bug(format!("invalid place {p:?}")))
    }
    fn addr(&mut self, p: &Place) -> Result<cl::Value> {
        let mut ty = self.f.decl.locals.get(p.local.0 as usize).ok_or_else(|| Error::bug("missing local"))?.ty.clone();
        let mut addr = if matches!(p.proj.first(), Some(Proj::Deref)) {
            self.read(&Place::local(p.local))?.value.unwrap()
        } else {
            match self.locals[p.local.0 as usize] {
                Slot::Stack(s) => self.b.ins().stack_addr(types::I64, s, 0),
                Slot::Empty => {
                    let s = self.stack(1, 1);
                    self.b.ins().stack_addr(types::I64, s, 0)
                }
                Slot::Ssa(_) => return Err(Error::bug("address-taken SSA local")),
            }
        };
        let mut payload = None;
        for (i, pr) in p.proj.iter().enumerate() {
            match pr {
                Proj::Deref => {
                    let Ty::Ref(_, inner) = ty else {
                        return Err(Error::bug("deref non-reference"));
                    };
                    if i != 0 {
                        addr = self.b.ins().load(types::I64, cl::MemFlags::new(), addr, 0);
                    }
                    ty = *inner;
                }
                Proj::Downcast(v) => {
                    payload = Some(layout::layout(self.t, &ty)?.variants.get(*v as usize).ok_or_else(|| Error::bug("invalid variant"))?.clone());
                }
                Proj::Field(i) => {
                    let fields = match payload.take() {
                        Some(xs) => xs,
                        None => layout::layout(self.t, &ty)?.fields,
                    };
                    let (offset, field) = fields.get(*i as usize).ok_or_else(|| Error::bug("invalid field"))?;
                    addr = self.b.ins().iadd_imm(addr, i64::from(*offset));
                    ty = field.clone();
                }
                Proj::Index(idx) => {
                    let Ty::Array(elem, n) = ty else {
                        return Err(Error::unsupported("slice indexing"));
                    };
                    let index = self.read(&Place::local(*idx))?.value.unwrap();
                    let invalid = self.b.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, index, n as i64);
                    self.fault_if(invalid);
                    let offset = self.b.ins().imul_imm(index, i64::from(layout::layout(self.t, &elem)?.size));
                    addr = self.b.ins().iadd(addr, offset);
                    ty = *elem;
                }
            }
        }
        Ok(addr)
    }
    fn read(&mut self, p: &Place) -> Result<Val> {
        let ty = self.place_ty(p)?;
        let l = layout::layout(self.t, &ty)?;
        if l.size == 0 {
            return Ok(Val { value: None, ty });
        }
        if p.proj.is_empty()
            && let Slot::Ssa(var) = self.locals[p.local.0 as usize]
        {
            return Ok(Val { value: Some(self.b.use_var(var)), ty });
        }
        let addr = self.addr(p)?;
        let value = if let Some(st) = scalar(&ty) { self.b.ins().load(st, cl::MemFlags::new(), addr, 0) } else { addr };
        Ok(Val { value: Some(value), ty })
    }
    fn write(&mut self, p: &Place, v: Val) -> Result<()> {
        let ty = self.place_ty(p)?;
        if ty != v.ty {
            return Err(Error::bug(format!("assignment ABI mismatch {ty:?} <- {:?}", v.ty)));
        }
        let l = layout::layout(self.t, &ty)?;
        if l.size == 0 {
            return Ok(());
        }
        let value = v.value.ok_or_else(|| Error::bug("missing nonvoid value"))?;
        if p.proj.is_empty()
            && let Slot::Ssa(var) = self.locals[p.local.0 as usize]
        {
            self.b.def_var(var, value);
            return Ok(());
        }
        let addr = self.addr(p)?;
        if scalar(&ty).is_some() {
            self.b.ins().store(cl::MemFlags::new(), value, addr, 0);
        } else {
            self.copy(addr, value, l.size);
        }
        Ok(())
    }
    fn copy(&mut self, dest: cl::Value, source: cl::Value, size: u32) {
        // Small internal copy, no dependency on libc memcpy signature. Load all
        // bytes before writing so overlapping aggregate assignments are safe.
        let values: Vec<_> = (0..size).map(|i| self.b.ins().load(types::I8, cl::MemFlags::new(), source, i as i32)).collect();
        for (i, v) in values.into_iter().enumerate() {
            self.b.ins().store(cl::MemFlags::new(), v, dest, i as i32);
        }
    }
    fn operand(&mut self, o: &Operand) -> Result<Val> {
        match o {
            Operand::Copy(p) | Operand::Move(p) => self.read(p),
            Operand::Const(c) => Ok(match c {
                Const::Int(n, i) => Val { value: Some(self.b.ins().iconst(scalar(&Ty::Int(*i)).unwrap(), *n as i64)), ty: Ty::Int(*i) },
                Const::Bool(v) => Val { value: Some(self.b.ins().iconst(types::I8, i64::from(*v))), ty: Ty::Bool },
                Const::Float(s, f) => {
                    let n: f64 = s.replace('_', "").parse().map_err(|_| Error::bug("invalid float constant"))?;
                    Val { value: Some(if *f == FloatTy::F32 { self.b.ins().f32const(n as f32) } else { self.b.ins().f64const(n) }), ty: Ty::Float(*f) }
                }
                Const::Unit => Val { value: None, ty: Ty::Void },
                Const::Str(s) => {
                    let id = self.module.declare_anonymous_data(false, false).map_err(|e| Error::bug(e.to_string()))?;
                    let mut data = DataDescription::new();
                    data.define(s.as_bytes().to_vec().into_boxed_slice());
                    self.module.define_data(id, &data).map_err(|e| Error::bug(e.to_string()))?;
                    let gv = self.module.declare_data_in_func(id, self.b.func);
                    let ptr = self.b.ins().global_value(types::I64, gv);
                    let len = self.b.ins().iconst(types::I64, s.len() as i64);
                    Val { value: Some(self.runtime("tarn_rt_string", &[ptr, len])[0]), ty: Ty::Str }
                }
                _ => return Err(Error::unsupported(format!("constant {c:?}"))),
            }),
        }
    }
    fn statement(&mut self, op: &post::Op) -> Result<()> {
        match op {
            post::Op::Plain(StatementKind::Assign(p, rv)) => {
                let ty = self.place_ty(p)?;
                let value = self.rvalue(rv, &ty)?;
                self.write(p, value)?;
            }
            post::Op::Plain(StatementKind::StorageLive(_) | StatementKind::StorageDead(_)) => {}
            post::Op::Plain(StatementKind::Drop(_)) => return Err(Error::bug("abstract drop")),
            post::Op::Set(id, value) => {
                let v = self.b.ins().iconst(types::I8, i64::from(*value));
                match self.flags[id.0 as usize] {
                    Flag::Bit(var) => self.b.def_var(var, v),
                    Flag::Bits(s, n) => {
                        for i in 0..n {
                            self.b.ins().stack_store(v, s, i as i32);
                        }
                    }
                }
            }
            post::Op::ClearElement(id, idx) => {
                let Flag::Bits(s, n) = self.flags[id.0 as usize] else {
                    return Err(Error::bug("element flag not bitmap"));
                };
                let i = self.read(&Place::local(*idx))?.value.unwrap();
                let bad = self.b.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, i, n as i64);
                self.fault_if(bad);
                let ptr = self.b.ins().stack_addr(types::I64, s, 0);
                let ptr = self.b.ins().iadd(ptr, i);
                let zero = self.b.ins().iconst(types::I8, 0);
                self.b.ins().store(cl::MemFlags::new(), zero, ptr, 0);
            }
            post::Op::Destroy(d) => self.drop_plan(d)?,
        }
        Ok(())
    }
    fn rvalue(&mut self, rv: &Rvalue, dest: &Ty) -> Result<Val> {
        match rv {
            Rvalue::Use(o) => self.operand(o),
            Rvalue::Ref(m, p) => Ok(Val { value: Some(self.addr(p)?), ty: Ty::Ref(*m, Box::new(self.place_ty(p)?)) }),
            Rvalue::Binary(op, a, b) => {
                let a = self.operand(a)?;
                let b = self.operand(b)?;
                self.binary(*op, a, b)
            }
            Rvalue::Unary(op, o) => {
                let v = self.operand(o)?;
                let a = v.value.unwrap();
                let value = match (op, &v.ty) {
                    (UnOp::Not, Ty::Bool) => self.b.ins().icmp_imm(IntCC::Equal, a, 0),
                    (UnOp::Not, Ty::Int(_)) => self.b.ins().bnot(a),
                    (UnOp::Neg, Ty::Float(_)) => self.b.ins().fneg(a),
                    (UnOp::Neg, Ty::Int(i)) if i.signed() => {
                        let min = -(1i128 << (int_bits(*i) - 1));
                        let bad = self.b.ins().icmp_imm(IntCC::Equal, a, min as i64);
                        self.fault_if(bad);
                        self.b.ins().ineg(a)
                    }
                    _ => return Err(Error::unsupported("unary operation")),
                };
                Ok(Val { value: Some(value), ty: v.ty })
            }
            Rvalue::Aggregate(kind, ops) => {
                let l = layout::layout(self.t, dest)?;
                let s = self.stack(l.size, l.align);
                let addr = self.b.ins().stack_addr(types::I64, s, 0);
                let fields = match kind {
                    Aggregate::Struct(..) => l.fields,
                    Aggregate::Variant(_, v, _) => {
                        let tag = self.b.ins().iconst(types::I32, i64::from(*v));
                        self.b.ins().store(cl::MemFlags::new(), tag, addr, 0);
                        l.variants.get(*v as usize).ok_or_else(|| Error::bug("aggregate variant"))?.clone()
                    }
                    Aggregate::Array(elem) => {
                        let size = layout::layout(self.t, elem)?.size;
                        ops.iter().enumerate().map(|(i, _)| (i as u32 * size, elem.clone())).collect()
                    }
                    _ => return Err(Error::unsupported("closure aggregate")),
                };
                if fields.len() != ops.len() {
                    return Err(Error::bug("aggregate arity"));
                }
                // Evaluate all operands before storing destination bytes.
                let values = ops.iter().map(|o| self.operand(o)).collect::<Result<Vec<_>>>()?;
                for ((offset, ty), v) in fields.into_iter().zip(values) {
                    if ty != v.ty {
                        return Err(Error::bug("aggregate field type"));
                    }
                    let size = layout::layout(self.t, &ty)?.size;
                    if size == 0 {
                        continue;
                    }
                    let ptr = self.b.ins().iadd_imm(addr, i64::from(offset));
                    if scalar(&ty).is_some() {
                        self.b.ins().store(cl::MemFlags::new(), v.value.unwrap(), ptr, 0);
                    } else {
                        self.copy(ptr, v.value.unwrap(), size);
                    }
                }
                Ok(Val { value: if l.size == 0 { None } else { Some(addr) }, ty: dest.clone() })
            }
            Rvalue::Discriminant(p) => {
                let ptr = self.addr(p)?;
                let v = self.b.ins().load(types::I32, cl::MemFlags::new(), ptr, 0);
                let st = scalar(dest).ok_or_else(|| Error::bug("tag destination"))?;
                let v = if st == types::I32 { v } else { self.b.ins().uextend(st, v) };
                Ok(Val { value: Some(v), ty: dest.clone() })
            }
            Rvalue::Len(p) => {
                let Ty::Array(_, n) = self.place_ty(p)? else {
                    return Err(Error::unsupported("slice length"));
                };
                Ok(Val { value: Some(self.b.ins().iconst(types::I64, n as i64)), ty: Ty::Int(IntTy::Usize) })
            }
            Rvalue::Cast(o, ty) => {
                let v = self.operand(o)?;
                self.cast(v, ty)
            }
            Rvalue::Coerce(CoerceKind::MutToShared, o, ty) => {
                let v = self.operand(o)?;
                Ok(Val { value: v.value, ty: ty.clone() })
            }
            _ => Err(Error::unsupported(format!("rvalue {rv:?}"))),
        }
    }
    fn binary(&mut self, op: BinOp, a: Val, b: Val) -> Result<Val> {
        if a.ty != b.ty {
            return Err(Error::bug("binary operand type mismatch"));
        }
        let (x, y) = (a.value.unwrap(), b.value.unwrap());
        let cmp = matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge);
        let value = match &a.ty {
            Ty::Float(_) => match op {
                BinOp::Add => self.b.ins().fadd(x, y),
                BinOp::Sub => self.b.ins().fsub(x, y),
                BinOp::Mul => self.b.ins().fmul(x, y),
                BinOp::Div => self.b.ins().fdiv(x, y),
                BinOp::Rem => self.runtime(if a.ty == Ty::Float(FloatTy::F32) { "tarn_rt_rem_f32" } else { "tarn_rt_rem_f64" }, &[x, y])[0],
                _ if cmp => self.b.ins().fcmp(
                    match op {
                        BinOp::Eq => FloatCC::Equal,
                        BinOp::Ne => FloatCC::NotEqual,
                        BinOp::Lt => FloatCC::LessThan,
                        BinOp::Le => FloatCC::LessThanOrEqual,
                        BinOp::Gt => FloatCC::GreaterThan,
                        _ => FloatCC::GreaterThanOrEqual,
                    },
                    x,
                    y,
                ),
                _ => return Err(Error::unsupported("floating-point remainder/bit operation")),
            },
            Ty::Int(_) | Ty::Bool => {
                let signed = matches!(a.ty,Ty::Int(i) if i.signed());
                if cmp {
                    let cc = match (op, signed) {
                        (BinOp::Eq, _) => IntCC::Equal,
                        (BinOp::Ne, _) => IntCC::NotEqual,
                        (BinOp::Lt, true) => IntCC::SignedLessThan,
                        (BinOp::Le, true) => IntCC::SignedLessThanOrEqual,
                        (BinOp::Gt, true) => IntCC::SignedGreaterThan,
                        (BinOp::Ge, true) => IntCC::SignedGreaterThanOrEqual,
                        (BinOp::Lt, false) => IntCC::UnsignedLessThan,
                        (BinOp::Le, false) => IntCC::UnsignedLessThanOrEqual,
                        (BinOp::Gt, false) => IntCC::UnsignedGreaterThan,
                        _ => IntCC::UnsignedGreaterThanOrEqual,
                    };
                    self.b.ins().icmp(cc, x, y)
                } else {
                    match op {
                        BinOp::Add | BinOp::Sub | BinOp::Mul => {
                            let (v, overflow) = match (op, signed) {
                                (BinOp::Add, true) => self.b.ins().sadd_overflow(x, y),
                                (BinOp::Add, false) => self.b.ins().uadd_overflow(x, y),
                                (BinOp::Sub, true) => self.b.ins().ssub_overflow(x, y),
                                (BinOp::Sub, false) => self.b.ins().usub_overflow(x, y),
                                (BinOp::Mul, true) => self.b.ins().smul_overflow(x, y),
                                _ => self.b.ins().umul_overflow(x, y),
                            };
                            self.fault_if(overflow);
                            v
                        }
                        BinOp::Div | BinOp::Rem => {
                            let zero = self.b.ins().icmp_imm(IntCC::Equal, y, 0);
                            self.fault_if(zero);
                            if signed {
                                let Ty::Int(i) = a.ty else {
                                    return Err(Error::bug("signed bool"));
                                };
                                let min = -(1i128 << (int_bits(i) - 1));
                                let lhs = self.b.ins().icmp_imm(IntCC::Equal, x, min as i64);
                                let rhs = self.b.ins().icmp_imm(IntCC::Equal, y, -1);
                                let bad = self.b.ins().band(lhs, rhs);
                                self.fault_if(bad);
                            }
                            match (op, signed) {
                                (BinOp::Div, true) => self.b.ins().sdiv(x, y),
                                (BinOp::Div, false) => self.b.ins().udiv(x, y),
                                (BinOp::Rem, true) => self.b.ins().srem(x, y),
                                _ => self.b.ins().urem(x, y),
                            }
                        }
                        BinOp::BitAnd => self.b.ins().band(x, y),
                        BinOp::BitOr => self.b.ins().bor(x, y),
                        BinOp::BitXor => self.b.ins().bxor(x, y),
                        BinOp::Shl | BinOp::Shr => return Err(Error::unsupported("shift semantics pending native validation")),
                        _ => return Err(Error::unsupported("integer operation")),
                    }
                }
            }
            _ => return Err(Error::unsupported("binary nonnumeric value")),
        };
        Ok(Val { value: Some(value), ty: if cmp { Ty::Bool } else { a.ty } })
    }
    fn cast(&mut self, v: Val, ty: &Ty) -> Result<Val> {
        let value = v.value.ok_or_else(|| Error::bug("cast void"))?;
        let target = scalar(ty).ok_or_else(|| Error::unsupported("aggregate cast"))?;
        let out = match (&v.ty, ty) {
            (Ty::Int(src), Ty::Int(dst)) => {
                let (lo, hi) = dst.range();
                let (slo, shi) = src.range();
                let st = scalar(&v.ty).unwrap();
                if slo < lo {
                    let bad = self.b.ins().icmp_imm(if src.signed() { IntCC::SignedLessThan } else { IntCC::UnsignedLessThan }, value, lo as i64);
                    self.fault_if(bad);
                }
                if shi > hi {
                    let bad = self.b.ins().icmp_imm(if src.signed() { IntCC::SignedGreaterThan } else { IntCC::UnsignedGreaterThan }, value, hi as i64);
                    self.fault_if(bad);
                }
                if target.bits() < st.bits() {
                    self.b.ins().ireduce(target, value)
                } else if target.bits() > st.bits() {
                    if src.signed() { self.b.ins().sextend(target, value) } else { self.b.ins().uextend(target, value) }
                } else {
                    value
                }
            }
            (Ty::Int(i), Ty::Float(_)) => {
                if i.signed() {
                    self.b.ins().fcvt_from_sint(target, value)
                } else {
                    self.b.ins().fcvt_from_uint(target, value)
                }
            }
            (Ty::Float(src), Ty::Float(dst)) => {
                if src == dst {
                    value
                } else if *dst == FloatTy::F64 {
                    self.b.ins().fpromote(target, value)
                } else {
                    self.b.ins().fdemote(target, value)
                }
            }
            _ => return Err(Error::unsupported("float-to-integer or nonnumeric checked cast")),
        };
        Ok(Val { value: Some(out), ty: ty.clone() })
    }
    fn term(&mut self, term: &Terminator) -> Result<()> {
        match term {
            Terminator::Goto(next) => {
                self.b.ins().jump(self.blocks[next.0 as usize], &[]);
            }
            Terminator::Switch { discr, cases, otherwise } => {
                let v = self.operand(discr)?.value.ok_or_else(|| Error::bug("void switch"))?;
                // Comparison chain preserves signed constants and arbitrary
                // sparse cases without changing IR control flow semantics.
                for (n, target) in cases {
                    let next = self.b.create_block();
                    let eq = self.b.ins().icmp_imm(IntCC::Equal, v, *n as i64);
                    self.b.ins().brif(eq, self.blocks[target.0 as usize], &[], next, &[]);
                    self.b.switch_to_block(next);
                }
                self.b.ins().jump(self.blocks[otherwise.0 as usize], &[]);
            }
            Terminator::Return => {
                let ret = self.read(&Place::local(RETURN))?;
                if let Some(sret) = self.sret {
                    if let Some(ptr) = ret.value {
                        self.copy(sret, ptr, layout::layout(self.t, &ret.ty)?.size);
                    }
                    self.b.ins().return_(&[]);
                } else {
                    self.b.ins().return_(&ret.value.into_iter().collect::<Vec<_>>());
                }
            }
            Terminator::Unreachable => {
                self.b.ins().trap(cl::TrapCode::user(2).unwrap());
            }
            Terminator::Call { callee, args, dest, next, spawn, .. } => {
                if *spawn {
                    return Err(Error::unsupported("spawn"));
                }
                let args = args.iter().map(|o| self.operand(o)).collect::<Result<Vec<_>>>()?;
                let dest_ty = self.place_ty(dest)?;
                let value = match callee {
                    Callee::Fn(id, type_args) => {
                        if !type_args.is_empty() {
                            return Err(Error::unsupported("generic function call"));
                        }
                        let f = &self.p.functions.get(id.0 as usize).ok_or_else(|| Error::bug("missing callee"))?.decl;
                        if args.len() != f.param_count as usize || dest_ty != f.ret {
                            return Err(Error::bug("direct call/return ABI mismatch"));
                        }
                        for (arg, p) in args.iter().zip(f.params()) {
                            if arg.ty != f.local(p).ty {
                                return Err(Error::bug("direct call parameter ABI mismatch"));
                            }
                        }
                        let mut values = Vec::new();
                        let aggregate = layout::layout(self.t, &f.ret)?.size > 0 && scalar(&f.ret).is_none();
                        let ptr = if aggregate {
                            let ptr = self.addr(dest)?;
                            values.push(ptr);
                            Some(ptr)
                        } else {
                            None
                        };
                        values.extend(args.iter().filter_map(|v| v.value));
                        let id = *self.ids.get(id).ok_or_else(|| Error::unsupported("external/intrinsic function body"))?;
                        let target = self.module.declare_func_in_func(id, self.b.func);
                        let call = self.b.ins().call(target, &values);
                        if aggregate { Val { value: ptr, ty: dest_ty.clone() } } else { Val { value: self.b.inst_results(call).first().copied(), ty: dest_ty.clone() } }
                    }
                    Callee::Builtin(Builtin::Print) => {
                        if args.len() != 1 {
                            return Err(Error::bug("print arity"));
                        }
                        // Copy primitives are passed by value; owned values
                        // are observed through the frontend's explicit borrow.
                        let (inner, v) = if let Ty::Ref(_, inner) = &args[0].ty {
                            let ty = scalar(inner).ok_or_else(|| Error::unsupported("printing aggregate"))?;
                            (inner.as_ref(), self.b.ins().load(ty, cl::MemFlags::new(), args[0].value.unwrap(), 0))
                        } else {
                            (&args[0].ty, args[0].value.ok_or_else(|| Error::unsupported("print void"))?)
                        };
                        let ty = scalar(inner).ok_or_else(|| Error::unsupported("printing aggregate"))?;
                        let (name, v) = match inner {
                            Ty::Int(i) => {
                                let v = if ty.bits() < 64 { if i.signed() { self.b.ins().sextend(types::I64, v) } else { self.b.ins().uextend(types::I64, v) } } else { v };
                                (if i.signed() { "tarn_rt_print_i64" } else { "tarn_rt_print_u64" }, v)
                            }
                            Ty::Bool => ("tarn_rt_print_bool", v),
                            Ty::Str => ("tarn_rt_print_string", v),
                            Ty::Float(f) => {
                                let v = if *f == FloatTy::F32 { self.b.ins().fpromote(types::F64, v) } else { v };
                                ("tarn_rt_print_f64", v)
                            }
                            _ => return Err(Error::unsupported("print type")),
                        };
                        self.runtime(name, &[v]);
                        Val { value: None, ty: Ty::Void }
                    }
                    Callee::Builtin(Builtin::Panic) => {
                        if args.len() != 1 {
                            return Err(Error::bug("panic arity"));
                        }
                        let v = match &args[0].ty {
                            Ty::Str => args[0].value.unwrap(),
                            Ty::Ref(_, x) if **x == Ty::Str => self.b.ins().load(types::I64, cl::MemFlags::new(), args[0].value.unwrap(), 0),
                            _ => return Err(Error::unsupported("panic nonstring")),
                        };
                        self.runtime("tarn_rt_panic", &[v]);
                        self.b.ins().trap(cl::TrapCode::user(1).unwrap());
                        return Ok(());
                    }
                    Callee::Intrinsic(name) => self.intrinsic(name, &args, &dest_ty)?,
                    _ => return Err(Error::unsupported(format!("callee {callee:?}"))),
                };
                self.write(dest, value)?;
                if let Some(next) = next {
                    self.b.ins().jump(self.blocks[next.0 as usize], &[]);
                } else {
                    self.b.ins().trap(cl::TrapCode::user(2).unwrap());
                }
            }
        }
        Ok(())
    }
    fn intrinsic(&mut self, name: &str, args: &[Val], dest: &Ty) -> Result<Val> {
        if args.len() != 1 {
            return Err(Error::unsupported(format!("intrinsic {name}")));
        }
        let a = args[0].value.unwrap();
        let value = match (name, &args[0].ty) {
            ("f32.sqrt" | "f64.sqrt", Ty::Float(_)) => self.b.ins().sqrt(a),
            ("f32.abs" | "f64.abs", Ty::Float(_)) => self.b.ins().fabs(a),
            (_, Ty::Int(i)) if name == format!("{}.abs", i.name()) && i.signed() => {
                let min = -(1i128 << (int_bits(*i) - 1));
                let bad = self.b.ins().icmp_imm(IntCC::Equal, a, min as i64);
                self.fault_if(bad);
                let negative = self.b.ins().icmp_imm(IntCC::SignedLessThan, a, 0);
                let neg = self.b.ins().ineg(a);
                self.b.ins().select(negative, neg, a)
            }
            ("string.len" | "string.is_empty" | "string.clone", Ty::Ref(_, inner)) if **inner == Ty::Str => {
                let string = self.b.ins().load(types::I64, cl::MemFlags::new(), a, 0);
                let len = self.b.ins().load(types::I64, cl::MemFlags::new(), string, 0);
                match name {
                    "string.len" => len,
                    "string.is_empty" => self.b.ins().icmp_imm(IntCC::Equal, len, 0),
                    _ => {
                        let bytes = self.b.ins().iadd_imm(string, 8);
                        self.runtime("tarn_rt_string", &[bytes, len])[0]
                    }
                }
            }
            _ => return Err(Error::unsupported(format!("intrinsic {name}"))),
        };
        Ok(Val { value: Some(value), ty: dest.clone() })
    }
    fn drop_plan(&mut self, d: &post::Drop) -> Result<()> {
        match d {
            post::Drop::Value(p) => self.drop_value(p)?,
            post::Drop::Guard(id, d) => {
                let Flag::Bit(v) = self.flags[id.0 as usize] else {
                    return Err(Error::bug("guard bitmap"));
                };
                let c = self.b.use_var(v);
                let yes = self.b.create_block();
                let next = self.b.create_block();
                self.b.ins().brif(c, yes, &[], next, &[]);
                self.b.switch_to_block(yes);
                self.drop_plan(d)?;
                self.b.ins().jump(next, &[]);
                self.b.switch_to_block(next);
            }
            post::Drop::Fields { fields, .. } => {
                for (_, d) in fields {
                    self.drop_plan(d)?;
                }
            }
            post::Drop::Variants { place, variants } => {
                let addr = self.addr(place)?;
                let tag = self.b.ins().load(types::I32, cl::MemFlags::new(), addr, 0);
                let next = self.b.create_block();
                for (v, fields) in variants.iter().enumerate() {
                    let yes = self.b.create_block();
                    let no = self.b.create_block();
                    let c = self.b.ins().icmp_imm(IntCC::Equal, tag, v as i64);
                    self.b.ins().brif(c, yes, &[], no, &[]);
                    self.b.switch_to_block(yes);
                    for (_, d) in fields {
                        self.drop_plan(d)?;
                    }
                    self.b.ins().jump(next, &[]);
                    self.b.switch_to_block(no);
                }
                self.runtime("tarn_rt_fault", &[]);
                self.b.ins().trap(cl::TrapCode::user(2).unwrap());
                self.b.switch_to_block(next);
            }
            post::Drop::Remaining { place, flag } => {
                let Flag::Bits(slot, n) = self.flags[flag.0 as usize] else {
                    return Err(Error::bug("remaining without bitmap"));
                };
                for i in 0..n {
                    let c = self.b.ins().stack_load(types::I8, slot, i as i32);
                    let yes = self.b.create_block();
                    let next = self.b.create_block();
                    self.b.ins().brif(c, yes, &[], next, &[]);
                    self.b.switch_to_block(yes);
                    self.drop_array_element(place, i)?;
                    self.b.ins().jump(next, &[]);
                    self.b.switch_to_block(next);
                }
            }
        }
        Ok(())
    }
    fn drop_array_element(&mut self, place: &Place, i: u64) -> Result<()> {
        let Ty::Array(elem, _) = self.place_ty(place)? else {
            return Err(Error::bug("drop array type"));
        };
        let addr = self.addr(place)?;
        let offset = i.checked_mul(u64::from(layout::layout(self.t, &elem)?.size)).ok_or_else(|| Error::bug("drop offset overflow"))?;
        let addr = self.b.ins().iadd_imm(addr, offset as i64);
        self.drop_at(addr, &elem)
    }
    fn drop_value(&mut self, p: &Place) -> Result<()> {
        let ty = self.place_ty(p)?;
        if matches!(ty, Ty::Str) {
            let ptr = self.read(p)?.value.unwrap();
            self.runtime("tarn_rt_drop_string", &[ptr]);
            return Ok(());
        }
        if matches!(ty, Ty::Ref(..) | Ty::Fn(..)) || self.t.decls.is_copy(&ty) {
            return Ok(());
        }
        let addr = self.addr(p)?;
        self.drop_at(addr, &ty)
    }
    fn drop_at(&mut self, addr: cl::Value, ty: &Ty) -> Result<()> {
        if self.t.decls.is_copy(ty) || matches!(ty, Ty::Ref(..)) {
            return Ok(());
        }
        match ty {
            Ty::Str => {
                let ptr = self.b.ins().load(types::I64, cl::MemFlags::new(), addr, 0);
                self.runtime("tarn_rt_drop_string", &[ptr]);
            }
            Ty::Array(elem, n) => {
                let size = layout::layout(self.t, elem)?.size;
                for i in 0..*n {
                    let ptr = self.b.ins().iadd_imm(addr, (i * u64::from(size)) as i64);
                    self.drop_at(ptr, elem)?;
                }
            }
            Ty::Adt(..) => {
                let l = layout::layout(self.t, ty)?;
                if l.variants.is_empty() {
                    for (offset, ty) in l.fields {
                        let ptr = self.b.ins().iadd_imm(addr, i64::from(offset));
                        self.drop_at(ptr, &ty)?;
                    }
                } else {
                    let tag = self.b.ins().load(types::I32, cl::MemFlags::new(), addr, 0);
                    let next = self.b.create_block();
                    for (v, fields) in l.variants.iter().enumerate() {
                        let yes = self.b.create_block();
                        let no = self.b.create_block();
                        let c = self.b.ins().icmp_imm(IntCC::Equal, tag, v as i64);
                        self.b.ins().brif(c, yes, &[], no, &[]);
                        self.b.switch_to_block(yes);
                        for (offset, ty) in fields {
                            let ptr = self.b.ins().iadd_imm(addr, i64::from(*offset));
                            self.drop_at(ptr, ty)?;
                        }
                        self.b.ins().jump(next, &[]);
                        self.b.switch_to_block(no);
                    }
                    self.runtime("tarn_rt_fault", &[]);
                    self.b.ins().trap(cl::TrapCode::user(2).unwrap());
                    self.b.switch_to_block(next);
                }
            }
            _ => return Err(Error::unsupported(format!("drop glue {ty:?}"))),
        }
        Ok(())
    }
}
