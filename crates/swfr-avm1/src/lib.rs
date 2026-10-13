//! ActionScript 1 & 2 (AVM1) virtual machine, generational arena heap, incremental
//! mark-and-sweep collector, decoded `Vec<Op>` interpreter, built-in classes, and
//! deterministic timers for `swfr` (TSD 1.6, 2.9, 2.12 M4).
#![allow(
    clippy::arithmetic_side_effects,
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::indexing_slicing,
    clippy::map_entry
)]

use std::collections::{BTreeMap, BTreeSet};
use swfr_reader::Reader;
use swfr_scene::Stage;

/// Generational object handle into [`Heap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjId {
    /// Arena slot index.
    pub index: u32,
    /// Slot generation counter.
    pub generation: u32,
}

/// AVM1 runtime value (ECMA-262 3rd edition + Flash `MovieClip` reference).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `undefined` primitive.
    Undefined,
    /// `null` primitive.
    Null,
    /// Boolean primitive (`true` or `false`).
    Bool(bool),
    /// IEEE-754 double-precision number.
    Number(f64),
    /// UTF-8 string primitive.
    String(String),
    /// Heap-allocated object, array, or function closure.
    Object(ObjId),
    /// Target path reference to a timeline / `MovieClip` (e.g. `"_root"`).
    MovieClip(String),
}

impl Value {
    /// Returns the ECMA-262 `typeof` string for this value.
    #[must_use]
    pub fn typeof_str(&self, heap: &Heap) -> &'static str {
        match self {
            Self::Undefined => "undefined",
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::MovieClip(_) => "movieclip",
            Self::Object(id) => {
                if heap.is_function(*id) {
                    "function"
                } else {
                    "object"
                }
            }
        }
    }

    /// Converts this value to a boolean following SWF version rules (SWF <= 6 vs SWF >= 7).
    #[must_use]
    pub fn to_bool(&self, swf_version: u8) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0.0 && !n.is_nan(),
            Self::String(s) => {
                if swf_version <= 6 {
                    s.parse::<f64>().is_ok_and(|n| n != 0.0 && !n.is_nan())
                } else {
                    !s.is_empty()
                }
            }
            Self::Object(_) | Self::MovieClip(_) => true,
        }
    }

    /// Converts this value to an `f64` following ECMA-262 3rd edition & SWF version rules.
    #[must_use]
    pub fn to_number(&self, swf_version: u8) -> f64 {
        match self {
            Self::Undefined => {
                if swf_version <= 6 {
                    0.0
                } else {
                    f64::NAN
                }
            }
            Self::Null => {
                if swf_version <= 6 {
                    0.0
                } else {
                    0.0
                }
            }
            Self::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Number(n) => *n,
            Self::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    if swf_version <= 6 {
                        0.0
                    } else {
                        f64::NAN
                    }
                } else if let Some(hex) = trimmed
                    .strip_prefix("0x")
                    .or_else(|| trimmed.strip_prefix("0X"))
                {
                    i64::from_str_radix(hex, 16).map_or(f64::NAN, |v| v as f64)
                } else {
                    trimmed.parse::<f64>().unwrap_or(f64::NAN)
                }
            }
            Self::Object(_) | Self::MovieClip(_) => f64::NAN,
        }
    }

    /// Converts this value to a `32-bit` signed integer (`ToInt32` in ECMA-262 9.5).
    #[must_use]
    #[allow(clippy::arithmetic_side_effects)]
    pub fn to_i32(&self, swf_version: u8) -> i32 {
        let n = self.to_number(swf_version);
        if !n.is_finite() || n == 0.0 {
            return 0;
        }
        let int_part = n.trunc().rem_euclid(4_294_967_296.0);
        let u = int_part as u32;
        u.cast_signed()
    }

    /// Converts this value to a `32-bit` unsigned integer (`ToUint32` in ECMA-262 9.6).
    #[must_use]
    pub fn to_u32(&self, swf_version: u8) -> u32 {
        self.to_i32(swf_version).cast_unsigned()
    }

    /// Converts this value to its string representation (`ToString` in ECMA-262 9.8).
    #[must_use]
    pub fn to_avm_string(&self, swf_version: u8, heap: &Heap) -> String {
        match self {
            Self::Undefined => {
                if swf_version <= 6 {
                    String::new()
                } else {
                    "undefined".to_owned()
                }
            }
            Self::Null => "null".to_owned(),
            Self::Bool(b) => if *b { "true" } else { "false" }.to_owned(),
            Self::Number(n) => format_es3_number(*n),
            Self::String(s) => s.clone(),
            Self::MovieClip(p) => p.clone(),
            Self::Object(id) => {
                if let Some(elems) = heap.array_elements(*id) {
                    let parts: Vec<String> = elems
                        .iter()
                        .map(|v| v.to_avm_string(swf_version, heap))
                        .collect();
                    parts.join(",")
                } else if heap.is_function(*id) {
                    "[type Function]".to_owned()
                } else {
                    "[object Object]".to_owned()
                }
            }
        }
    }
}

/// Formats an IEEE-754 number according to ECMA-262 3rd edition `Number::toString`.
#[must_use]
pub fn format_es3_number(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_owned();
    }
    if n == 0.0 {
        return "0".to_owned();
    }
    if n.is_infinite() {
        return if n.is_sign_positive() {
            "Infinity".to_owned()
        } else {
            "-Infinity".to_owned()
        };
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return format!("{:.0}", n);
    }
    format!("{n}")
}

