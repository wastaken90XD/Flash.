# R6 — Garbage Collection, Lifetimes, and `WStr` (`05-gc-memory.md`)

- **Pinned Release Tag:** `v0.7.1`
- **Pinned Commit SHA:** `89a7049965f3090e706265670df309a5128233ac`
- **Repository:** `https://github.com/ruffle-rs/ruffle`
- **Overall Confidence:** **High** (verified directly from `core/src/player.rs`, `core/common/src/avm_string/`, and `wstr/src/` at `89a7049965f3090e706265670df309a5128233ac`).

---

## 1. `gc-arena` Root, Pointers, Write Barriers, and Collection Pacing (`Confidence: High`)

- **Arena Root (`GcArena` and `GcRoot<'gc>`, `core/src/player.rs:87-93, 170-232, 302, 350`):**
  - `type GcArena = gc_arena::Arena<Rootable![GcRoot<'_>]>;` (`core/src/player.rs:302`), stored in `Player.gc_arena: Rc<RefCell<GcArena>>` (`core/src/player.rs:350`).
  - `GcRoot<'gc>` holds `avm2_callstack: GcRefLock<'gc, CallStack<'gc>>` and `data: GcRefLock<'gc, GcRootData<'gc>>` (`core/src/player.rs:89-92`).
  - `GcRootData<'gc>` (`core/src/player.rs:171-232`) owns all GC-traced state: `library`, `stage`, `mouse_data`, `drag_object`, `avm1`, `avm2`, `action_queue`, `interner`, `load_manager`, `avm1_shared_objects`, `avm2_shared_objects`, `unbound_text_fields`, `timers`, `audio_manager`, `stream_manager`, `orphan_manager`, `dynamic_root`, and `post_frame_callbacks`.
