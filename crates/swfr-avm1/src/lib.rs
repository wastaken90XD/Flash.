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
use std::sync::Arc;
use swfr_reader::Reader;
use swfr_scene::Stage;
use swfr_swf::SoundInfo;

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
            Self::Null => 0.0,
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
    pub ops: Arc<[Op]>,
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

fn swf4_property_name(idx: i32) -> Option<&'static str> {
    Some(match idx {
        0 => "_x",
        1 => "_y",
        2 => "_xscale",
        3 => "_yscale",
        4 => "_currentframe",
        5 => "_totalframes",
        6 => "_alpha",
        7 => "_visible",
        8 => "_width",
        9 => "_height",
        10 => "_rotation",
        11 => "_target",
        12 => "_framesloaded",
        13 => "_name",
        14 => "_droptarget",
        15 => "_url",
        16 => "_highquality",
        17 => "_focusrect",
        18 => "_soundbuftime",
        19 => "_quality",
        20 => "_xmouse",
        21 => "_ymouse",
        _ => return None,
    })
}

fn parent_clip_path(path: &str) -> String {
    path.rsplit_once('.').map_or_else(
        || "_root".to_owned(),
        |(parent, _)| {
            if parent.is_empty() {
                "_root".to_owned()
            } else {
                parent.to_owned()
            }
        },
    )
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
    /// `ActionGetTime` (`0x34`).
    GetTime,
    /// `ActionCharToAscii` (`0x32`) / `ActionMBCharToAscii` (`0x36`).
    CharToAscii,
    /// `ActionAsciiToChar` (`0x33`) / `ActionMBAsciiToChar` (`0x37`).
    AsciiToChar,
    /// `ActionTargetPath` (`0x45`).
    TargetPath,
    /// `ActionGetURL` (`0x83`, SWF4 form with literal URL and target strings).
    GetUrl {
        /// URL string.
        url: String,
        /// Target window / level string.
        target: String,
    },
    /// `ActionGetURL2` (`0x9A`, stack form).
    GetUrl2,
    /// `ActionSetTarget` (`0x8B`) / `ActionSetTarget2` (`0x1F`, stack form when `stack_form`).
    SetTarget {
        /// Literal target path (empty in stack form).
        target: String,
        /// Whether the target is popped from the stack.
        stack_form: bool,
    },
    /// `ActionWaitForFrame` (`0x8A`) / `ActionWaitForFrame2` (`0x8D`, stack form when `stack_form`).
    WaitForFrame {
        /// Literal frame number (0-based) in the non-stack form.
        frame: u16,
        /// Number of following instructions to skip when the frame is not yet loaded.
        skip: u8,
        /// Whether the frame is popped from the stack.
        stack_form: bool,
    },
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
            0x1F => Op::SetTarget {
                target: String::new(),
                stack_form: true,
            },
            0x21 => Op::StringAdd,
            0x22 => Op::GetProperty,
            0x23 => Op::SetProperty,
            0x26 => Op::Trace,
            0x30 => Op::RandomNumber,
            0x32 | 0x36 => Op::CharToAscii,
            0x33 | 0x37 => Op::AsciiToChar,
            0x34 => Op::GetTime,
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
            0x45 => Op::TargetPath,
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
            0x83 => Op::GetUrl {
                url: pr.read_c_string().unwrap_or_default(),
                target: pr.read_c_string().unwrap_or_default(),
            },
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
            0x8A => Op::WaitForFrame {
                frame: pr.read_u16().unwrap_or(0),
                skip: pr.read_u8().unwrap_or(0),
                stack_form: false,
            },
            0x8B => Op::SetTarget {
                target: pr.read_c_string().unwrap_or_default(),
                stack_form: false,
            },
            0x8C => Op::GotoLabel(pr.read_c_string().unwrap_or_default()),
            0x8D => Op::WaitForFrame {
                frame: 0,
                skip: pr.read_u8().unwrap_or(0),
                stack_form: true,
            },
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
            0x9A => Op::GetUrl2,
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
    /// Persistent `SharedObject` data objects keyed by local name (`SharedObject.getLocal`).
    pub shared_objects: BTreeMap<String, ObjId>,
    /// 64 KiB `RETRO_MEMORY_SAVE_RAM` buffer with `"SWFS"` magic + CRC32 header (TSD 2.5, M5.5).
    pub save_ram: Vec<u8>,
    /// Pending `loadMovie(url, target)` requests queued during the frame (TSD M5.6).
    pub load_movie_requests: Vec<(String, String)>,
    /// Target-frame actions queued by `gotoAndStop` / `gotoAndPlay` during an action block (TSD 4.6 item 7).
    pending_goto_actions: Vec<(String, Vec<u8>)>,
    /// Dynamic properties assigned to movie clips, keyed by canonical clip path.
    clip_properties: BTreeMap<String, BTreeMap<String, Value>>,
    /// Canonical target path of the action block currently executing.
    current_target_path: String,
    /// Bounded cache of decoded action blocks (real movies re-run the same frame/clip scripts every frame).
    ops_cache: BTreeMap<Vec<u8>, Arc<[Op]>>,
    /// Event sounds requested by `Sound.attachSound()` / `Sound.start()`.
    pub sound_starts: Vec<(u16, SoundInfo)>,
}

/// Size in bytes of the libretro `RETRO_MEMORY_SAVE_RAM` buffer (`64 KiB`, TSD 2.5).
pub const SAVE_RAM_SIZE: usize = 64 * 1024;

