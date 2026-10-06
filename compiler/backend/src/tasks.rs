use super::*;

// Phase 14A cooperative task records (ADR 0038). Scheduling, completion and
// abandonment are decided in Tarn; ownership by the frontend. This module only
// moves typed result bytes in and out of runtime records and destroys an
// unjoined result with the element type known after monomorphization.
impl Cx<'_, '_> {
    fn task_record(&mut self, reference: &Val) -> Result<cl::Value> {
        let addr = reference.value.ok_or_else(|| Error::bug("task record storage"))?;
        Ok(self.b.ins().load(types::I64, cl::MemFlags::new(), addr, 0))
    }

    /// Build `Option<T>`: `ready` selects Some, whose payload is copied from `from`.
    fn task_option(&mut self, dest: &Ty, ready: cl::Value, fill: impl FnOnce(&mut Self, cl::Value) -> Result<()>) -> Result<Val> {
        let Ty::Adt(option, args) = dest else { return Err(Error::bug("task option result")) };
        let def = self.t.decls.enums.get(option).ok_or_else(|| Error::bug("option layout"))?;
        let none = def.variants.iter().position(|v| v.name == "None").ok_or_else(|| Error::bug("Option.None"))?;
        let some = def.variants.iter().position(|v| v.name == "Some").ok_or_else(|| Error::bug("Option.Some"))?;
        let l = layout::layout(self.t, dest)?;
        let payload = l.variants.get(some).and_then(|v| v.first()).cloned().ok_or_else(|| Error::bug("Option payload"))?;
        if args.first() != Some(&payload.1) {
            return Err(Error::bug("task option payload"));
        }
        let slot = self.stack(l.size, l.align);
        let out = self.b.ins().stack_addr(types::I64, slot, 0);
        let (on_none, on_some, done) = (self.b.create_block(), self.b.create_block(), self.b.create_block());
        self.b.ins().brif(ready, on_some, &[], on_none, &[]);
        self.b.switch_to_block(on_none);
        let tag = self.b.ins().iconst(types::I32, none as i64);
        self.b.ins().store(cl::MemFlags::new(), tag, out, 0);
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(on_some);
        let target = self.b.ins().iadd_imm(out, i64::from(payload.0));
        fill(self, target)?;
        let tag = self.b.ins().iconst(types::I32, some as i64);
        self.b.ins().store(cl::MemFlags::new(), tag, out, 0);
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
        Ok(Val { value: Some(out), ty: dest.clone() })
    }

    fn store_bytes(&mut self, at: cl::Value, value: &Val) -> Result<()> {
        let size = layout::layout(self.t, &value.ty)?.size;
        if size == 0 {
            return Ok(());
        }
        let v = value.value.ok_or_else(|| Error::bug("missing task value"))?;
        if scalar(&value.ty).is_some() {
            self.b.ins().store(cl::MemFlags::new(), v, at, 0);
        } else {
            self.copy(at, v, size);
        }
        Ok(())
    }