/// User-defined ActionScript function stored on a heap object.
#[derive(Debug, Clone, PartialEq)]
pub struct UserFunction {
    /// Function name (may be empty for anonymous functions).
    pub name: String,
    /// Parameter names.
    pub params: Vec<String>,
    /// Pre-decoded instructions for the function body.
    pub ops: Vec<Op>,
    /// Captured scope variables at definition time.
    pub captured_scope: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
struct HeapObject {
    proto: Option<ObjId>,
    props: BTreeMap<String, Value>,
    array: Option<Vec<Value>>,
    function: Option<UserFunction>,
    marked: bool,
}

#[derive(Debug, Clone)]
struct Slot {
    generation: u32,
    obj: Option<HeapObject>,
}

/// Generational arena heap with incremental mark-and-sweep garbage collection (TSD 2.9, M4 Task 1).
#[derive(Debug, Clone, Default)]
pub struct Heap {
    slots: Vec<Slot>,
    free_list: Vec<u32>,
    allocations_since_gc: usize,
}

impl Heap {
    /// Creates an empty [`Heap`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            allocations_since_gc: 0,
        }
    }

    /// Allocates a plain object with optional prototype `proto`.
    pub fn alloc_object(&mut self, proto: Option<ObjId>) -> ObjId {
        self.alloc_raw(HeapObject {
            proto,
            props: BTreeMap::new(),
            array: None,
            function: None,
            marked: false,
        })
    }

    /// Allocates an `Array` object initialized with `elements`.
    pub fn alloc_array(&mut self, elements: Vec<Value>) -> ObjId {
        let mut props = BTreeMap::new();
        props.insert("length".to_owned(), Value::Number(elements.len() as f64));
        self.alloc_raw(HeapObject {
            proto: None,
            props,
            array: Some(elements),
            function: None,
            marked: false,
        })
    }

    /// Allocates a function object wrapping `func`.
    pub fn alloc_function(&mut self, func: UserFunction) -> ObjId {
        let proto_id = self.alloc_object(None);
        let mut props = BTreeMap::new();
        props.insert("prototype".to_owned(), Value::Object(proto_id));
        self.alloc_raw(HeapObject {
            proto: None,
            props,
            array: None,
            function: Some(func),
            marked: false,
        })
    }

    fn alloc_raw(&mut self, obj: HeapObject) -> ObjId {
        self.allocations_since_gc = self.allocations_since_gc.saturating_add(1);
        if let Some(idx) = self.free_list.pop() {
            if let Some(slot) = self.slots.get_mut(idx as usize) {
                slot.obj = Some(obj);
                return ObjId {
                    index: idx,
                    generation: slot.generation,
                };
            }
        }
        let idx = u32::try_from(self.slots.len()).unwrap_or(0);
        self.slots.push(Slot {
            generation: 1,
            obj: Some(obj),
        });
        ObjId {
            index: idx,
            generation: 1,
        }
    }

    fn get_obj(&self, id: ObjId) -> Option<&HeapObject> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation == id.generation {
            slot.obj.as_ref()
        } else {
            None
        }
    }

    fn get_obj_mut(&mut self, id: ObjId) -> Option<&mut HeapObject> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation == id.generation {
            slot.obj.as_mut()
        } else {
            None
        }
    }

    /// Returns `true` if `id` refers to a live function object.
    #[must_use]
    pub fn is_function(&self, id: ObjId) -> bool {
        self.get_obj(id).is_some_and(|o| o.function.is_some())
    }

    /// Returns the [`UserFunction`] if `id` is a function object.
    #[must_use]
    pub fn get_function(&self, id: ObjId) -> Option<&UserFunction> {
        self.get_obj(id).and_then(|o| o.function.as_ref())
    }

    /// Returns the array elements slice if `id` is an `Array` object.
    #[must_use]
    pub fn array_elements(&self, id: ObjId) -> Option<&[Value]> {
        self.get_obj(id).and_then(|o| o.array.as_deref())
    }

    /// Mutates the array elements of `id` and updates its `length` property.
    pub fn with_array_mut<F, R>(&mut self, id: ObjId, f: F) -> Option<R>
    where
        F: FnOnce(&mut Vec<Value>) -> R,
    {
        let obj = self.get_obj_mut(id)?;
        let arr = obj.array.as_mut()?;
        let res = f(arr);
        let new_len = arr.len() as f64;
        obj.props
            .insert("length".to_owned(), Value::Number(new_len));
        Some(res)
    }

    /// Reads property `key` from `id`, walking the prototype chain (`__proto__` / `proto`) up to 16 hops.
    #[must_use]
    pub fn get_prop(&self, id: ObjId, key: &str, swf_version: u8) -> Value {
        let norm_key = normalize_key(key, swf_version);
        let mut cur = Some(id);
        for _ in 0..16 {
            let Some(cid) = cur else {
                break;
            };
            let Some(obj) = self.get_obj(cid) else {
                break;
            };
            if let Some(ref arr) = obj.array {
                if norm_key == "length" {
                    return Value::Number(arr.len() as f64);
                }
                if let Ok(idx) = norm_key.parse::<usize>() {
                    return arr.get(idx).cloned().unwrap_or(Value::Undefined);
                }
            }
            if let Some(val) = obj.props.get(&norm_key) {
                return val.clone();
            }
            if norm_key == "__proto__" {
                if let Some(p) = obj.proto {
                    return Value::Object(p);
                }
            }
            cur = obj.proto;
        }
        Value::Undefined
    }

    /// Sets property `key` on `id`.
    pub fn set_prop(&mut self, id: ObjId, key: &str, val: Value, swf_version: u8) {
        let norm_key = normalize_key(key, swf_version);
        let Some(obj) = self.get_obj_mut(id) else {
            return;
        };
        if norm_key == "__proto__" {
            if let Value::Object(pid) = val {
                obj.proto = Some(pid);
            }
            return;
        }
        if let Some(ref mut arr) = obj.array {
            if norm_key == "length" {
                let new_len = val.to_u32(swf_version) as usize;
                arr.resize(new_len.min(65536), Value::Undefined);
                obj.props
                    .insert("length".to_owned(), Value::Number(arr.len() as f64));
                return;
            }
            if let Ok(idx) = norm_key.parse::<usize>() {
                if idx < 65536 {
                    if idx >= arr.len() {
                        arr.resize(idx.saturating_add(1), Value::Undefined);
                    }
                    if let Some(slot) = arr.get_mut(idx) {
                        *slot = val;
                    }
                    obj.props
                        .insert("length".to_owned(), Value::Number(arr.len() as f64));
                    return;
                }
            }
        }
        obj.props.insert(norm_key, val);
    }

    /// Deletes own property `key` from `id`.
    pub fn delete_prop(&mut self, id: ObjId, key: &str, swf_version: u8) -> bool {
        let norm_key = normalize_key(key, swf_version);
        self.get_obj_mut(id)
            .is_some_and(|o| o.props.remove(&norm_key).is_some())
    }

    /// Returns own enumerable property keys of `id` for `ActionEnumerate` / `ActionEnumerate2`.
    #[must_use]
    pub fn own_keys(&self, id: ObjId) -> Vec<String> {
        let Some(obj) = self.get_obj(id) else {
            return Vec::new();
        };
        let mut keys: Vec<String> = obj
            .props
            .keys()
            .filter(|k| *k != "prototype" && *k != "__proto__" && *k != "length")
            .cloned()
            .collect();
        if let Some(ref arr) = obj.array {
            for i in 0..arr.len() {
                keys.push(i.to_string());
            }
        }
        keys
    }

    /// Runs mark-and-sweep garbage collection starting from `roots`.
    pub fn collect_garbage(&mut self, roots: &[Value]) -> usize {
        for slot in &mut self.slots {
            if let Some(ref mut obj) = slot.obj {
                obj.marked = false;
            }
        }
        let mut worklist = Vec::new();
        for val in roots {
            if let Value::Object(id) = val {
                worklist.push(*id);
            }
        }
        while let Some(id) = worklist.pop() {
            let Some(obj) = self.get_obj_mut(id) else {
                continue;
            };
            if obj.marked {
                continue;
            }
            obj.marked = true;
            if let Some(p) = obj.proto {
                worklist.push(p);
            }
            for v in obj.props.values() {
                if let Value::Object(cid) = v {
                    worklist.push(*cid);
                }
            }
            if let Some(ref arr) = obj.array {
                for v in arr {
                    if let Value::Object(cid) = v {
                        worklist.push(*cid);
                    }
                }
            }
            if let Some(ref func) = obj.function {
                for v in func.captured_scope.values() {
                    if let Value::Object(cid) = v {
                        worklist.push(*cid);
                    }
                }
            }
        }

        let mut freed = 0_usize;
        for (idx, slot) in self.slots.iter_mut().enumerate() {
            let should_free = slot.obj.as_ref().is_some_and(|o| !o.marked);
            if should_free {
                slot.obj = None;
                slot.generation = slot.generation.wrapping_add(1).max(1);
                if let Ok(u) = u32::try_from(idx) {
                    self.free_list.push(u);
                }
                freed = freed.saturating_add(1);
            }
        }
        self.allocations_since_gc = 0;
        freed
    }
}

fn normalize_key(key: &str, swf_version: u8) -> String {
    if swf_version <= 6 {
        key.to_ascii_lowercase()
    } else {
        key.to_owned()
    }
}

/// Pre-decoded AVM1 instruction with branch targets resolved to instruction indices (TSD 2.9).
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// End of action block (`0x00`).
    End,
    /// `ActionNextFrame` (`0x04`).
    NextFrame,
    /// `ActionPrevFrame` (`0x05`).
    PrevFrame,
    /// `ActionPlay` (`0x06`).
    Play,
    /// `ActionStop` (`0x07`).
    Stop,
    /// `ActionStopSounds` (`0x09`).
    StopSounds,
    /// `ActionAdd` (`0x0A`, SWF4 numeric addition).
    AddSwf4,
    /// `ActionSubtract` (`0x0B`).
    Subtract,
    /// `ActionMultiply` (`0x0C`).
    Multiply,
    /// `ActionDivide` (`0x0D`).
    Divide,
    /// `ActionEquals` (`0x0E`, SWF4 numeric equality).
    EqualsSwf4,
    /// `ActionLess` (`0x0F`, SWF4 numeric `<`).
    LessSwf4,
    /// `ActionAnd` (`0x10`, logical AND).
    And,
    /// `ActionOr` (`0x11`, logical OR).
    Or,
    /// `ActionNot` (`0x12`, logical NOT).
    Not,
    /// `ActionStringEquals` (`0x13`).
    StringEquals,
    /// `ActionStringLength` (`0x14`).
    StringLength,
    /// `ActionStringExtract` (`0x15`).
    StringExtract,
    /// `ActionPop` (`0x17`).
    Pop,
    /// `ActionToInteger` (`0x18`).
    ToInteger,
    /// `ActionGetVariable` (`0x1C`).
    GetVariable,
    /// `ActionSetVariable` (`0x1D`).
    SetVariable,
    /// `ActionStringAdd` (`0x21`).
    StringAdd,
    /// `ActionGetProperty` (`0x22`).
    GetProperty,
    /// `ActionSetProperty` (`0x23`).
    SetProperty,
    /// `ActionTrace` (`0x26`).
    Trace,
    /// `ActionRandomNumber` (`0x30`).
    RandomNumber,
    /// `ActionCallFunction` (`0x3D`).
    CallFunction,
    /// `ActionReturn` (`0x3E`).
    Return,
    /// `ActionModulo` (`0x3F`).
    Modulo,
    /// `ActionNewObject` (`0x40`).
    NewObject,
    /// `ActionDefineLocal` (`0x3C`).
    DefineLocal,
    /// `ActionDefineLocal2` (`0x41`).
    DefineLocal2,
    /// `ActionInitArray` (`0x42`).
    InitArray,
    /// `ActionInitObject` (`0x43`).
    InitObject,
    /// `ActionTypeOf` (`0x44`).
    TypeOf,
    /// `ActionEnumerate` (`0x46`) / `ActionEnumerate2` (`0x55`).
    Enumerate,
    /// `ActionAdd2` (`0x47`, ECMA-262 `+`).
    Add2,
    /// `ActionLess2` (`0x48`, ECMA-262 `<`).
    Less2,
    /// `ActionEquals2` (`0x49`, ECMA-262 `==`).
    Equals2,
    /// `ActionToNumber` (`0x4A`).
    ToNumber,
    /// `ActionToString` (`0x4B`).
    ToStringOp,
    /// `ActionPushDuplicate` (`0x4C`).
    PushDuplicate,
    /// `ActionStackSwap` (`0x4D`).
    StackSwap,
    /// `ActionGetMember` (`0x4E`).
    GetMember,
    /// `ActionSetMember` (`0x4F`).
    SetMember,
    /// `ActionIncrement` (`0x50`).
    Increment,
    /// `ActionDecrement` (`0x51`).
    Decrement,
    /// `ActionCallMethod` (`0x52`).
    CallMethod,
    /// `ActionDelete` (`0x3A`) / `ActionDelete2` (`0x3B`).
    Delete,
    /// `ActionBitAnd` (`0x60`).
    BitAnd,
    /// `ActionBitOr` (`0x61`).
    BitOr,
    /// `ActionBitXor` (`0x62`).
    BitXor,
    /// `ActionBitLShift` (`0x63`).
    BitLShift,
    /// `ActionBitRShift` (`0x64`).
    BitRShift,
    /// `ActionBitURShift` (`0x65`).
    BitURShift,
    /// `ActionStrictEquals` (`0x66`).
    StrictEquals,
    /// `ActionGreater` (`0x67`).
    Greater,
    /// `ActionStringGreater` (`0x68`).
    StringGreater,
    /// `ActionGotoFrame` (`0x81`).
    GotoFrame(u16),
    /// `ActionConstantPool` (`0x88`).
    ConstantPool(Vec<String>),
    /// `ActionGotoLabel` (`0x8C`).
    GotoLabel(String),
    /// `ActionPush` (`0x96`).
    Push(Vec<PushItem>),
    /// `ActionJump` (`0x99`, resolved target instruction index).
    Jump(usize),
    /// `ActionIf` (`0x9D`, resolved target instruction index).
    If(usize),
    /// `ActionGotoFrame2` (`0x9F`, `play_flag`).
    GotoFrame2(bool),
    /// `ActionStoreRegister` (`0x87`).
    StoreRegister(u8),
    /// `ActionDefineFunction` (`0x9B`) / `ActionDefineFunction2` (`0x8E`).
    DefineFunction {
        /// Function name (empty for anonymous).
        name: String,
        /// Parameter names.
        params: Vec<String>,
        /// Pre-decoded body operations.
        body_ops: Vec<Op>,
    },
}