/// Computes standard IEEE 802.3 CRC32 over `bytes`.
#[must_use]
pub fn compute_crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
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
        let mut save_ram = vec![0_u8; SAVE_RAM_SIZE];
        let empty_crc = compute_crc32(&[]);
        save_ram[0..4].copy_from_slice(b"SWFS");
        save_ram[4..8].copy_from_slice(&empty_crc.to_le_bytes());
        save_ram[8..12].copy_from_slice(&0_u32.to_le_bytes());
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
            shared_objects: BTreeMap::new(),
            save_ram,
            load_movie_requests: Vec::new(),
            pending_goto_actions: Vec::new(),
            clip_properties: BTreeMap::new(),
            current_target_path: "_root".to_owned(),
            ops_cache: BTreeMap::new(),
            sound_starts: Vec::new(),
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

        // SharedObject built-in object (TSD 2.5, M5.5)
        let so_id = self.heap.alloc_object(None);
        self.heap.set_prop(
            so_id,
            "__builtin__",
            Value::String("SharedObject".to_owned()),
            self.swf_version,
        );
        self.set_global("SharedObject", Value::Object(so_id));
    }

    /// Serializes all `SharedObject` instances into the 64 KiB `"SWFS"` + CRC32 `save_ram` buffer.
    pub fn flush_shared_objects_to_save_ram(&mut self) {
        let mut payload = Vec::new();
        for (so_name, &data_id) in &self.shared_objects {
            for key in self.heap.own_keys(data_id) {
                if key.starts_with("__") {
                    continue;
                }
                let val = self.heap.get_prop(data_id, &key, self.swf_version);
                let line = match val {
                    Value::Number(n) => format!("{so_name}\t{key}\tN\t{n}\n"),
                    Value::Bool(b) => format!("{so_name}\t{key}\tB\t{b}\n"),
                    Value::String(s) => {
                        let escaped = s.replace(['\t', '\n', '\r'], " ");
                        format!("{so_name}\t{key}\tS\t{escaped}\n")
                    }
                    _ => continue,
                };
                if 12_usize
                    .saturating_add(payload.len())
                    .saturating_add(line.len())
                    <= SAVE_RAM_SIZE
                {
                    payload.extend_from_slice(line.as_bytes());
                }
            }
        }
        let crc = compute_crc32(&payload);
        let len_u32 = u32::try_from(payload.len()).unwrap_or(0);
        if self.save_ram.len() < SAVE_RAM_SIZE {
            self.save_ram.resize(SAVE_RAM_SIZE, 0);
        }
        self.save_ram.fill(0);
        self.save_ram[0..4].copy_from_slice(b"SWFS");
        self.save_ram[4..8].copy_from_slice(&crc.to_le_bytes());
        self.save_ram[8..12].copy_from_slice(&len_u32.to_le_bytes());
        let end = 12_usize.saturating_add(payload.len()).min(SAVE_RAM_SIZE);
        self.save_ram[12..end].copy_from_slice(&payload[..end.saturating_sub(12)]);
    }

    /// Loads and verifies a `"SWFS"` + CRC32 `SAVE_RAM` buffer into `shared_objects` (TSD 2.5, M5.5).
    pub fn load_save_ram(&mut self, buf: &[u8]) -> bool {
        if buf.len() < 12 || &buf[0..4] != b"SWFS" {
            return false;
        }
        let expected_crc = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let payload_len = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]) as usize;
        let Some(end) = 12_usize.checked_add(payload_len) else {
            return false;
        };
        if end > buf.len() || end > SAVE_RAM_SIZE {
            return false;
        }
        let payload = &buf[12..end];
        if compute_crc32(payload) != expected_crc {
            return false;
        }
        let Ok(text) = core::str::from_utf8(payload) else {
            return false;
        };
        for line in text.lines() {
            let mut parts = line.splitn(4, '\t');
            let (Some(so_name), Some(key), Some(kind), Some(raw_val)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let data_id = *self
                .shared_objects
                .entry(so_name.to_owned())
                .or_insert_with(|| self.heap.alloc_object(None));
            let parsed = match kind {
                "N" => Value::Number(raw_val.parse::<f64>().unwrap_or(f64::NAN)),
                "B" => Value::Bool(raw_val == "true"),
                "S" => Value::String(raw_val.to_owned()),
                _ => continue,
            };
            self.heap.set_prop(data_id, key, parsed, self.swf_version);
        }
        if self.save_ram.len() < SAVE_RAM_SIZE {
            self.save_ram.resize(SAVE_RAM_SIZE, 0);
        }
        self.save_ram.fill(0);
        let copy_len = buf.len().min(SAVE_RAM_SIZE);
        self.save_ram[..copy_len].copy_from_slice(&buf[..copy_len]);
        true
    }

    /// Sets the key state for `Key.isDown(code)` and `Key.getCode()`.
    ///
    /// Also maps libretro `retro_key` codes (`RETROK_UP = 273`, `RETROK_DOWN = 274`,
    /// `RETROK_RIGHT = 275`, `RETROK_LEFT = 276`, `RETROK_a..=RETROK_z = 97..=122`,
    /// `RETROK_RSHIFT/LSHIFT = 303..=304`, `RETROK_RCTRL/LCTRL = 305..=306`) to their
    /// Adobe Flash `Key.*` virtual keycode equivalents (`38`, `40`, `39`, `37`, `65..=90`,
    /// `16`, `17`) so SWF movies checking either `retro_key` or Flash virtual keycodes work.
    pub fn set_key_state(&mut self, key_code: u32, down: bool) {
        let flash_alias = match key_code {
            273 => Some(38),
            274 => Some(40),
            275 => Some(39),
            276 => Some(37),
            97..=122 => Some(key_code - 32),
            303 | 304 => Some(16),
            305 | 306 => Some(17),
            _ => None,
        };
        if down {
            self.keys_down.insert(key_code);
            if let Some(alias) = flash_alias {
                self.keys_down.insert(alias);
            }
            self.last_key_code = flash_alias.unwrap_or(key_code);
        } else {
            self.keys_down.remove(&key_code);
            if let Some(alias) = flash_alias {
                self.keys_down.remove(&alias);
            }
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

    fn cached_ops(&mut self, bytecode: &[u8]) -> Arc<[Op]> {
        if let Some(ops) = self.ops_cache.get(bytecode) {
            return Arc::clone(ops);
        }
        let ops: Arc<[Op]> = Arc::from(decode_ops(bytecode));
        if self.ops_cache.len() >= 512 {
            self.ops_cache.clear();
        }
        self.ops_cache.insert(bytecode.to_vec(), Arc::clone(&ops));
        ops
    }

    fn current_target(&self) -> String {
        if self.current_target_path.is_empty() {
            "_root".to_owned()
        } else {
            self.current_target_path.clone()
        }
    }

    fn canonical_target(&self, target: &str) -> String {
        Stage::resolve_target_path(&self.current_target(), target)
    }

    fn clip_prop_get(&self, canon: &str, prop: &str) -> Option<Value> {
        let norm = normalize_key(prop, self.swf_version);
        self.clip_properties
            .get(canon)
            .and_then(|m| m.get(&norm).cloned())
    }

    fn clip_prop_set(&mut self, canon: &str, prop: &str, val: Value) {
        let norm = normalize_key(prop, self.swf_version);
        self.clip_properties
            .entry(canon.to_owned())
            .or_default()
            .insert(norm, val);
    }

    fn split_path_variable(&self, name: &str) -> Option<(String, String)> {
        if !name.contains(['.', '/', ':']) {
            return None;
        }
        let normalized = name.replace(':', ".").replace('/', ".");
        let (parent, last) = normalized.rsplit_once('.')?;
        if parent.is_empty() || last.is_empty() {
            return None;
        }
        let canon_parent = Stage::resolve_target_path(&self.current_target(), parent);
        Some((canon_parent, last.to_owned()))
    }

    fn resolve_path_value(&self, name: &str, stage: &Stage) -> Option<Value> {
        let whole = Stage::resolve_target_path(
            &self.current_target(),
            &name.replace(':', ".").replace('/', "."),
        );
        if stage.find_timeline(&whole).is_some() || stage.find_display_object(&whole).is_some() {
            return Some(Value::MovieClip(whole));
        }
        let (parent, last) = self.split_path_variable(name)?;
        let val = self.get_member_value(&Value::MovieClip(parent), &last, stage);
        if matches!(val, Value::Undefined) {
            None
        } else {
            Some(val)
        }
    }

    fn set_path_variable(&mut self, name: &str, val: Value, stage: &mut Stage) -> bool {
        let Some((parent, last)) = self.split_path_variable(name) else {
            return false;
        };
        if stage.find_timeline(&parent).is_none() && stage.find_display_object(&parent).is_none() {
            return false;
        }
        self.set_member_value(&Value::MovieClip(parent), &last, val, stage);
        true
    }

    /// Decodes and executes an AVM1 bytecode slice against `stage` with `_root` as its target.
    pub fn execute_bytecode(&mut self, bytecode: &[u8], stage: &mut Stage) {
        self.execute_bytecode_for_target(bytecode, "_root", stage);
    }

    /// Decodes (cached) and executes an AVM1 bytecode slice as the timeline at `target_path`.
    pub fn execute_bytecode_for_target(
        &mut self,
        bytecode: &[u8],
        target_path: &str,
        stage: &mut Stage,
    ) {
        let ops = self.cached_ops(bytecode);
        self.execute_ops_for_target(&ops, target_path, stage);
    }

    /// Executes a pre-decoded ` &[Op]` slice against `stage` with `_root` as its target.
    pub fn execute_ops(&mut self, ops: &[Op], stage: &mut Stage) {
        self.execute_ops_for_target(ops, "_root", stage);
    }

    /// Executes a pre-decoded ` &[Op]` slice as the timeline at `target_path`.
    pub fn execute_ops_for_target(&mut self, ops: &[Op], target_path: &str, stage: &mut Stage) {
        self.watchdog_tripped = false;
        let previous_target = self.current_target_path.clone();
        self.current_target_path = Stage::resolve_target_path("_root", target_path);
        let mut budget = self.max_ops_per_run;
        let mut locals = BTreeMap::new();
        let _ = self.run_ops_inner(ops, None, &mut locals, stage, &mut budget, 0);
        let mut hops = 0_usize;
        while !self.pending_goto_actions.is_empty() && hops < 16 && !self.watchdog_tripped {
            hops += 1;
            let batch: Vec<(String, Vec<u8>)> = core::mem::take(&mut self.pending_goto_actions);
            for (target, code) in batch {
                let target_ops = self.cached_ops(&code);
                self.current_target_path = Stage::resolve_target_path("_root", &target);
                let mut target_locals = BTreeMap::new();
                let _ = self.run_ops_inner(
                    &target_ops,
                    None,
                    &mut target_locals,
                    stage,
                    &mut budget,
                    0,
                );
                if self.watchdog_tripped {
                    break;
                }
            }
        }
        self.pending_goto_actions.clear();
        self.current_target_path = previous_target;
        if self.heap.allocations_since_gc > 256 {
            let mut roots: Vec<Value> = self.stack.clone();
            roots.extend(self.globals.values().cloned());
            roots.extend(self.registers.iter().cloned());
            roots.extend(self.shared_objects.values().copied().map(Value::Object));
            for props in self.clip_properties.values() {
                roots.extend(props.values().cloned());
            }
            let _ = self.heap.collect_garbage(&roots);
        }
    }

    fn queue_goto_frame(
        &mut self,
        stage: &mut Stage,
        path: &str,
        target_frame: u16,
        play_after: bool,
    ) {
        let canon = self.canonical_target(path);
        let prev_frame = stage
            .find_timeline(&canon)
            .map_or(0, |t| t.current_frame);
        let ev = stage.goto_frame(&canon, target_frame, play_after);
        let new_frame = stage
            .find_timeline(&canon)
            .map_or(prev_frame, |t| t.current_frame);
        if new_frame != prev_frame {
            for (act_path, act) in ev.actions {
                if !act.is_empty() {
                    self.pending_goto_actions.push((act_path, act));
                }
            }
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
                    let target_path = self.current_target();
                    let next = stage
                        .find_timeline(&target_path)
                        .map_or(0, |t| {
                            t.current_frame
                                .saturating_add(1)
                                .min(t.frame_count.saturating_sub(1))
                        });
                    self.queue_goto_frame(stage, &target_path, next, true);
                }
                Op::PrevFrame => {
                    let target_path = self.current_target();
                    let prev = stage
                        .find_timeline(&target_path)
                        .map_or(0, |t| t.current_frame.saturating_sub(1));
                    self.queue_goto_frame(stage, &target_path, prev, false);
                }
                Op::Play => {
                    let target_path = self.current_target();
                    if let Some(tl) = stage.find_timeline_mut(&target_path) {
                        tl.playing = true;
                    }
                }
                Op::Stop => {
                    let target_path = self.current_target();
                    if let Some(tl) = stage.find_timeline_mut(&target_path) {
                        tl.playing = false;
                    }
                }
                Op::StopSounds => {
                    self.stop_sounds_requested = true;
                }
                Op::GotoFrame(f) => {
                    let target_path = self.current_target();
                    self.queue_goto_frame(stage, &target_path, *f, false);
                }
                Op::GotoLabel(label) => {
                    let target_path = self.current_target();
                    if let Some(f) = stage
                        .find_timeline(&target_path)
                        .and_then(|t| t.resolve_label(label))
                    {
                        self.queue_goto_frame(stage, &target_path, f, false);
                    }
                }
                Op::GotoFrame2(play_after) => {
                    let target = self.pop();
                    let target_path = self.current_target();
                    match target {
                        Value::Number(n) if n.is_finite() && (n as i32) >= 1 => {
                            let f = ((n as i32) - 1) as u16;
                            self.queue_goto_frame(stage, &target_path, f, *play_after);
                        }
                        Value::String(s) => {
                            if let Some(f) = stage
                                .find_timeline(&target_path)
                                .and_then(|t| t.resolve_label(&s))
                            {
                                self.queue_goto_frame(stage, &target_path, f, *play_after);
                            } else if let Ok(n) = s.parse::<u16>() {
                                if n >= 1 {
                                    self.queue_goto_frame(stage, &target_path, n - 1, *play_after);
                                }
                            }
                        }
                        Value::MovieClip(p) => {
                            if let Some(f) = stage
                                .find_timeline(&p)
                                .and_then(|t| t.resolve_label(""))
                            {
                                self.queue_goto_frame(stage, &p, f, *play_after);
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
                Op::GetTime => {
                    self.stack.push(Value::Number(self.elapsed_ms as f64));
                }
                Op::CharToAscii => {
                    let s = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let code = s.chars().next().map_or(0.0, |c| f64::from(c as u32));
                    self.stack.push(Value::Number(code));
                }
                Op::AsciiToChar => {
                    let code = self.pop().to_i32(self.swf_version);
                    let s = u32::try_from(code)
                        .ok()
                        .and_then(char::from_u32)
                        .map_or_else(String::new, |c| c.to_string());
                    self.stack.push(Value::String(s));
                }
                Op::TargetPath => {
                    let v = self.pop();
                    let out = match v {
                        Value::MovieClip(p) => {
                            let canon = self.canonical_target(&p);
                            format!(
                                "/{}",
                                canon.strip_prefix("_root.").unwrap_or("").replace('.', "/")
                            )
                        }
                        _ => String::new(),
                    };
                    self.stack.push(Value::String(out));
                }
                Op::GetUrl { url, target } => {
                    if !url.is_empty()
                        && !url.to_ascii_lowercase().starts_with("fscommand:")
                        && !url.to_ascii_lowercase().starts_with("javascript:")
                    {
                        self.load_movie_requests
                            .push((url.clone(), target.clone()));
                    }
                }
                Op::GetUrl2 => {
                    let target = self.pop().to_avm_string(self.swf_version, &self.heap);
                    let url = self.pop().to_avm_string(self.swf_version, &self.heap);
                    if !url.is_empty()
                        && !url.to_ascii_lowercase().starts_with("fscommand:")
                        && !url.to_ascii_lowercase().starts_with("javascript:")
                    {
                        self.load_movie_requests.push((url, target));
                    }
                }
                Op::SetTarget { target, stack_form } => {
                    let raw = if *stack_form {
                        self.pop().to_avm_string(self.swf_version, &self.heap)
                    } else {
                        target.clone()
                    };
                    if !raw.is_empty() {
                        self.current_target_path =
                            Stage::resolve_target_path(&self.current_target(), &raw);
                    }
                }
                Op::WaitForFrame {
                    frame,
                    skip,
                    stack_form,
                } => {
                    let wanted = if *stack_form {
                        match self.pop() {
                            Value::Number(n) if n.is_finite() && n >= 1.0 => (n as u16).saturating_sub(1),
                            Value::String(s) => stage
                                .find_timeline(&self.current_target())
                                .and_then(|t| t.resolve_label(&s))
                                .unwrap_or(0),
                            _ => 0,
                        }
                    } else {
                        *frame
                    };
                    let loaded = stage
                        .find_timeline(&self.current_target())
                        .map_or(0, |t| t.frame_count);
                    if wanted >= loaded {
                        pc = pc.saturating_add(usize::from(*skip));
                    }
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
                    } else if self.set_path_variable(&name, val.clone(), stage) {
                        // Assigned through a target path such as `_root.hero.score` or `/hero:score`.
                    } else {
                        let s_str = val.to_avm_string(self.swf_version, &self.heap);
                        let target_path = self.current_target();
                        stage.sync_variable(&name, &s_str);
                        self.clip_prop_set(&target_path, &name, val.clone());
                        if target_path == "_root" {
                            self.set_global(&name, val);
                        }
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
                    let target = self.pop();
                    let path = match &target {
                        Value::MovieClip(p) => self.canonical_target(p),
                        Value::String(s) => self.canonical_target(s),
                        _ => self.current_target(),
                    };
                    let prop = swf4_property_name(idx);
                    let val = prop.map_or(Value::Undefined, |p| {
                        self.get_member_value(&Value::MovieClip(path.clone()), p, stage)
                    });
                    self.stack.push(val);
                }
                Op::SetProperty => {
                    let val = self.pop();
                    let idx = self.pop().to_i32(self.swf_version);
                    let target = self.pop();
                    let path = match &target {
                        Value::MovieClip(p) => self.canonical_target(p),
                        Value::String(s) => self.canonical_target(s),
                        _ => self.current_target(),
                    };
                    if let Some(prop) = swf4_property_name(idx) {
                        self.set_member_value(&Value::MovieClip(path), prop, val, stage);
                    }
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
                        ops: Arc::from(body_ops.as_slice()),
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
            return this_obj.map_or_else(
                || Value::MovieClip(self.current_target()),
                Value::Object,
            );
        }
        if norm == "_root" || norm == "_level0" {
            return Value::MovieClip("_root".to_owned());
        }
        if norm == "_parent" {
            return Value::MovieClip(parent_clip_path(&self.current_target()));
        }
        if let Some(v) = locals.get(&norm) {
            return v.clone();
        }
        if let Some(v) = self.resolve_path_value(name, stage) {
            return v;
        }
        let target = self.current_target();
        if let Some(v) = self.clip_prop_get(&target, name) {
            return v;
        }
        // A bare name may be a child clip of the current timeline (very common in AS1/AS2).
        if let Some(child) = stage.resolve_child_clip(&target, name) {
            return Value::MovieClip(child);
        }
        if matches!(norm.as_str(), "_currentframe" | "_totalframes" | "_framesloaded") {
            return self.get_member_value(
                &Value::MovieClip(target),
                &norm,
                stage,
            );
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
                let canon = self.canonical_target(path);
                let norm = normalize_key(prop, self.swf_version);
                let timeline = stage.find_timeline(&canon);
                let display = stage.find_display_object(&canon);
                match norm.as_str() {
                    "_root" | "_level0" => return Value::MovieClip("_root".to_owned()),
                    "_parent" => return Value::MovieClip(parent_clip_path(&canon)),
                    "_target" => {
                        return Value::String(format!(
                            "/{}",
                            canon
                                .strip_prefix("_root.")
                                .unwrap_or("")
                                .replace('.', "/")
                        ));
                    }
                    "_name" => {
                        return Value::String(
                            canon
                                .rsplit_once('.')
                                .map_or("", |(_, n)| n)
                                .to_owned(),
                        );
                    }
                    "_currentframe" => {
                        return Value::Number(
                            timeline.map_or(1.0, |t| f64::from(t.current_frame) + 1.0),
                        );
                    }
                    "_totalframes" | "_framesloaded" => {
                        return Value::Number(timeline.map_or(0.0, |t| f64::from(t.frame_count)));
                    }
                    "_x" => return Value::Number(display.map_or(0.0, |o| o.x_px())),
                    "_y" => return Value::Number(display.map_or(0.0, |o| o.y_px())),
                    "_xscale" => return Value::Number(display.map_or(100.0, |o| o.xscale_pct())),
                    "_yscale" => return Value::Number(display.map_or(100.0, |o| o.yscale_pct())),
                    "_rotation" => {
                        return Value::Number(display.map_or(0.0, |o| o.rotation_deg()));
                    }
                    "_alpha" => return Value::Number(display.map_or(100.0, |o| o.alpha_pct())),
                    "_visible" => {
                        return Value::Bool(display.map_or(canon == "_root", |o| o.visible));
                    }
                    "_width" => {
                        return Value::Number(stage.clip_bounds_stage_twips(&canon).map_or(
                            f64::from(stage.width_px),
                            |b| f64::from(b.x_max.saturating_sub(b.x_min)) / 20.0,
                        ));
                    }
                    "_height" => {
                        return Value::Number(stage.clip_bounds_stage_twips(&canon).map_or(
                            f64::from(stage.height_px),
                            |b| f64::from(b.y_max.saturating_sub(b.y_min)) / 20.0,
                        ));
                    }
                    "_xmouse" => {
                        let origin = stage
                            .clip_bounds_stage_twips(&canon)
                            .map_or(0.0, |b| f64::from(b.x_min) / 20.0);
                        return Value::Number(f64::from(stage.pointer_twips.0) / 20.0 - origin);
                    }
                    "_ymouse" => {
                        let origin = stage
                            .clip_bounds_stage_twips(&canon)
                            .map_or(0.0, |b| f64::from(b.y_min) / 20.0);
                        return Value::Number(f64::from(stage.pointer_twips.1) / 20.0 - origin);
                    }
                    "text" => {
                        if let Some(o) = display {
                            if let Some(t) = &o.dynamic_text {
                                return Value::String(t.clone());
                            }
                        }
                        if let Some(o) = display {
                            if let Some(swfr_scene::Character::EditText(e)) =
                                stage.dictionary.get(&o.character_id)
                            {
                                return Value::String(e.initial_text.clone().unwrap_or_default());
                            }
                        }
                    }
                    "_url" => return Value::String(String::new()),
                    "_droptarget" => return Value::String(String::new()),
                    "_focusrect" => return Value::Bool(true),
                    "_highquality" => return Value::Number(1.0),
                    "_quality" => return Value::String("HIGH".to_owned()),
                    "_soundbuftime" => return Value::Number(5.0),
                    _ => {}
                }
                if let Some(v) = self.clip_prop_get(&canon, prop) {
                    return v;
                }
                if let Some(child) = stage.resolve_child_clip(&canon, prop) {
                    return Value::MovieClip(child);
                }
                if canon == "_root" {
                    self.get_global(prop)
                } else {
                    Value::Undefined
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
            Value::MovieClip(path) => {
                let canon = self.canonical_target(path);
                let norm = normalize_key(prop, self.swf_version);
                let mut handled = true;
                match norm.as_str() {
                    "_x" => {
                        if let Some(o) = stage.find_display_object_mut(&canon) {
                            o.set_x_px(val.to_number(self.swf_version));
                        }
                    }
                    "_y" => {
                        if let Some(o) = stage.find_display_object_mut(&canon) {
                            o.set_y_px(val.to_number(self.swf_version));
                        }
                    }
                    "_xscale" => {
                        if let Some(o) = stage.find_display_object_mut(&canon) {
                            o.set_xscale_pct(val.to_number(self.swf_version));
                        }
                    }
                    "_yscale" => {
                        if let Some(o) = stage.find_display_object_mut(&canon) {
                            o.set_yscale_pct(val.to_number(self.swf_version));
                        }
                    }
                    "_rotation" => {
                        if let Some(o) = stage.find_display_object_mut(&canon) {
                            o.set_rotation_deg(val.to_number(self.swf_version));
                        }
                    }
                    "_alpha" => {
                        if let Some(o) = stage.find_display_object_mut(&canon) {
                            o.set_alpha_pct(val.to_number(self.swf_version));
                        }
                    }
                    "_visible" => {
                        if let Some(o) = stage.find_display_object_mut(&canon) {
                            o.visible = val.to_bool(self.swf_version);
                        }
                    }
                    "_name" => {
                        let new_name = val.to_avm_string(self.swf_version, &self.heap);
                        if let Some(o) = stage.find_display_object_mut(&canon) {
                            o.name = Some(new_name);
                        }
                    }
                    "text" => {
                        let s = val.to_avm_string(self.swf_version, &self.heap);
                        let mut direct = false;
                        if let Some(o) = stage.find_display_object_mut(&canon) {
                            o.dynamic_text = Some(s.clone());
                            direct = true;
                        }
                        if !direct {
                            stage.sync_variable(&format!("{canon}.text"), &s);
                        }
                    }
                    "_currentframe" | "_totalframes" | "_framesloaded" | "_width" | "_height"
                    | "_target" | "_parent" | "_root" => {}
                    _ => handled = false,
                }
                if handled {
                    stage.mark_stage_dirty();
                    return;
                }
                let s_str = val.to_avm_string(self.swf_version, &self.heap);
                stage.sync_variable(prop, &s_str);
                self.clip_prop_set(&canon, prop, val.clone());
                if canon == "_root" {
                    self.set_global(prop, val);
                }
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
            "loadmovie" | "loadmovienum" => {
                let url = args.first().map_or_else(String::new, |v| {
                    v.to_avm_string(self.swf_version, &self.heap)
                });
                let target = args.get(1).map_or_else(
                    || "_root".to_owned(),
                    |v| v.to_avm_string(self.swf_version, &self.heap),
                );
                self.load_movie_requests.push((url, target));
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
            let canon = self.canonical_target(path);
            return match norm.as_str() {
                "play" => {
                    if let Some(tl) = stage.find_timeline_mut(&canon) {
                        tl.playing = true;
                    }
                    Value::Undefined
                }
                "stop" => {
                    if let Some(tl) = stage.find_timeline_mut(&canon) {
                        tl.playing = false;
                    }
                    Value::Undefined
                }
                "gotoandstop" | "gotoandplay" => {
                    let play_after = norm == "gotoandplay";
                    if let Some(arg) = args.first() {
                        match arg {
                            Value::Number(n) if n.is_finite() && (*n as i32) >= 1 => {
                                let f = ((*n as i32) - 1) as u16;
                                self.queue_goto_frame(stage, &canon, f, play_after);
                            }
                            Value::String(lbl) => {
                                if let Some(f) = stage
                                    .find_timeline(&canon)
                                    .and_then(|t| t.resolve_label(lbl))
                                {
                                    self.queue_goto_frame(stage, &canon, f, play_after);
                                } else if let Ok(n) = lbl.parse::<u16>() {
                                    if n >= 1 {
                                        self.queue_goto_frame(stage, &canon, n - 1, play_after);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Value::Undefined
                }
                "nextframe" => {
                    let next = stage.find_timeline(&canon).map_or(0, |t| {
                        t.current_frame
                            .saturating_add(1)
                            .min(t.frame_count.saturating_sub(1))
                    });
                    self.queue_goto_frame(stage, &canon, next, false);
                    Value::Undefined
                }
                "prevframe" => {
                    let prev = stage
                        .find_timeline(&canon)
                        .map_or(0, |t| t.current_frame.saturating_sub(1));
                    self.queue_goto_frame(stage, &canon, prev, false);
                    Value::Undefined
                }
                "createemptymovieclip" => {
                    let name = args.first().map_or_else(String::new, |v| {
                        v.to_avm_string(self.swf_version, &self.heap)
                    });
                    let depth = args
                        .get(1)
                        .map_or(1, |v| v.to_i32(self.swf_version).clamp(0, 65535) as u16);
                    let child = stage.create_empty_movie_clip(&canon, &name, depth);
                    Value::MovieClip(child)
                }
                "attachmovie" => {
                    let id_name = args.first().map_or_else(String::new, |v| {
                        v.to_avm_string(self.swf_version, &self.heap)
                    });
                    let new_name = args.get(1).map_or_else(String::new, |v| {
                        v.to_avm_string(self.swf_version, &self.heap)
                    });
                    let depth = args
                        .get(2)
                        .map_or(1, |v| v.to_i32(self.swf_version).clamp(0, 65535) as u16);
                    match stage.attach_movie(&canon, &id_name, &new_name, depth) {
                        Some((child, ev)) => {
                            for (t, code) in ev.actions {
                                self.execute_bytecode_for_target(&code, &t, stage);
                            }
                            Value::MovieClip(child)
                        }
                        None => Value::Undefined,
                    }
                }
                "duplicatemovieclip" => {
                    let new_name = args.first().map_or_else(String::new, |v| {
                        v.to_avm_string(self.swf_version, &self.heap)
                    });
                    let depth = args
                        .get(1)
                        .map_or(1, |v| v.to_i32(self.swf_version).clamp(0, 65535) as u16);
                    stage
                        .duplicate_movie_clip(&canon, &new_name, depth)
                        .map_or(Value::Undefined, Value::MovieClip)
                }
                "removemovieclip" => {
                    let _ = stage.remove_movie_clip(&canon);
                    Value::Undefined
                }
                "unloadmovie" => {
                    if canon != "_root" {
                        let _ = stage.remove_movie_clip(&canon);
                    }
                    Value::Undefined
                }
                "swapdepths" => {
                    let depth = match args.first() {
                        Some(Value::MovieClip(other)) => stage
                            .find_display_object(&self.canonical_target(other))
                            .map_or(0, |o| o.depth),
                        Some(v) => v.to_i32(self.swf_version).clamp(0, 65535) as u16,
                        None => 0,
                    };
                    let _ = stage.swap_depths(&canon, depth);
                    Value::Undefined
                }
                "getnexthighestdepth" => {
                    Value::Number(f64::from(stage.get_next_highest_depth(&canon)))
                }
                "getbytesloaded" | "getbytestotal" => {
                    Value::Number(f64::from(stage.swf_byte_len))
                }
                "hittest" => {
                    let shape_flag = args
                        .get(2)
                        .map_or(false, |v| v.to_bool(self.swf_version));
                    match args.first() {
                        Some(Value::MovieClip(other)) => {
                            let other_canon = self.canonical_target(other);
                            let a = stage.clip_bounds_stage_twips(&canon);
                            let b = stage.clip_bounds_stage_twips(&other_canon);
                            match (a, b) {
                                (Some(a), Some(b)) => Value::Bool(
                                    a.x_min <= b.x_max
                                        && b.x_min <= a.x_max
                                        && a.y_min <= b.y_max
                                        && b.y_min <= a.y_max,
                                ),
                                _ => Value::Bool(false),
                            }
                        }
                        Some(xv) => {
                            let x = (xv.to_number(self.swf_version) * 20.0).round() as i32;
                            let y = args.get(1).map_or(0, |v| {
                                (v.to_number(self.swf_version) * 20.0).round() as i32
                            });
                            Value::Bool(stage.hit_test_point(&canon, x, y, shape_flag))
                        }
                        None => Value::Bool(false),
                    }
                }
                "loadmovie" => {
                    let url = args.first().map_or_else(String::new, |v| {
                        v.to_avm_string(self.swf_version, &self.heap)
                    });
                    self.load_movie_requests.push((url, canon.clone()));
                    Value::Undefined
                }
                "geturl" => {
                    let url = args.first().map_or_else(String::new, |v| {
                        v.to_avm_string(self.swf_version, &self.heap)
                    });
                    if !url.is_empty() {
                        self.load_movie_requests.push((url, canon.clone()));
                    }
                    Value::Undefined
                }
                _ => {
                    if let Some(Value::Object(fid)) = self.clip_prop_get(&canon, mname) {
                        let previous = self.current_target_path.clone();
                        self.current_target_path = canon.clone();
                        let ret = self
                            .invoke_user_function(fid, None, args, stage, budget)
                            .unwrap_or(Value::Undefined);
                        self.current_target_path = previous;
                        ret
                    } else {
                        Value::Undefined
                    }
                }
            };
        }

        // 3. Heap Objects (Built-in Math, Key, Sound, SharedObject, Array, or user objects)
        let Value::Object(obj_id) = *receiver else {
            return Value::Undefined;
        };

        if let Value::String(btag) = self.heap.get_prop(obj_id, "__builtin__", self.swf_version) {
            if btag == "SharedObject" && norm == "getlocal" {
                let name = args.first().map_or_else(
                    || "default".to_owned(),
                    |v| v.to_avm_string(self.swf_version, &self.heap),
                );
                let data_id = *self
                    .shared_objects
                    .entry(name)
                    .or_insert_with(|| self.heap.alloc_object(None));
                let inst_id = self.heap.alloc_object(None);
                self.heap.set_prop(
                    inst_id,
                    "__builtin__",
                    Value::String("SharedObjectInstance".to_owned()),
                    self.swf_version,
                );
                self.heap
                    .set_prop(inst_id, "data", Value::Object(data_id), self.swf_version);
                return Value::Object(inst_id);
            }
            if btag == "SharedObjectInstance" {
                return match norm.as_str() {
                    "flush" => {
                        self.flush_shared_objects_to_save_ram();
                        Value::Bool(true)
                    }
                    "clear" => {
                        if let Value::Object(data_id) =
                            self.heap.get_prop(obj_id, "data", self.swf_version)
                        {
                            for k in self.heap.own_keys(data_id) {
                                let _ = self.heap.delete_prop(data_id, &k, self.swf_version);
                            }
                        }
                        self.flush_shared_objects_to_save_ram();
                        Value::Undefined
                    }
                    _ => Value::Undefined,
                };
            }
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
            if btag == "Color" {
                let target_path = match self.heap.get_prop(obj_id, "__target", self.swf_version) {
                    Value::MovieClip(p) => self.canonical_target(&p),
                    Value::String(s) => self.canonical_target(&s),
                    _ => self.current_target(),
                };
                return match norm.as_str() {
                    "setrgb" => {
                        let rgb = args
                            .first()
                            .map_or(0, |v| v.to_i32(self.swf_version))
                            .cast_unsigned();
                        if let Some(o) = stage.find_display_object_mut(&target_path) {
                            o.color_transform.r_mult = 0;
                            o.color_transform.g_mult = 0;
                            o.color_transform.b_mult = 0;
                            o.color_transform.a_mult = 256;
                            o.color_transform.r_add = ((rgb >> 16) & 0xFF) as i16;
                            o.color_transform.g_add = ((rgb >> 8) & 0xFF) as i16;
                            o.color_transform.b_add = (rgb & 0xFF) as i16;
                            o.color_transform.a_add = 0;
                        }
                        stage.mark_stage_dirty();
                        Value::Undefined
                    }
                    "getrgb" => {
                        let val = stage.find_display_object(&target_path).map_or(0, |o| {
                            let cx = o.color_transform;
                            let chan = |mult: i16, add: i16| -> i32 {
                                (((255 * i32::from(mult)) >> 8) + i32::from(add)).clamp(0, 255)
                            };
                            let r = chan(cx.r_mult, cx.r_add);
                            let g = chan(cx.g_mult, cx.g_add);
                            let b = chan(cx.b_mult, cx.b_add);
                            (r << 16) | (g << 8) | b
                        });
                        Value::Number(f64::from(val))
                    }
                    "settransform" => {
                        if let Some(Value::Object(cx_id)) = args.first() {
                            let getp = |k: &str| {
                                self.heap
                                    .get_prop(*cx_id, k, self.swf_version)
                                    .to_number(self.swf_version)
                            };
                            if let Some(o) = stage.find_display_object_mut(&target_path) {
                                o.color_transform.r_mult = (getp("ra") * 2.56).round() as i16;
                                o.color_transform.g_mult = (getp("ga") * 2.56).round() as i16;
                                o.color_transform.b_mult = (getp("ba") * 2.56).round() as i16;
                                o.color_transform.a_mult = (getp("aa") * 2.56).round() as i16;
                                o.color_transform.r_add = getp("rb").round() as i16;
                                o.color_transform.g_add = getp("gb").round() as i16;
                                o.color_transform.b_add = getp("bb").round() as i16;
                                o.color_transform.a_add = getp("ab").round() as i16;
                            }
                            stage.mark_stage_dirty();
                        }
                        Value::Undefined
                    }
                    "gettransform" => {
                        let out = self.heap.alloc_object(None);
                        if let Some(o) = stage.find_display_object(&target_path) {
                            let cx = o.color_transform;
                            for (k, v) in [
                                ("ra", f64::from(cx.r_mult) / 2.56),
                                ("ga", f64::from(cx.g_mult) / 2.56),
                                ("ba", f64::from(cx.b_mult) / 2.56),
                                ("aa", f64::from(cx.a_mult) / 2.56),
                                ("rb", f64::from(cx.r_add)),
                                ("gb", f64::from(cx.g_add)),
                                ("bb", f64::from(cx.b_add)),
                                ("ab", f64::from(cx.a_add)),
                            ] {
                                self.heap.set_prop(out, k, Value::Number(v), self.swf_version);
                            }
                        }
                        Value::Object(out)
                    }
                    _ => Value::Undefined,
                };
            }
            if btag == "Sound" {
                return match norm.as_str() {
                    "attachsound" => {
                        let linkage = args.first().map_or_else(String::new, |v| {
                            v.to_avm_string(self.swf_version, &self.heap)
                        });
                        if let Some(cid) = stage
                            .exports
                            .get(&linkage)
                            .or_else(|| stage.exports.get(&linkage.to_ascii_lowercase()))
                            .copied()
                        {
                            self.heap.set_prop(
                                obj_id,
                                "__sound_id",
                                Value::Number(f64::from(cid)),
                                self.swf_version,
                            );
                        }
                        self.heap.set_prop(
                            obj_id,
                            "__sound_name",
                            Value::String(linkage),
                            self.swf_version,
                        );
                        Value::Undefined
                    }
                    "start" => {
                        if let Value::Number(idn) =
                            self.heap.get_prop(obj_id, "__sound_id", self.swf_version)
                        {
                            let offset_secs = args
                                .first()
                                .map_or(0.0, |v| v.to_number(self.swf_version))
                                .max(0.0);
                            let loops = args.get(1).map_or(1, |v| {
                                v.to_i32(self.swf_version).clamp(1, 65535) as u16
                            });
                            self.sound_starts.push((
                                idn as u16,
                                SoundInfo {
                                    in_point: Some((offset_secs * 44100.0).round() as u32),
                                    loop_count: Some(loops),
                                    ..SoundInfo::default()
                                },
                            ));
                        }
                        Value::Undefined
                    }
                    "setvolume" => {
                        self.sound_volume = args
                            .first()
                            .map_or(100, |v| v.to_i32(self.swf_version).clamp(0, 100));
                        Value::Undefined
                    }
                    "getvolume" => Value::Number(f64::from(self.sound_volume)),
                    "setpan" | "settransform" => Value::Undefined,
                    "getpan" | "gettransform" => Value::Number(0.0),
                    "getbytesloaded" | "getbytestotal" => {
                        Value::Number(f64::from(stage.swf_byte_len))
                    }
                    "stop" => {
                        if let Value::Number(idn) =
                            self.heap.get_prop(obj_id, "__sound_id", self.swf_version)
                        {
                            self.sound_starts.push((
                                idn as u16,
                                SoundInfo {
                                    sync_stop: true,
                                    ..SoundInfo::default()
                                },
                            ));
                        } else {
                            self.stop_sounds_requested = true;
                        }
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
            "color" => {
                let cid = self.heap.alloc_object(None);
                self.heap.set_prop(
                    cid,
                    "__builtin__",
                    Value::String("Color".to_owned()),
                    self.swf_version,
                );
                let target = match args.first() {
                    Some(Value::MovieClip(p)) => Value::MovieClip(self.canonical_target(p)),
                    Some(Value::String(s)) => Value::MovieClip(self.canonical_target(s)),
                    _ => Value::MovieClip(self.current_target()),
                };
                self.heap.set_prop(cid, "__target", target, self.swf_version);
                Value::Object(cid)
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