    pub(super) fn tasks(&mut self, name: &str, args: &[Val], dest: &Ty) -> Result<Option<Val>> {
        let Some(operation) = name.strip_prefix("runtime._") else { return Ok(None) };
        if !tarn_types::TASK_INTRINSICS.contains(&name.trim_start_matches("runtime.")) {
            return Ok(None);
        }
        let value = match (operation, args) {
            ("task_new", [owner]) => {
                let Ty::Adt(task, ts) = dest else { return Err(Error::bug("task handle type")) };
                if Some(*task) != self.t.decls.async_task || ts.len() != 1 {
                    return Err(Error::bug("task handle identity"));
                }
                let native = self.task_record(owner)?;
                let size = self.b.ins().iconst(types::I64, i64::from(layout::layout(self.t, &ts[0])?.size));
                let record = self.runtime("tarn_rt_async_task_new", &[native, size])[0];
                let l = layout::layout(self.t, dest)?;
                let slot = self.stack(l.size, l.align);
                let out = self.b.ins().stack_addr(types::I64, slot, 0);
                self.b.ins().store(cl::MemFlags::new(), record, out, 0);
                Val { value: Some(out), ty: dest.clone() }
            }
            ("task_complete", [task, value]) => {
                let record = self.task_record(task)?;
                let at = self.runtime("tarn_rt_async_task_result", &[record])[0];
                self.store_bytes(at, value)?;
                self.runtime("tarn_rt_async_task_complete", &[record]);
                Val { value: None, ty: Ty::Void }
            }
            ("task_take", [task]) => {
                let Ty::Ref(false, handle) = &task.ty else { return Err(Error::bug("task take receiver")) };
                let Ty::Adt(_, ts) = handle.as_ref() else { return Err(Error::bug("task take handle")) };
                let elem = ts.first().cloned().ok_or_else(|| Error::bug("task result type"))?;
                let record = self.task_record(task)?;
                let ready = self.runtime("tarn_rt_async_task_take", &[record])[0];
                let size = layout::layout(self.t, &elem)?.size;
                self.task_option(dest, ready, |cx, target| {
                    let from = cx.runtime("tarn_rt_async_task_result", &[record])[0];
                    cx.copy(target, from, size);
                    Ok(())
                })?
            }
            ("task_wait", [record, waker]) => {
                let l = layout::layout(self.t, dest)?;
                let slot = self.stack(l.size, l.align);
                let out = self.b.ins().stack_addr(types::I64, slot, 0);
                let record = record.value.ok_or_else(|| Error::bug("task record"))?;
                let waker = self.task_record(waker)?;
                self.runtime("tarn_rt_async_task_wait", &[out, record, waker]);
                Val { value: Some(out), ty: dest.clone() }
            }
            ("task_abandoned", [record]) => {
                let record = record.value.ok_or_else(|| Error::bug("task record"))?;
                let flag = self.runtime("tarn_rt_async_task_abandoned", &[record])[0];
                Val { value: Some(flag), ty: Ty::Bool }
            }
            ("inbox_push", [owner, spawned]) => {
                let native = self.task_record(owner)?;
                let bytes = spawned.value.ok_or_else(|| Error::bug("spawned operation"))?;
                let size = self.b.ins().iconst(types::I64, i64::from(layout::layout(self.t, &spawned.ty)?.size));
                self.runtime("tarn_rt_async_inbox_push", &[native, bytes, size]);
                Val { value: None, ty: Ty::Void }
            }
            ("inbox_pop", [owner]) => {
                let Ty::Adt(_, ts) = dest else { return Err(Error::bug("inbox result")) };
                let elem = ts.first().cloned().ok_or_else(|| Error::bug("inbox element"))?;
                let native = self.task_record(owner)?;
                let size = i64::from(layout::layout(self.t, &elem)?.size);
                // Pop into a scratch slot first, then build the Option.
                let l = layout::layout(self.t, &elem)?;
                let scratch = self.stack(l.size, l.align);
                let scratch = self.b.ins().stack_addr(types::I64, scratch, 0);
                let width = self.b.ins().iconst(types::I64, size);
                let ready = self.runtime("tarn_rt_async_inbox_pop", &[native, scratch, width])[0];
                self.task_option(dest, ready, |cx, target| {
                    cx.copy(target, scratch, size as u32);
                    Ok(())
                })?
            }
            _ => return Err(Error::bug(format!("task intrinsic ABI: {operation}"))),
        };
        Ok(Some(value))
    }

    /// AsyncTask<R> destruction: destroy an unjoined result once, or mark a
    /// pending task abandoned; then release the handle's reference.
    pub(super) fn drop_async_task(&mut self, addr: cl::Value, ty: &Ty) -> Result<()> {
        let Ty::Adt(_, ts) = ty else { return Err(Error::bug("task handle drop")) };
        let elem = ts.first().cloned().ok_or_else(|| Error::bug("task result type"))?;
        let record = self.b.ins().load(types::I64, cl::MemFlags::new(), addr, 0);
        let owned = self.runtime("tarn_rt_async_task_handle_drop", &[record])[0];
        let (destroy, release) = (self.b.create_block(), self.b.create_block());
        self.b.ins().brif(owned, destroy, &[], release, &[]);
        self.b.switch_to_block(destroy);
        let at = self.runtime("tarn_rt_async_task_result", &[record])[0];
        self.drop_at(at, &elem)?;
        self.b.ins().jump(release, &[]);
        self.b.switch_to_block(release);
        self.runtime("tarn_rt_async_task_release", &[record]);
        Ok(())
    }

    /// The runner's record reference: released when its frame is destroyed.
    pub(super) fn drop_task_ref(&mut self, addr: cl::Value) {
        let record = self.b.ins().load(types::I64, cl::MemFlags::new(), addr, 0);
        self.runtime("tarn_rt_async_task_release", &[record]);
    }
}