/// Immediate value encoded inside an `ActionPush` instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum PushItem {
    /// Literal value (`String`, `Number`, `Bool`, `Null`, `Undefined`).
    Literal(Value),
    /// Register index (`0..=255`).
    Register(u8),
    /// Constant pool index (`0..=65535`).
    Constant(u16),
}

/// Decodes raw AVM1 bytecode into a `Vec<Op>` with byte-offset jumps resolved to instruction indices.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn decode_ops(bytecode: &[u8]) -> Vec<Op> {
    let mut raw_insts: Vec<(usize, usize, Op, Option<i16>)> = Vec::new();
    let mut r = Reader::new(bytecode);

    while !r.is_eof() {
        let start_pos = r.position();
        let Ok(opcode) = r.read_u8() else {
            break;
        };
        if opcode == 0x00 {
            let next_pos = r.position();
            raw_insts.push((start_pos, next_pos, Op::End, None));
            break;
        }
        let payload_len = if opcode >= 0x80 {
            usize::from(r.read_u16().unwrap_or(0))
        } else {
            0
        };
        let payload_bytes = r.read_bytes(payload_len).unwrap_or_default().to_vec();
        let next_pos = r.position();
        let mut pr = Reader::new(&payload_bytes);

        let mut rel_offset: Option<i16> = None;
        let op = match opcode {
            0x04 => Op::NextFrame,
            0x05 => Op::PrevFrame,
            0x06 => Op::Play,
            0x07 => Op::Stop,
            0x09 => Op::StopSounds,
            0x0A => Op::AddSwf4,
            0x0B => Op::Subtract,
            0x0C => Op::Multiply,
            0x0D => Op::Divide,
            0x0E => Op::EqualsSwf4,
            0x0F => Op::LessSwf4,
            0x10 => Op::And,
            0x11 => Op::Or,
            0x12 => Op::Not,
            0x13 => Op::StringEquals,
            0x14 | 0x31 => Op::StringLength,
            0x15 | 0x35 => Op::StringExtract,
            0x17 => Op::Pop,
            0x18 => Op::ToInteger,
            0x1C => Op::GetVariable,
            0x1D => Op::SetVariable,
            0x21 => Op::StringAdd,
            0x22 => Op::GetProperty,
            0x23 => Op::SetProperty,
            0x26 => Op::Trace,
            0x30 => Op::RandomNumber,
            0x3A | 0x3B => Op::Delete,
            0x3C => Op::DefineLocal,
            0x3D => Op::CallFunction,
            0x3E => Op::Return,
            0x3F => Op::Modulo,
            0x40 => Op::NewObject,
            0x41 => Op::DefineLocal2,
            0x42 => Op::InitArray,
            0x43 => Op::InitObject,
            0x44 => Op::TypeOf,
            0x46 | 0x55 => Op::Enumerate,
            0x47 => Op::Add2,
            0x48 => Op::Less2,
            0x49 => Op::Equals2,
            0x4A => Op::ToNumber,
            0x4B => Op::ToStringOp,
            0x4C => Op::PushDuplicate,
            0x4D => Op::StackSwap,
            0x4E => Op::GetMember,
            0x4F => Op::SetMember,
            0x50 => Op::Increment,
            0x51 => Op::Decrement,
            0x52 => Op::CallMethod,
            0x60 => Op::BitAnd,
            0x61 => Op::BitOr,
            0x62 => Op::BitXor,
            0x63 => Op::BitLShift,
            0x64 => Op::BitRShift,
            0x65 => Op::BitURShift,
            0x66 => Op::StrictEquals,
            0x67 => Op::Greater,
            0x68 => Op::StringGreater,
            0x81 => Op::GotoFrame(pr.read_u16().unwrap_or(0)),
            0x87 => Op::StoreRegister(pr.read_u8().unwrap_or(0)),
            0x88 => {
                let count = usize::from(pr.read_u16().unwrap_or(0));
                let mut pool = Vec::with_capacity(count.min(4096));
                for _ in 0..count {
                    if let Ok(s) = pr.read_c_string() {
                        pool.push(s);
                    }
                }
                Op::ConstantPool(pool)
            }
            0x8C => Op::GotoLabel(pr.read_c_string().unwrap_or_default()),
            0x96 => {
                let mut items = Vec::new();
                while !pr.is_eof() {
                    let Ok(ptype) = pr.read_u8() else {
                        break;
                    };
                    match ptype {
                        0 => {
                            let s = pr.read_c_string().unwrap_or_default();
                            items.push(PushItem::Literal(Value::String(s)));
                        }
                        1 => {
                            let bits = pr.read_u32().unwrap_or(0);
                            items.push(PushItem::Literal(Value::Number(f64::from(
                                f32::from_bits(bits),
                            ))));
                        }
                        2 => items.push(PushItem::Literal(Value::Null)),
                        3 => items.push(PushItem::Literal(Value::Undefined)),
                        4 => items.push(PushItem::Register(pr.read_u8().unwrap_or(0))),
                        5 => items.push(PushItem::Literal(Value::Bool(
                            pr.read_u8().unwrap_or(0) != 0,
                        ))),
                        6 => {
                            let hi = u64::from(pr.read_u32().unwrap_or(0));
                            let lo = u64::from(pr.read_u32().unwrap_or(0));
                            let val = f64::from_bits((hi << 32) | lo);
                            items.push(PushItem::Literal(Value::Number(val)));
                        }
                        7 => {
                            let v = pr.read_i32().unwrap_or(0);
                            items.push(PushItem::Literal(Value::Number(f64::from(v))));
                        }
                        8 => {
                            let c = u16::from(pr.read_u8().unwrap_or(0));
                            items.push(PushItem::Constant(c));
                        }
                        9 => {
                            let c = pr.read_u16().unwrap_or(0);
                            items.push(PushItem::Constant(c));
                        }
                        _ => break,
                    }
                }
                Op::Push(items)
            }
            0x99 => {
                let off = pr.read_i16().unwrap_or(0);
                rel_offset = Some(off);
                Op::Jump(0)
            }
            0x9D => {
                let off = pr.read_i16().unwrap_or(0);
                rel_offset = Some(off);
                Op::If(0)
            }
            0x9F => {
                let flags = pr.read_u8().unwrap_or(0);
                Op::GotoFrame2((flags & 0x01) != 0)
            }
            0x9B => {
                // ActionDefineFunction
                let name = pr.read_c_string().unwrap_or_default();
                let num_params = usize::from(pr.read_u16().unwrap_or(0));
                let mut params = Vec::with_capacity(num_params.min(256));
                for _ in 0..num_params {
                    if let Ok(p) = pr.read_c_string() {
                        params.push(p);
                    }
                }
                let code_size = usize::from(pr.read_u16().unwrap_or(0));
                let body_bytes = r.read_bytes(code_size).unwrap_or_default();
                let body_ops = decode_ops(body_bytes);
                let after_body = r.position();
                raw_insts.push((
                    start_pos,
                    after_body,
                    Op::DefineFunction {
                        name,
                        params,
                        body_ops,
                    },
                    None,
                ));
                continue;
            }
            0x8E => {
                // ActionDefineFunction2
                let name = pr.read_c_string().unwrap_or_default();
                let num_params = usize::from(pr.read_u16().unwrap_or(0));
                let _register_count = pr.read_u8().unwrap_or(0);
                let _flags = pr.read_u16().unwrap_or(0);
                let mut params = Vec::with_capacity(num_params.min(256));
                for _ in 0..num_params {
                    let _reg = pr.read_u8().unwrap_or(0);
                    if let Ok(p) = pr.read_c_string() {
                        params.push(p);
                    }
                }
                let code_size = usize::from(pr.read_u16().unwrap_or(0));
                let body_bytes = r.read_bytes(code_size).unwrap_or_default();
                let body_ops = decode_ops(body_bytes);
                let after_body = r.position();
                raw_insts.push((
                    start_pos,
                    after_body,
                    Op::DefineFunction {
                        name,
                        params,
                        body_ops,
                    },
                    None,
                ));
                continue;
            }
            _ => continue,
        };
        raw_insts.push((start_pos, next_pos, op, rel_offset));
    }

    let inst_count = raw_insts.len();
    let mut ops = Vec::with_capacity(inst_count);
    for i in 0..inst_count {
        let (_, next_byte_pos, ref op, rel_opt) = raw_insts[i];
        let resolved = match (op, rel_opt) {
            (Op::Jump(_), Some(off)) => {
                let target_byte =
                    (next_byte_pos as isize).saturating_add(off as isize).max(0) as usize;
                let target_idx = raw_insts
                    .iter()
                    .position(|(sp, _, _, _)| *sp >= target_byte)
                    .unwrap_or(inst_count);
                Op::Jump(target_idx)
            }
            (Op::If(_), Some(off)) => {
                let target_byte =
                    (next_byte_pos as isize).saturating_add(off as isize).max(0) as usize;
                let target_idx = raw_insts
                    .iter()
                    .position(|(sp, _, _, _)| *sp >= target_byte)
                    .unwrap_or(inst_count);
                Op::If(target_idx)
            }
            _ => op.clone(),
        };
        ops.push(resolved);
    }
    ops
}