- **GC Pointer Types and Write Barriers:**
  - `Gc<'gc, T>`: Immutable GC-managed pointer (`Copy`, 1 word).
  - `GcRefLock<'gc, T>` (`gc_arena::lock::RefLock<T>` wrapped in `Gc`): Interior mutability with write barriers (`borrow_mut(mc: &Mutation<'gc>)` automatically triggers `gc_arena`'s backward write barrier so mutated black objects are re-grayed).
  - Field-level write barriers use `gc_arena::barrier::unlock!(Gc::write(mc, self.0), Type, field)` (`core/src/display_object/container.rs:116-120`) to mutate `Lock<T>` fields without runtime `RefCell` borrow flags.
  - `GcWeak<'gc, T>`: Weak GC pointer used for parent/mask back-pointers and weak string interning (`core/common/src/avm_string/interner.rs:128-134`).
- **Collection Pacing (`collect_debt`, `core/src/player.rs:2442-2443`):**
  - At the end of every `Player::update(...)` call (which wraps `run_frame`, input events, timers, and async callbacks), Ruffle calls `self.gc_arena.borrow_mut().collect_debt()` (`core/src/player.rs:2443`), performing incremental tri-color mark-and-sweep work proportional to allocations performed during that update.

---

## 2. `WString` / `WStr` (UCS-2 / UTF-16 vs Latin-1) (`Confidence: High`)

- **Dual-Width Representation (`wstr/src/lib.rs:1-69`, `wstr/src/ptr.rs:1-130`):**
  - `WStr` (borrowed slice DST) and `WString` (owned buffer) store strings as either:
    - `Units::Bytes(&[u8])` (`is_wide = false`) when all code units are `<= 0xFF` (ASCII / Latin-1), saving 50% memory, or
    - `Units::Wide(&[u16])` (`is_wide = true`) when any code unit is `> 0xFF` (UCS-2 / potentially unpaired UTF-16 surrogates, matching Flash Player's UTF-16 `String.length` and `charAt`/`charCodeAt` semantics).
  - `WStrMetadata(u32)` (`wstr/src/ptr.rs:42-129`) packs the 31-bit unit length (`<= 0x7FFF_FFFF`) and `WIDE_MASK = 0x8000_0000` into a custom fat-pointer metadata word.
  - **Stacked Borrows Caveat (`wstr/src/ptr.rs:18-31`):** `WStr` casts `&[u8]` / `&[u16]` into `*const [()]` with modified slice length metadata, which `wstr/src/ptr.rs:20-25` explicitly notes is technically unsound under Miri / Stacked Borrows and relies on `unsafe` pointer arithmetic throughout `wstr/src/ptr.rs`.

---

## 3. String Interning (`StringContext`, `AvmAtom`, `AvmString`) (`Confidence: High`)

- **`AvmStringRepr<'gc>` (`core/common/src/avm_string/repr.rs:12-140`):**
  - Supports three string ownership modes inside the GC arena:
    1. **Owned** (`owner: None`, `capacity > 0`): Heap-allocated `WString` buffer; supports **in-place append** (`try_append_inline`, `core/common/src/avm_string/repr.rs:77-140`) when concatenating onto the end of a string with spare capacity!
    2. **Dependent** (`owner: Some(Gc<'gc, Self>)`, `capacity == 0`): Zero-copy substring (`AvmString::substring`, `core/common/src/avm_string/repr.rs:53-60`) pointing directly into `owner`'s buffer!
    3. **Static** (`owner: None`, `capacity == 0`): Zero-copy pointer to a compile-time `'static WStr`.
- **`AvmAtom<'gc>` and `AvmStringInterner<'gc>` (`core/common/src/avm_string/interner.rs:15-140`):**
  - `AvmAtom<'gc>` wraps an interned `Gc<'gc, AvmStringRepr<'gc>>` and implements `PartialEq` and `Hash` via **O(1) pointer equality** (`Gc::ptr_eq`, `core/common/src/avm_string/interner.rs:20-32`).
  - `AvmStringInterner` uses a `WeakSet<'gc, AvmStringRepr<'gc>>` (`HashTable<GcWeak<'gc, T>>` + `FnvBuildHasher`) that automatically prunes dead interned strings during GC tracing (`core/common/src/avm_string/interner.rs:122-134`), plus pre-interned `CommonStrings` (all 128 single-character ASCII strings and built-in identifier names, `core/common/src/avm_string/interner.rs:109-116`).

---

## 4. Comparison with `swfr-avm1` (`BTreeMap<u32, ObjectFields>` + `String`) (`Confidence: High`)

- **How `swfr-avm1` works (`crates/swfr-avm1/src/lib.rs`):**
  - Stores objects in a central table `objects: BTreeMap<u32, ObjectFields>` indexed by `ObjectId(u32)` handles, and represents strings as standard safe Rust `String` (`UTF-8`).
- **Does `swfr-avm1` leak cyclic objects?**
  - **Yes, until a mark-and-sweep pass is run over `objects`:** Because `swfr-avm1` allocates monotonic `u32` IDs into `self.objects: BTreeMap<u32, ObjectFields>` without garbage collection, unreachable objects (both cyclic and non-cyclic temporary objects created per frame in `onEnterFrame`) remain in `self.objects` for the lifetime of the movie.
  - However, because `swfr-avm1` already stores all objects in a flat arena (`BTreeMap<u32, ObjectFields>`), adding a **safe mark-and-sweep collector** over `self.objects` (rooted at `globals`, `scope_stack`, `value_stack`, `registers`, `clip_props`, and `timers`) takes ~80 lines of 100% safe Rust — requiring zero `gc-arena` lifetimes (`'gc`) or `unsafe` code!

---

## 5. Verification Against Regression Tests (`Confidence: High`)

1. **Claim 1 — UTF-16 code unit length (`"😋".length == 2`) and unpaired surrogates (`wstr/src/lib.rs:1-8`):**
   - Verified against `tests/tests/swfs/avm1/string_methods/output.txt:1-24` (`"😋".length` -> `2`, `"😋Спасибо𝌆𝌆hello".length` -> `18`, `s.charAt(3)` -> ``).
2. **Claim 2 — SWF6 `length()` vs `mblength()` and `ord()` vs `mbord()` (`core/src/avm1/activation.rs`):**
   - Verified against `tests/tests/swfs/avm1/string_ops_swf6/output.txt:1-25` (`length("😋")` -> `2`, `mblength("😋")` -> `2`).
3. **Claim 3 — Custom `toString()` coercion and fallback `[type Object]` (`core/src/avm1/value.rs`):**
   - Verified against `tests/tests/swfs/avm1/string_coercion/output.txt:1-25`.

---

## 6. What Could Not Be Verified (`Confidence: High`)

- Memory usage and GC pause durations of `gc-arena` under heavy allocation workloads were not benchmarked at runtime because `cargo` is not installed in the sandbox.

---

## 7. Open Questions

1. Should `swfr-avm1` add a periodic safe mark-and-sweep pass over `self.objects: BTreeMap<u32, ObjectFields>` (e.g., at the end of `Player::run_frame` when `self.objects.len()` exceeds a threshold) so long-running games that allocate temporary objects in `onEnterFrame` do not grow `self.objects` unboundedly?

---

## 8. Test Ideas for `swfr`

1. **Per-frame temporary object GC test:** Run 1,000 frames of an `onEnterFrame` handler that creates cyclic temporary objects (`var a = {}; var b = {a: a}; a.b = b;`) and assert live object count in `swfr-avm1` stays bounded after collection.
2. **UTF-16 surrogate pair `String.length` / `charCodeAt` test:** Verify whether `swfr-avm1` string index operations count UTF-16 code units (`encode_utf16().count()`) for non-BMP characters (`tests/tests/swfs/avm1/string_methods/output.txt:7-10`).
3. **`String.charAt` 32-bit integer wrap test:** Verify `s.charAt(4294967297)` wraps `4294967297` to `1` via `coerce_to_i32` (`tests/tests/swfs/avm1/string_methods/output.txt:23-24`).
