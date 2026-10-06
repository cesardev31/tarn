# Core and hardcoded behavior audit (phase 9)

Canonical public signatures are in `stdlib/core/core.tarn`; standard string wrappers
are in `stdlib/string/string.tarn`. `FnSig.contract` derives passing/result facts,
and the borrow checker uses declared sources or inferred body summaries. Every
remaining name/behavior category in this implemented path is classified below.
Ordinary user field/member-name lookup is not an intrinsic or bootstrap API.

| Names / behavior | Classification | Authority and remaining work |
|---|---|---|
| bool, i8/i16/i32/i64, u8/u16/u32/u64, isize/usize, f32/f64, string, void, never | Required compiler intrinsic | Primitive type identity, numeric widths, operators, casts and source literals need compiler semantics. Physical layout is exclusively backend layout.rs. |
| Copy; `copy` struct/enum; primitive/shared-reference Copy rules | Required compiler intrinsic | Capability identity is declared in core; compiler enforces copying/resource compatibility and bounds. No user destructor semantics. |
| Option/Result; Some/None/Ok/Err; try payload selection | Required compiler intrinsic | ADTs are core declarations, selected by resolved lang-item identity. Try currently uses canonical variant spellings in declaration vectors. Changing this needs an explicit language decision. |
| print | Required compiler intrinsic / candidate for richer Tarn declarations | Polymorphic printable-type checking and borrowed owned arguments are Rust lowering rules. Native print lanes are runtime ABI details. Ordinary functions cannot currently express this polymorphic overload family. |
| panic | Required compiler intrinsic / candidate for Tarn declaration | Never-returning, abort-only lowering is compiler semantics. A monomorphic string declaration could eventually replace the prelude entry; panic effects and abort terminators still need compiler knowledge. |
| Array/slice len/is_empty and array unsizing, indexing, slice ranges | Required compiler intrinsic; declaration candidate | Owner syntax for generic built-in collections is missing. Frontend computes typed receiver adjustment; backend executes length/bounds/stride. No ownership inference occurs there. |
| string.len/is_empty/clone | Required compiler intrinsic + runtime ABI detail | Signatures are core declarations. Header access and unique allocation cloning need representation operations. The string module wraps them in Tarn. |
| string.view/choose | Tarn declarations and implementations | Moved to ordinary core bodies; result sources inferred by 6B. No native name implementation remains. |
| f32/f64 sqrt/abs; i8/i16/i32/i64/isize abs | Required compiler intrinsic | Core signatures select known typed primitive operations; signed abs faults on minimum. Runtime fmod wrappers implement float remainder. |
| Error, Channel, Sender, channel(), send/recv-style method fallback | Temporary bootstrap debt | Placeholder Error and unmodeled channel operations reject E3040. No concurrency contracts/runtime added. Channel/Sender declarations alone do not provide executable support. |
| Standard module names (collections, fs, path, process, io, time, json, http, net, tls, testing, logging, crypto, encoding, compression, cli, os) | Temporary bootstrap debt / declaration candidates | Resolver recognizes import names. Unknown members/types reject E3040. String has checked embedded declarations; local modules remain source-checked. |
| core automatic loading / public prelude export; extern "intrinsic" restricted to core | Required compiler intrinsic | Compiler embeds canonical declarations and reserves trusted primitive implementation markers. User modules cannot mint intrinsic implementations. |
| main; private tarn_fn_N/tarn_thunk_N/tarn_vtable_N | Runtime ABI detail | Native entry shim and deterministic private object symbols. These are not user API contracts. |
| tarn_rt_string/drop_string/print_i64/print_u64/print_f64/print_bool/print_string/panic/fault/rem_f32/rem_f64; string len header; TARN_TRACE_DROPS | Runtime ABI detail | C runtime and native code must honor ownership, layout and abort promises. Trace is opt-in test observation. No user destructor hook. |
| DynTable, declaration-index methods, fat pairs, flags and IterArray bitmaps | Required compiler representation / private ABI detail | Explicit lowered operations and canonical layout. They introduce no public magic API name or new semantic inference. |
| Callable invocation and capture ownership (phase 10 update) | Required semantic rule; ADR 0032 | Shared/mutable calls borrow; consuming calls move. Inferred captures are shared/mutable borrows or ownership moves. Generated ordinary destruction functions pass through post-drop. Borrowed environments carry explicit storage loans. |
| tarn_rt_env_alloc/env_free/env_drop; environment destruction header | Private runtime ABI detail; ADR 0032 | Unique owned allocations, destruction-thunk dispatch and free. Borrowed/capture-free closures allocate no heap environment. No managed-object system, scheduler or user destructors. |

An audit is not a proof that trusted native implementations are correct. Unsupported
stdlib APIs, FFI, resource handles, concurrency, pointer semantics and trusted native implementation contracts still prevent a complete end-to-end safety claim.