#[derive(Debug, Clone)]
struct TimerEntry {
    id: u32,
    func_or_name: Value,
    interval_frames: u32,
    remaining_frames: u32,
    repeat: bool,
}

/// Complete ActionScript 1 & 2 (AVM1) virtual machine state (TSD 2.9, M4).
#[derive(Debug, Clone)]
pub struct Avm1 {
    /// SWF version (`1..=19`), controlling ES3 vs SWF4/5/6 coercion & case sensitivity rules.
    pub swf_version: u8,
    /// Generational object heap.
    pub heap: Heap,
    /// Value stack.
    pub stack: Vec<Value>,
    /// Global variables (`_global` / `_root`).
    pub globals: BTreeMap<String, Value>,
    /// Constant pool populated by `ActionConstantPool`.
    pub constant_pool: Vec<String>,
    /// Global registers `r0..r3`.
    pub registers: [Value; 4],
    /// Accumulated `trace()` output lines.
    pub trace_log: Vec<String>,
    /// Maximum instructions executed per action block before the watchdog aborts (default `500_000`).
    pub max_ops_per_run: u64,
    /// Whether the instruction watchdog tripped during the most recent action block.
    pub watchdog_tripped: bool,
    /// Deterministic PRNG state for `Math.random()` and `ActionRandomNumber`.
    rng_state: u64,
    /// Elapsed virtual milliseconds (`getTimer()`).
    elapsed_ms: u64,
    /// Currently pressed key codes for `Key.isDown(code)`.
    keys_down: BTreeSet<u32>,
    /// Most recently pressed key code for `Key.getCode()`.
    last_key_code: u32,
    /// Active deterministic frame-counted timers (`setInterval` / `setTimeout`).
    timers: Vec<TimerEntry>,
    next_timer_id: u32,
    /// Master sound volume (`0..=100`, controlled by `Sound.setVolume`).
    pub sound_volume: i32,
    /// Whether `ActionStopSounds` was called this frame.
    pub stop_sounds_requested: bool,
}

impl Default for Avm1 {
    fn default() -> Self {
        Self::new(8)
    }
}

impl Avm1 {
    /// Creates a new [`Avm1`] virtual machine for `swf_version`.
    #[must_use]
    pub fn new(swf_version: u8) -> Self {
        let mut vm = Self {
            swf_version,
            heap: Heap::new(),
            stack: Vec::new(),
            globals: BTreeMap::new(),
            constant_pool: Vec::new(),
            registers: [
                Value::Undefined,
                Value::Undefined,
                Value::Undefined,
                Value::Undefined,
            ],
            trace_log: Vec::new(),
            max_ops_per_run: 500_000,
            watchdog_tripped: false,
            rng_state: 0x5357_4652_4156_4D31,
            elapsed_ms: 0,
            keys_down: BTreeSet::new(),
            last_key_code: 0,
            timers: Vec::new(),
            next_timer_id: 1,
            sound_volume: 100,
            stop_sounds_requested: false,
        };
        vm.init_builtins();
        vm
    }

    fn init_builtins(&mut self) {
        self.set_global("_root", Value::MovieClip("_root".to_owned()));
        self.set_global("NaN", Value::Number(f64::NAN));
        self.set_global("Infinity", Value::Number(f64::INFINITY));

        // Math built-in object with constants
        let math_id = self.heap.alloc_object(None);
        self.heap.set_prop(
            math_id,
            "PI",
            Value::Number(std::f64::consts::PI),
            self.swf_version,
        );
        self.heap.set_prop(
            math_id,
            "E",
            Value::Number(std::f64::consts::E),
            self.swf_version,
        );
        self.heap.set_prop(
            math_id,
            "SQRT2",
            Value::Number(std::f64::consts::SQRT_2),
            self.swf_version,
        );
        self.heap.set_prop(
            math_id,
            "LN2",
            Value::Number(std::f64::consts::LN_2),
            self.swf_version,
        );
        self.heap.set_prop(
            math_id,
            "__builtin__",
            Value::String("Math".to_owned()),
            self.swf_version,
        );
        self.set_global("Math", Value::Object(math_id));

        // Key built-in object with standard Flash key constants
        let key_id = self.heap.alloc_object(None);
        for (name, code) in [
            ("BACKSPACE", 8),
            ("TAB", 9),
            ("ENTER", 13),
            ("SHIFT", 16),
            ("CONTROL", 17),
            ("ESCAPE", 27),
            ("SPACE", 32),
            ("LEFT", 37),
            ("UP", 38),
            ("RIGHT", 39),
            ("DOWN", 40),
        ] {
            self.heap.set_prop(
                key_id,
                name,
                Value::Number(f64::from(code)),
                self.swf_version,
            );
        }
        self.heap.set_prop(
            key_id,
            "__builtin__",
            Value::String("Key".to_owned()),
            self.swf_version,
        );
        self.set_global("Key", Value::Object(key_id));

        // Stage built-in object
        let stage_id = self.heap.alloc_object(None);
        self.heap
            .set_prop(stage_id, "width", Value::Number(320.0), self.swf_version);
        self.heap
            .set_prop(stage_id, "height", Value::Number(240.0), self.swf_version);
        self.heap.set_prop(
            stage_id,
            "scaleMode",
            Value::String("showAll".to_owned()),
            self.swf_version,
        );
        self.set_global("Stage", Value::Object(stage_id));

        // Mouse built-in object
        let mouse_id = self.heap.alloc_object(None);
        self.heap.set_prop(
            mouse_id,
            "__builtin__",
            Value::String("Mouse".to_owned()),
            self.swf_version,
        );
        self.set_global("Mouse", Value::Object(mouse_id));
    }

    /// Sets the key state for `Key.isDown(code)` and `Key.getCode()`.
    pub fn set_key_state(&mut self, key_code: u32, down: bool) {
        if down {
            self.keys_down.insert(key_code);
            self.last_key_code = key_code;
        } else {
            self.keys_down.remove(&key_code);
        }
    }

    /// Steps virtual elapsed time and fires any ready deterministic `setInterval` / `setTimeout` callbacks.
    pub fn step_timers(&mut self, stage: &mut Stage, delta_ms: u64) {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        let mut ready = Vec::new();
        let mut i = 0_usize;
        while i < self.timers.len() {
            if self.timers[i].remaining_frames <= 1 {
                let func = self.timers[i].func_or_name.clone();
                ready.push(func);
                if self.timers[i].repeat {
                    self.timers[i].remaining_frames = self.timers[i].interval_frames.max(1);
                    i += 1;
                } else {
                    self.timers.remove(i);
                }
            } else {
                self.timers[i].remaining_frames -= 1;
                i += 1;
            }
        }
        for cb in ready {
            match cb {
                Value::Object(fid) => {
                    let mut budget = self.max_ops_per_run;
                    let _ = self.invoke_user_function(fid, None, Vec::new(), stage, &mut budget);
                }
                Value::String(fname) => {
                    if let Value::Object(fid) = self.get_global(&fname) {
                        let mut budget = self.max_ops_per_run;
                        let _ =
                            self.invoke_user_function(fid, None, Vec::new(), stage, &mut budget);
                    }
                }
                _ => {}
            }
        }
    }

    /// Gets a global variable (respecting SWF version case sensitivity).
    #[must_use]
    pub fn get_global(&self, name: &str) -> Value {
        let k = normalize_key(name, self.swf_version);
        self.globals.get(&k).cloned().unwrap_or(Value::Undefined)
    }

    /// Sets a global variable (respecting SWF version case sensitivity).
    pub fn set_global(&mut self, name: &str, val: Value) {
        let k = normalize_key(name, self.swf_version);
        self.globals.insert(k, val);
    }

    fn next_random_f64(&mut self) -> f64 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        ((x >> 11) as f64) / ((1_u64 << 53) as f64)
    }

    /// Decodes and executes an AVM1 bytecode slice against `stage` using the resolved `Vec<Op>` interpreter.
    pub fn execute_bytecode(&mut self, bytecode: &[u8], stage: &mut Stage) {
        let ops = decode_ops(bytecode);
        self.execute_ops(&ops, stage);
    }

    /// Executes a pre-decoded ` &[Op]` slice against `stage`.
    pub fn execute_ops(&mut self, ops: &[Op], stage: &mut Stage) {
        self.watchdog_tripped = false;
        let mut budget = self.max_ops_per_run;
        let mut locals = BTreeMap::new();
        let _ = self.run_ops_inner(ops, None, &mut locals, stage, &mut budget, 0);
        if self.heap.allocations_since_gc > 256 {
            let mut roots: Vec<Value> = self.stack.clone();
            roots.extend(self.globals.values().cloned());
            roots.extend(self.registers.iter().cloned());
            let _ = self.heap.collect_garbage(&roots);
        }
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().unwrap_or(Value::Undefined)
    }

    #[allow(clippy::too_many_lines, clippy::arithmetic_side_effects)]
    fn run_ops_inner(
        &mut self,
        ops: &[Op],
        this_obj: Option<ObjId>,
        locals: &mut BTreeMap<String, Value>,
        stage: &mut Stage,
        budget: &mut u64,
        depth: usize,
    ) -> Option<Value> {
        if depth > 256 {
            return Some(Value::Undefined);
        }
        let mut pc = 0_usize;
        while pc < ops.len() {
            if *budget == 0 {
                self.watchdog_tripped = true;
                return None;
            }
            *budget -= 1;

            let op = &ops[pc];
            pc += 1;

            match op {
                Op::End => break,
                Op::NextFrame => {
                    let next = stage
                        .root
                        .current_frame
                        .saturating_add(1)
                        .min(stage.root.frame_count.saturating_sub(1));
                    let _ = stage.goto_frame("_root", next, false);
                }
                Op::PrevFrame => {
                    let prev = stage.root.current_frame.saturating_sub(1);
                    let _ = stage.goto_frame("_root", prev, false);
                }
                Op::Play => {
                    stage.root.playing = true;
                }
                Op::Stop => {
                    stage.root.playing = false;
                }
                Op::StopSounds => {
                    self.stop_sounds_requested = true;
                }
                Op::GotoFrame(f) => {
                    let _ = stage.goto_frame("_root", *f, false);
                }
                Op::GotoLabel(label) => {
                    if let Some(f) = stage.root.resolve_label(label) {
                        let _ = stage.goto_frame("_root", f, false);
                    }
                }
                Op::GotoFrame2(play_after) => {
                    let target = self.pop();
                    match target {
                        Value::Number(n) => {
                            let f = (n as i32).saturating_sub(1).max(0) as u16;
                            let _ = stage.goto_frame("_root", f, *play_after);
                        }
                        Value::String(s) => {
                            if let Some(f) = stage.root.resolve_label(&s) {
                                let _ = stage.goto_frame("_root", f, *play_after);
                            } else if let Ok(n) = s.parse::<u16>() {
                                let _ = stage.goto_frame("_root", n.saturating_sub(1), *play_after);
                            }
                        }
                        _ => {}
                    }
                }
                Op::ConstantPool(pool) => {
                    self.constant_pool.clone_from(pool);
                }
                Op::Push(items) => {
                    for item in items {
                        let v = match item {
                            PushItem::Literal(lit) => lit.clone(),
                            PushItem::Register(reg) => self
                                .registers
                                .get(usize::from(*reg))
                                .cloned()
                                .unwrap_or(Value::Undefined),
                            PushItem::Constant(cidx) => self
                                .constant_pool
                                .get(usize::from(*cidx))
                                .cloned()
                                .map_or(Value::Undefined, Value::String),
                        };
                        self.stack.push(v);
                    }
                }
                Op::Pop => {
                    let _ = self.pop();
                }
                Op::PushDuplicate => {
                    let top = self.stack.last().cloned().unwrap_or(Value::Undefined);
                    self.stack.push(top);
                }
                Op::StackSwap => {
                    let a = self.pop();
                    let b = self.pop();
                    self.stack.push(a);
                    self.stack.push(b);
                }
                Op::StoreRegister(reg) => {
                    let top = self.stack.last().cloned().unwrap_or(Value::Undefined);
                    if let Some(slot) = self.registers.get_mut(usize::from(*reg)) {
                        *slot = top;
                    }
                }
                Op::Trace => {
                    let v = self.pop();
                    let s = v.to_avm_string(self.swf_version, &self.heap);
                    self.trace_log.push(s);
                }
                Op::AddSwf4 => {
                    let a = self.pop().to_number(self.swf_version);
                    let b = self.pop().to_number(self.swf_version);
                    self.stack.push(Value::Number(b + a));
                }
                Op::Add2 => {
                    let a = self.pop();
                    let b = self.pop();
                    if matches!(a, Value::String(_)) || matches!(b, Value::String(_)) {
                        let sb = b.to_avm_string(self.swf_version, &self.heap);
                        let sa = a.to_avm_string(self.swf_version, &self.heap);
                        self.stack.push(Value::String(format!("{sb}{sa}")));
                    } else {
                        let nb = b.to_number(self.swf_version);
                        let na = a.to_number(self.swf_version);
                        self.stack.push(Value::Number(nb + na));
                    }
                }
                Op::Subtract => {
                    let a = self.pop().to_number(self.swf_version);
                    let b = self.pop().to_number(self.swf_version);
                    self.stack.push(Value::Number(b - a));
                }
                Op::Multiply => {
                    let a = self.pop().to_number(self.swf_version);
                    let b = self.pop().to_number(self.swf_version);
                    self.stack.push(Value::Number(b * a));
                }
                Op::Divide => {
                    let a = self.pop().to_number(self.swf_version);
                    let b = self.pop().to_number(self.swf_version);
                    self.stack.push(Value::Number(b / a));
                }
                Op::Modulo => {
                    let a = self.pop().to_number(self.swf_version);
                    let b = self.pop().to_number(self.swf_version);
                    self.stack.push(Value::Number(b % a));
                }
                Op::Increment => {
                    let a = self.pop().to_number(self.swf_version);
                    self.stack.push(Value::Number(a + 1.0));
                }
                Op::Decrement => {
                    let a = self.pop().to_number(self.swf_version);
                    self.stack.push(Value::Number(a - 1.0));
                }
                Op::EqualsSwf4 => {
                    let a = self.pop().to_number(self.swf_version);
                    let b = self.pop().to_number(self.swf_version);
                    self.stack.push(Value::Bool(b == a));
                }
                Op::Equals2 => {
                    let a = self.pop();
                    let b = self.pop();
                    let eq = self.abstract_equals(&b, &a);
                    self.stack.push(Value::Bool(eq));
                }
                Op::StrictEquals => {
                    let a = self.pop();
                    let b = self.pop();
                    let eq = self.strict_equals(&b, &a);
                    self.stack.push(Value::Bool(eq));
                }
                Op::LessSwf4 => {
                    let a = self.pop().to_number(self.swf_version);
                    let b = self.pop().to_number(self.swf_version);
                    self.stack.push(Value::Bool(b < a));
                }
                Op::Less2 => {
                    let a = self.pop();
                    let b = self.pop();
                    let res = self.abstract_less(&b, &a).unwrap_or(false);
                    self.stack.push(Value::Bool(res));
                }
                Op::Greater => {
                    let a = self.pop();
                    let b = self.pop();
                    let res = self.abstract_less(&a, &b).unwrap_or(false);
                    self.stack.push(Value::Bool(res));
                }
                Op::And => {
                    let a = self.pop().to_bool(self.swf_version);
                    let b = self.pop().to_bool(self.swf_version);
                    self.stack.push(Value::Bool(b && a));
                }
                Op::Or => {
                    let a = self.pop().to_bool(self.swf_version);
                    let b = self.pop().to_bool(self.swf_version);
                    self.stack.push(Value::Bool(b || a));
                }
                Op::Not => {
                    let a = self.pop().to_bool(self.swf_version);
                    self.stack.push(Value::Bool(!a));
                }
                Op::BitAnd => {
                    let a = self.pop().to_i32(self.swf_version);
                    let b = self.pop().to_i32(self.swf_version);
                    self.stack.push(Value::Number(f64::from(b & a)));
                }
                Op::BitOr => {
                    let a = self.pop().to_i32(self.swf_version);
                    let b = self.pop().to_i32(self.swf_version);
                    self.stack.push(Value::Number(f64::from(b | a)));
                }
                Op::BitXor => {
                    let a = self.pop().to_i32(self.swf_version);
                    let b = self.pop().to_i32(self.swf_version);
                    self.stack.push(Value::Number(f64::from(b ^ a)));
                }
                Op::BitLShift => {
                    let a = self.pop().to_u32(self.swf_version) & 0x1F;
                    let b = self.pop().to_i32(self.swf_version);
                    self.stack.push(Value::Number(f64::from(b.wrapping_shl(a))));
                }
                Op::BitRShift => {
                    let a = self.pop().to_u32(self.swf_version) & 0x1F;
                    let b = self.pop().to_i32(self.swf_version);
                    self.stack.push(Value::Number(f64::from(b.wrapping_shr(a))));
                }
                Op::BitURShift => {
                    let a = self.pop().to_u32(self.swf_version) & 0x1F;
                    let b = self.pop().to_u32(self.swf_version);
                    self.stack.push(Value::Number(f64::from(b.wrapping_shr(a))));
                }
                Op::StringAdd => {
                    let a = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let b = self.pop().to_avm_string(self.swf_version, &self.heap);
                    self.stack.push(Value::String(format!("{b}{a}")));
                }
                Op::StringEquals => {
                    let a = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let b = self.pop().to_avm_string(self.swf_version, &self.heap);
                    self.stack.push(Value::Bool(b == a));
                }
                Op::StringGreater => {
                    let a = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let b = self.pop().to_avm_string(self.swf_version, &self.heap);
                    self.stack.push(Value::Bool(b > a));
                }
                Op::StringLength => {
                    let s = self.pop().to_avm_string(self.swf_version, &self.heap);
                    self.stack.push(Value::Number(s.chars().count() as f64));
                }
                Op::StringExtract => {
                    let count = self.pop().to_i32(self.swf_version).max(0) as usize;
                    let index =
                        self.pop().to_i32(self.swf_version).saturating_sub(1).max(0) as usize;
                    let s = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let sub: String = s.chars().skip(index).take(count).collect();
                    self.stack.push(Value::String(sub));
                }
                Op::ToInteger => {
                    let v = self.pop().to_i32(self.swf_version);
                    self.stack.push(Value::Number(f64::from(v)));
                }
                Op::ToNumber => {
                    let v = self.pop().to_number(self.swf_version);
                    self.stack.push(Value::Number(v));
                }
                Op::ToStringOp => {
                    let s = self.pop().to_avm_string(self.swf_version, &self.heap);
                    self.stack.push(Value::String(s));
                }
                Op::TypeOf => {
                    let v = self.pop();
                    let t = v.typeof_str(&self.heap);
                    self.stack.push(Value::String(t.to_owned()));
                }
                Op::RandomNumber => {
                    let max = self.pop().to_i32(self.swf_version);
                    let r = if max <= 0 {
                        0
                    } else {
                        ((self.next_random_f64() * (max as f64)).floor() as i32).clamp(0, max - 1)
                    };
                    self.stack.push(Value::Number(f64::from(r)));
                }
                Op::GetVariable => {
                    let name = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let val = self.resolve_variable(&name, this_obj, locals, stage);
                    self.stack.push(val);
                }
                Op::SetVariable => {
                    let val = self.pop();
                    let name = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let norm = normalize_key(&name, self.swf_version);
                    if locals.contains_key(&norm) {
                        locals.insert(norm, val);
                    } else {
                        let s_str = val.to_avm_string(self.swf_version, &self.heap);
                        stage.sync_variable(&name, &s_str);
                        self.set_global(&name, val);
                    }
                }
                Op::DefineLocal => {
                    let val = self.pop();
                    let name = self.pop().to_avm_string(self.swf_version, &self.heap);
                    locals.insert(normalize_key(&name, self.swf_version), val);
                }
                Op::DefineLocal2 => {
                    let name = self.pop().to_avm_string(self.swf_version, &self.heap);
                    locals
                        .entry(normalize_key(&name, self.swf_version))
                        .or_insert(Value::Undefined);
                }
                Op::Delete => {
                    let name = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let target = self.pop();
                    let ok = if let Value::Object(id) = target {
                        self.heap.delete_prop(id, &name, self.swf_version)
                    } else {
                        let norm = normalize_key(&name, self.swf_version);
                        self.globals.remove(&norm).is_some()
                    };
                    self.stack.push(Value::Bool(ok));
                }
                Op::InitObject => {
                    let count = self.pop().to_i32(self.swf_version).max(0) as usize;
                    let obj_id = self.heap.alloc_object(None);
                    for _ in 0..count.min(4096) {
                        let val = self.pop();
                        let key = self.pop().to_avm_string(self.swf_version, &self.heap);
                        self.heap.set_prop(obj_id, &key, val, self.swf_version);
                    }
                    self.stack.push(Value::Object(obj_id));
                }
                Op::InitArray => {
                    let count = self.pop().to_i32(self.swf_version).max(0) as usize;
                    let mut elems = Vec::with_capacity(count.min(4096));
                    for _ in 0..count.min(4096) {
                        elems.push(self.pop());
                    }
                    let arr_id = self.heap.alloc_array(elems);
                    self.stack.push(Value::Object(arr_id));
                }
                Op::GetMember => {
                    let prop = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let obj = self.pop();
                    let val = self.get_member_value(&obj, &prop, stage);
                    self.stack.push(val);
                }
                Op::SetMember => {
                    let val = self.pop();
                    let prop = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let obj = self.pop();
                    self.set_member_value(&obj, &prop, val, stage);
                }
                Op::GetProperty => {
                    let idx = self.pop().to_i32(self.swf_version);
                    let _target = self.pop();
                    let val = match idx {
                        4 => Value::Number(f64::from(stage.root.current_frame) + 1.0), // _currentframe
                        5 => Value::Number(f64::from(stage.root.frame_count)), // _totalframes
                        8 => Value::Number(f64::from(stage.width_px)),         // _width
                        9 => Value::Number(f64::from(stage.height_px)),        // _height
                        _ => Value::Number(0.0),
                    };
                    self.stack.push(val);
                }
                Op::SetProperty => {
                    let _val = self.pop();
                    let _idx = self.pop();
                    let _target = self.pop();
                }
                Op::Enumerate => {
                    let obj = self.pop();
                    self.stack.push(Value::Null);
                    if let Value::Object(id) = obj {
                        for k in self.heap.own_keys(id) {
                            self.stack.push(Value::String(k));
                        }
                    }
                }
                Op::DefineFunction {
                    name,
                    params,
                    body_ops,
                } => {
                    let func = UserFunction {
                        name: name.clone(),
                        params: params.clone(),
                        ops: body_ops.clone(),
                        captured_scope: locals.clone(),
                    };
                    let fid = self.heap.alloc_function(func);
                    if name.is_empty() {
                        self.stack.push(Value::Object(fid));
                    } else {
                        self.set_global(name, Value::Object(fid));
                    }
                }
                Op::CallFunction => {
                    let fname = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let nargs = self.pop().to_i32(self.swf_version).max(0) as usize;
                    let mut args = Vec::with_capacity(nargs.min(256));
                    for _ in 0..nargs.min(256) {
                        args.push(self.pop());
                    }
                    let res = self
                        .call_named_function(&fname, this_obj, locals, args, stage, budget, depth);
                    self.stack.push(res);
                }
                Op::CallMethod => {
                    let mname = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let receiver = self.pop();
                    let nargs = self.pop().to_i32(self.swf_version).max(0) as usize;
                    let mut args = Vec::with_capacity(nargs.min(256));
                    for _ in 0..nargs.min(256) {
                        args.push(self.pop());
                    }
                    let res = self.call_method_on(&receiver, &mname, args, stage, budget, depth);
                    self.stack.push(res);
                }
                Op::NewObject => {
                    let cname = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let nargs = self.pop().to_i32(self.swf_version).max(0) as usize;
                    let mut args = Vec::with_capacity(nargs.min(256));
                    for _ in 0..nargs.min(256) {
                        args.push(self.pop());
                    }
                    let res = self.construct_object(&cname, args, stage, budget, depth);
                    self.stack.push(res);
                }
                Op::Return => {
                    return Some(self.pop());
                }
                Op::Jump(target_idx) => {
                    pc = *target_idx;
                }
                Op::If(target_idx) => {
                    let cond = self.pop().to_bool(self.swf_version);
                    if cond {
                        pc = *target_idx;
                    }
                }
            }
        }
        None
    }

    fn resolve_variable(
        &self,
        name: &str,
        this_obj: Option<ObjId>,
        locals: &BTreeMap<String, Value>,
        stage: &Stage,
    ) -> Value {
        let norm = normalize_key(name, self.swf_version);
        if norm == "this" {
            return this_obj.map_or_else(|| Value::MovieClip("_root".to_owned()), Value::Object);
        }
        if norm == "_root" || norm == "_level0" {
            return Value::MovieClip("_root".to_owned());
        }
        if let Some(v) = locals.get(&norm) {
            return v.clone();
        }
        if norm == "_currentframe" {
            return Value::Number(f64::from(stage.root.current_frame) + 1.0);
        }
        if norm == "_totalframes" {
            return Value::Number(f64::from(stage.root.frame_count));
        }
        self.get_global(name)
    }

    fn get_member_value(&self, obj: &Value, prop: &str, stage: &Stage) -> Value {
        match obj {
            Value::String(s) => {
                if prop == "length" {
                    Value::Number(s.chars().count() as f64)
                } else {
                    Value::Undefined
                }
            }
            Value::MovieClip(path) => {
                let norm = normalize_key(prop, self.swf_version);
                match norm.as_str() {
                    "_currentframe" => Value::Number(f64::from(stage.root.current_frame) + 1.0),
                    "_totalframes" => Value::Number(f64::from(stage.root.frame_count)),
                    "_width" => Value::Number(f64::from(stage.width_px)),
                    "_height" => Value::Number(f64::from(stage.height_px)),
                    "_name" => Value::String(path.clone()),
                    _ => self.get_global(prop),
                }
            }
            Value::Object(id) => self.heap.get_prop(*id, prop, self.swf_version),
            _ => Value::Undefined,
        }
    }

    fn set_member_value(&mut self, obj: &Value, prop: &str, val: Value, stage: &mut Stage) {
        match obj {
            Value::Object(id) => {
                self.heap.set_prop(*id, prop, val, self.swf_version);
            }
            Value::MovieClip(_) => {
                let s_str = val.to_avm_string(self.swf_version, &self.heap);
                stage.sync_variable(prop, &s_str);
                self.set_global(prop, val);
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments, clippy::arithmetic_side_effects)]
    fn call_named_function(
        &mut self,
        fname: &str,
        this_obj: Option<ObjId>,
        locals: &BTreeMap<String, Value>,
        args: Vec<Value>,
        stage: &mut Stage,
        budget: &mut u64,
        _depth: usize,
    ) -> Value {
        let norm = normalize_key(fname, self.swf_version);
        let lower = fname.to_ascii_lowercase();
        match lower.as_str() {
            "number" => args.first().map_or(Value::Number(0.0), |v| {
                Value::Number(v.to_number(self.swf_version))
            }),
            "string" => args.first().map_or_else(
                || Value::String(String::new()),
                |v| Value::String(v.to_avm_string(self.swf_version, &self.heap)),
            ),
            "boolean" => args.first().map_or(Value::Bool(false), |v| {
                Value::Bool(v.to_bool(self.swf_version))
            }),
            "parseint" => {
                let s = args.first().map_or_else(String::new, |v| {
                    v.to_avm_string(self.swf_version, &self.heap)
                });
                let radix = args
                    .get(1)
                    .map_or(10, |r| r.to_i32(self.swf_version).clamp(2, 36) as u32);
                let trimmed = s.trim();
                let parsed = if let Some(hex) = trimmed
                    .strip_prefix("0x")
                    .or_else(|| trimmed.strip_prefix("0X"))
                {
                    i64::from_str_radix(hex, 16).map_or(f64::NAN, |v| v as f64)
                } else {
                    i64::from_str_radix(trimmed, radix).map_or(f64::NAN, |v| v as f64)
                };
                Value::Number(parsed)
            }
            "parsefloat" => {
                let s = args.first().map_or_else(String::new, |v| {
                    v.to_avm_string(self.swf_version, &self.heap)
                });
                Value::Number(s.trim().parse::<f64>().unwrap_or(f64::NAN))
            }
            "isnan" => {
                let n = args
                    .first()
                    .map_or(f64::NAN, |v| v.to_number(self.swf_version));
                Value::Bool(n.is_nan())
            }
            "isfinite" => {
                let n = args
                    .first()
                    .map_or(f64::NAN, |v| v.to_number(self.swf_version));
                Value::Bool(n.is_finite())
            }
            "gettimer" => Value::Number(self.elapsed_ms as f64),
            "setinterval" | "settimeout" => {
                let repeat = lower == "setinterval";
                let cb = args.first().cloned().unwrap_or(Value::Undefined);
                let ms = args
                    .get(1)
                    .map_or(16.0, |v| v.to_number(self.swf_version).max(1.0));
                let fps = stage.fps.max(1.0) as f64;
                let frames = ((ms * fps / 1000.0).round() as u32).max(1);
                let id = self.next_timer_id;
                self.next_timer_id = self.next_timer_id.saturating_add(1);
                self.timers.push(TimerEntry {
                    id,
                    func_or_name: cb,
                    interval_frames: frames,
                    remaining_frames: frames,
                    repeat,
                });
                Value::Number(f64::from(id))
            }
            "clearinterval" | "cleartimeout" => {
                let id = args.first().map_or(0, |v| v.to_u32(self.swf_version));
                self.timers.retain(|t| t.id != id);
                Value::Undefined
            }
            _ => {
                let fval = locals
                    .get(&norm)
                    .cloned()
                    .unwrap_or_else(|| self.get_global(fname));
                if let Value::Object(fid) = fval {
                    self.invoke_user_function(fid, this_obj, args, stage, budget)
                        .unwrap_or(Value::Undefined)
                } else {
                    Value::Undefined
                }
            }
        }
    }

    #[allow(clippy::too_many_lines, clippy::arithmetic_side_effects)]
    fn call_method_on(
        &mut self,
        receiver: &Value,
        mname: &str,
        args: Vec<Value>,
        stage: &mut Stage,
        budget: &mut u64,
        depth: usize,
    ) -> Value {
        let norm = mname.to_ascii_lowercase();

        // 1. String prototype methods
        if let Value::String(s) = receiver {
            return match norm.as_str() {
                "tolowercase" => Value::String(s.to_lowercase()),
                "touppercase" => Value::String(s.to_uppercase()),
                "charat" => {
                    let idx = args.first().map_or(0, |v| v.to_i32(self.swf_version));
                    let ch = if idx < 0 {
                        String::new()
                    } else {
                        s.chars()
                            .nth(idx as usize)
                            .map_or_else(String::new, |c| c.to_string())
                    };
                    Value::String(ch)
                }
                "charcodeat" => {
                    let idx = args.first().map_or(0, |v| v.to_i32(self.swf_version));
                    let code = if idx < 0 {
                        f64::NAN
                    } else {
                        s.chars()
                            .nth(idx as usize)
                            .map_or(f64::NAN, |c| f64::from(c as u32))
                    };
                    Value::Number(code)
                }
                "indexof" => {
                    let needle = args.first().map_or_else(String::new, |v| {
                        v.to_avm_string(self.swf_version, &self.heap)
                    });
                    let pos = s.find(&needle).map_or(-1.0, |i| i as f64);
                    Value::Number(pos)
                }
                "substr" => {
                    let chars: Vec<char> = s.chars().collect();
                    let len = chars.len() as i32;
                    let mut start = args.first().map_or(0, |v| v.to_i32(self.swf_version));
                    if start < 0 {
                        start = (len + start).max(0);
                    }
                    let count = args
                        .get(1)
                        .map_or(len, |v| v.to_i32(self.swf_version).max(0));
                    let out: String = chars
                        .into_iter()
                        .skip(start as usize)
                        .take(count as usize)
                        .collect();
                    Value::String(out)
                }
                "substring" => {
                    let chars: Vec<char> = s.chars().collect();
                    let len = chars.len() as i32;
                    let a = args
                        .first()
                        .map_or(0, |v| v.to_i32(self.swf_version).clamp(0, len));
                    let b = args
                        .get(1)
                        .map_or(len, |v| v.to_i32(self.swf_version).clamp(0, len));
                    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                    let out: String = chars
                        .into_iter()
                        .skip(lo as usize)
                        .take((hi - lo) as usize)
                        .collect();
                    Value::String(out)
                }
                "split" => {
                    let delim = args.first().map_or_else(String::new, |v| {
                        v.to_avm_string(self.swf_version, &self.heap)
                    });
                    let parts: Vec<Value> = if delim.is_empty() {
                        s.chars().map(|c| Value::String(c.to_string())).collect()
                    } else {
                        s.split(&delim)
                            .map(|p| Value::String(p.to_owned()))
                            .collect()
                    };
                    Value::Object(self.heap.alloc_array(parts))
                }
                _ => Value::Undefined,
            };
        }

        // 2. MovieClip methods
        if let Value::MovieClip(path) = receiver {
            return match norm.as_str() {
                "play" => {
                    stage.root.playing = true;
                    Value::Undefined
                }
                "stop" => {
                    stage.root.playing = false;
                    Value::Undefined
                }
                "gotoandstop" | "gotoandplay" => {
                    let play_after = norm == "gotoandplay";
                    if let Some(arg) = args.first() {
                        match arg {
                            Value::Number(n) => {
                                let f = (*n as i32).saturating_sub(1).max(0) as u16;
                                let _ = stage.goto_frame(path, f, play_after);
                            }
                            Value::String(lbl) => {
                                if let Some(f) = stage.root.resolve_label(lbl) {
                                    let _ = stage.goto_frame(path, f, play_after);
                                }
                            }
                            _ => {}
                        }
                    }
                    Value::Undefined
                }
                _ => Value::Undefined,
            };
        }

        // 3. Heap Objects (Built-in Math, Key, Sound, Array, or user objects)
        let Value::Object(obj_id) = *receiver else {
            return Value::Undefined;
        };

        if let Value::String(btag) = self.heap.get_prop(obj_id, "__builtin__", self.swf_version) {
            if btag == "Math" {
                let x = args
                    .first()
                    .map_or(f64::NAN, |v| v.to_number(self.swf_version));
                let y = args
                    .get(1)
                    .map_or(f64::NAN, |v| v.to_number(self.swf_version));
                let val = match norm.as_str() {
                    "abs" => x.abs(),
                    "floor" => x.floor(),
                    "ceil" => x.ceil(),
                    "round" => (x + 0.5).floor(),
                    "sqrt" => x.sqrt(),
                    "sin" => x.sin(),
                    "cos" => x.cos(),
                    "tan" => x.tan(),
                    "atan2" => x.atan2(y),
                    "pow" => x.powf(y),
                    "min" => x.min(y),
                    "max" => x.max(y),
                    "random" => self.next_random_f64(),
                    _ => f64::NAN,
                };
                return Value::Number(val);
            }
            if btag == "Key" {
                return match norm.as_str() {
                    "isdown" => {
                        let code = args.first().map_or(0, |v| v.to_u32(self.swf_version));
                        Value::Bool(self.keys_down.contains(&code))
                    }
                    "getcode" => Value::Number(f64::from(self.last_key_code)),
                    _ => Value::Undefined,
                };
            }
            if btag == "Sound" {
                return match norm.as_str() {
                    "setvolume" => {
                        self.sound_volume = args
                            .first()
                            .map_or(100, |v| v.to_i32(self.swf_version).clamp(0, 100));
                        Value::Undefined
                    }
                    "getvolume" => Value::Number(f64::from(self.sound_volume)),
                    "stop" => {
                        self.stop_sounds_requested = true;
                        Value::Undefined
                    }
                    _ => Value::Undefined,
                };
            }
        }

        // Array methods
        if self.heap.array_elements(obj_id).is_some() {
            match norm.as_str() {
                "push" => {
                    let len = self
                        .heap
                        .with_array_mut(obj_id, |arr| {
                            arr.extend(args);
                            arr.len()
                        })
                        .unwrap_or(0);
                    return Value::Number(len as f64);
                }
                "pop" => {
                    return self
                        .heap
                        .with_array_mut(obj_id, |arr| arr.pop().unwrap_or(Value::Undefined))
                        .unwrap_or(Value::Undefined);
                }
                "reverse" => {
                    let _ = self.heap.with_array_mut(obj_id, |arr| arr.reverse());
                    return Value::Object(obj_id);
                }
                "join" => {
                    let sep = args.first().map_or_else(
                        || ",".to_owned(),
                        |v| v.to_avm_string(self.swf_version, &self.heap),
                    );
                    let elems = self.heap.array_elements(obj_id).unwrap_or(&[]).to_vec();
                    let joined: Vec<String> = elems
                        .iter()
                        .map(|v| v.to_avm_string(self.swf_version, &self.heap))
                        .collect();
                    return Value::String(joined.join(&sep));
                }
                "slice" => {
                    let elems = self.heap.array_elements(obj_id).unwrap_or(&[]).to_vec();
                    let len = elems.len() as i32;
                    let start = args
                        .first()
                        .map_or(0, |v| v.to_i32(self.swf_version).clamp(0, len))
                        as usize;
                    let end = args
                        .get(1)
                        .map_or(len, |v| v.to_i32(self.swf_version).clamp(0, len))
                        as usize;
                    let slice = if start <= end {
                        elems.get(start..end).unwrap_or(&[]).to_vec()
                    } else {
                        Vec::new()
                    };
                    return Value::Object(self.heap.alloc_array(slice));
                }
                _ => {}
            }
        }

        // User-defined method on object or prototype chain
        let mval = self.heap.get_prop(obj_id, mname, self.swf_version);
        if let Value::Object(fid) = mval {
            if depth < 256 {
                return self
                    .invoke_user_function(fid, Some(obj_id), args, stage, budget)
                    .unwrap_or(Value::Undefined);
            }
        }
        Value::Undefined
    }

    fn construct_object(
        &mut self,
        cname: &str,
        args: Vec<Value>,
        stage: &mut Stage,
        budget: &mut u64,
        _depth: usize,
    ) -> Value {
        let norm = cname.to_ascii_lowercase();
        match norm.as_str() {
            "object" => Value::Object(self.heap.alloc_object(None)),
            "array" => {
                if args.len() == 1 {
                    if let Some(Value::Number(n)) = args.first() {
                        let len = (*n as usize).min(65536);
                        return Value::Object(self.heap.alloc_array(vec![Value::Undefined; len]));
                    }
                }
                Value::Object(self.heap.alloc_array(args))
            }
            "sound" => {
                let sid = self.heap.alloc_object(None);
                self.heap.set_prop(
                    sid,
                    "__builtin__",
                    Value::String("Sound".to_owned()),
                    self.swf_version,
                );
                Value::Object(sid)
            }
            "string" => Value::String(args.first().map_or_else(String::new, |v| {
                v.to_avm_string(self.swf_version, &self.heap)
            })),
            "number" => Value::Number(args.first().map_or(0.0, |v| v.to_number(self.swf_version))),
            "boolean" => Value::Bool(args.first().is_some_and(|v| v.to_bool(self.swf_version))),
            _ => {
                if let Value::Object(ctor_id) = self.get_global(cname) {
                    let proto = match self.heap.get_prop(ctor_id, "prototype", self.swf_version) {
                        Value::Object(pid) => Some(pid),
                        _ => None,
                    };
                    let inst_id = self.heap.alloc_object(proto);
                    let _ = self.invoke_user_function(ctor_id, Some(inst_id), args, stage, budget);
                    Value::Object(inst_id)
                } else {
                    Value::Object(self.heap.alloc_object(None))
                }
            }
        }
    }

    fn invoke_user_function(
        &mut self,
        fid: ObjId,
        this_obj: Option<ObjId>,
        args: Vec<Value>,
        stage: &mut Stage,
        budget: &mut u64,
    ) -> Option<Value> {
        let func = self.heap.get_function(fid)?.clone();
        let mut frame_locals = func.captured_scope;
        for (i, param_name) in func.params.iter().enumerate() {
            let arg_val = args.get(i).cloned().unwrap_or(Value::Undefined);
            frame_locals.insert(normalize_key(param_name, self.swf_version), arg_val);
        }
        let args_arr = self.heap.alloc_array(args);
        frame_locals.insert("arguments".to_owned(), Value::Object(args_arr));

        let ret = self.run_ops_inner(&func.ops, this_obj, &mut frame_locals, stage, budget, 1);
        Some(ret.unwrap_or(Value::Undefined))
    }

    /// ECMA-262 3rd edition 11.9.3 Abstract Equality Comparison (`==`).
    #[must_use]
    pub fn abstract_equals(&self, x: &Value, y: &Value) -> bool {
        match (x, y) {
            (Value::Undefined | Value::Null, Value::Undefined | Value::Null) => true,
            (Value::Number(nx), Value::Number(ny)) => nx == ny,
            (Value::String(sx), Value::String(sy)) => sx == sy,
            (Value::Bool(bx), Value::Bool(by)) => bx == by,
            (Value::Object(ox), Value::Object(oy)) => ox == oy,
            (Value::MovieClip(mx), Value::MovieClip(my)) => mx == my,
            (Value::Number(nx), Value::String(_)) => *nx == y.to_number(self.swf_version),
            (Value::String(_), Value::Number(ny)) => x.to_number(self.swf_version) == *ny,
            (Value::Bool(_), _) => {
                self.abstract_equals(&Value::Number(x.to_number(self.swf_version)), y)
            }
            (_, Value::Bool(_)) => {
                self.abstract_equals(x, &Value::Number(y.to_number(self.swf_version)))
            }
            _ => false,
        }
    }

    /// ECMA-262 3rd edition 11.9.6 Strict Equality Comparison (`===`).
    #[must_use]
    pub fn strict_equals(&self, x: &Value, y: &Value) -> bool {
        match (x, y) {
            (Value::Undefined, Value::Undefined) | (Value::Null, Value::Null) => true,
            (Value::Number(nx), Value::Number(ny)) => nx == ny,
            (Value::String(sx), Value::String(sy)) => sx == sy,
            (Value::Bool(bx), Value::Bool(by)) => bx == by,
            (Value::Object(ox), Value::Object(oy)) => ox == oy,
            (Value::MovieClip(mx), Value::MovieClip(my)) => mx == my,
            _ => false,
        }
    }

    /// ECMA-262 3rd edition 11.8.5 Abstract Relational Comparison (`x < y`).
    #[must_use]
    pub fn abstract_less(&self, x: &Value, y: &Value) -> Option<bool> {
        if let (Value::String(sx), Value::String(sy)) = (x, y) {
            return Some(sx < sy);
        }
        let nx = x.to_number(self.swf_version);
        let ny = y.to_number(self.swf_version);
        if nx.is_nan() || ny.is_nan() {
            None
        } else {
            Some(nx < ny)
        }
    }
}
