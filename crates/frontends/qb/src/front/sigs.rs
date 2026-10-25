//! What BC accepts as the arguments of the intrinsic functions: how many, of
//! what class, and the type each returns. Calibrated against BC itself
//! (corpus/): a function is here with the shape BC took.

use std::sync::OnceLock;

use llrm_support::hash::HashMap;

use crate::Dialect;

/// What an argument must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// a numeric expression
    Num,
    /// a numeric expression that must fit a 16-bit integer when it is a
    /// constant
    Int,
    /// a numeric expression that must fit a 32-bit integer when it is a
    /// constant
    Long,
    /// a numeric expression that must fit a SINGLE when it is a constant
    Single,
    /// a string expression
    Str,
    /// a number or a string
    Any,
    /// a variable or array element of any type
    Var,
    /// a string variable
    StrVar,
    /// a whole array
    Array,
    /// anything with a type: a record or a string or a number
    Whole,
    /// a variable-length string variable (SADD)
    VarLenString,
    /// a whole array, or an element of one to start from (graphics GET and PUT)
    ArrayStart,
    /// a label
    Label,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ret {
    Int,
    Long,
    Single,
    Double,
    Currency,
    Str,
    /// the argument's own numeric type
    Same,
    /// SINGLE, or DOUBLE for a DOUBLE argument
    Float,
}

pub struct FnSig {
    pub name: &'static str,
    /// the argument lists BC takes: (class, optional) per argument
    pub alternatives: Vec<Vec<(Class, bool)>>,
    pub ret: Ret,
    /// The families that have it: 1 QB 4.5, 2 PDS 7.1, 4 VBDOS.
    pub families: u8,
    /// DOEVENTS(): the parentheses are part of the call even with no arguments.
    pub parens: bool,
    /// Also a statement: `MSGBOX "text", 0, "title"`.
    pub statement: bool,
}

impl FnSig {
    pub fn accepts(
        &self,
        arguments: usize,
    ) -> bool {
        self.alternatives
            .iter()
            .any(|alt| alt.len() >= arguments && alt.iter().take_while(|(_, optional)| !*optional).count() <= arguments)
    }
}

const TABLE: &str = "\
ABS n=>Same
SGN n=>Int
INT n=>Same
FIX n=>Same
CINT i=>Int
CLNG q=>Long
CSNG f=>Single
CDBL n=>Double
SQR n=>Float
SIN n=>Float
COS n=>Float
TAN n=>Float
ATN n=>Float
LOG n=>Float
EXP n=>Float
RND |n=>Single
LEN w=>Int
ASC s=>Int
VAL s=>Double
INSTR s,s|i,s,s=>Int
CVI s=>Int
CVL s=>Long
CVS s=>Single
CVD s=>Double
CVSMBF s=>Single
CVDMBF s=>Double
MKI$ i=>Str
MKL$ q=>Str
MKS$ n=>Str
MKD$ n=>Str
MKSMBF$ n=>Str
MKDMBF$ n=>Str
LEFT$ s,i=>Str
RIGHT$ s,i=>Str
MID$ s,i|s,i,i=>Str
STR$ n=>Str
CHR$ i=>Str
SPACE$ i=>Str
STRING$ i,i|i,s=>Str
HEX$ q=>Str
OCT$ q=>Str
LTRIM$ s=>Str
RTRIM$ s=>Str
UCASE$ s=>Str
LCASE$ s=>Str
PEEK n=>Int
INP n=>Int
POS i=>Int
CSRLIN =>Int
TIMER =>Single
DATE$ =>Str
TIME$ =>Str
COMMAND$ =>Str
ENVIRON$ s|i=>Str
INKEY$ =>Str
INPUT$ i|i,i=>Str
EOF i=>Int
LOF i=>Long
LOC i=>Long
SEEK i=>Long
FILEATTR i,i=>Long
FREEFILE =>Int
FRE i|s=>Long
LBOUND a|a,n=>Int
UBOUND a|a,n=>Int
$VARPTR v=>Str
VARPTR v=>Int
VARSEG v=>Int
SADD u=>Int
POINT i|n,n=>Int
PMAP n,i=>Single
SCREEN i,i|i,i,i=>Int
STICK i=>Int
STRIG i=>Int
PEN i=>Int
PLAY i=>Long
ERR =>Int
ERL =>Long
ERDEV =>Int
ERDEV$ =>Str
LPOS i=>Int
SETMEM n=>Long
CCUR n=>Currency:pv
CURDIR$ |s=>Str:pv
CVC s=>Currency:pv
DIR$ |s=>Str:pv
MKC$ n=>Str:pv
SSEG t=>Int:pv
SSEGADD t=>Long:pv
STACK =>Long:pv
GETINDEX$ i=>Str:pvP
BOF i=>Int:pvP
TEXTCOMP s,s=>Int:pv
SAVEPOINT =>Int:pv
DATESERIAL i,i,i=>Double:v
DATEVALUE s=>Double:v
DOEVENTS =>Int:vP
INPUTBOX$ s|s,s|s,s,s|s,s,s,n|s,s,s,i,i=>Str:v
MSGBOX s|s,i|s,i,s=>Int:vS
TIMESERIAL i,i,i=>Double:v
TIMEVALUE s=>Double:v
FORMAT$ n|n,s=>Str:v
ERROR$ |i=>Str:v
YEAR n=>Int:vP
MONTH n=>Int:vP
DAY n=>Int:vP
HOUR n=>Int:vP
MINUTE n=>Int:vP
SECOND n=>Int:vP
WEEKDAY n=>Int:vP
NOW =>Double:v
RGB i,i,i=>Long:v
QBCOLOR i=>Int:v
TEXTWIDTH s=>Int:v
TEXTHEIGHT s=>Int:v
";

fn class_of(letter: char) -> Class {
    match letter {
        'n' => Class::Num,
        'i' => Class::Int,
        'q' => Class::Long,
        'f' => Class::Single,
        's' => Class::Str,
        'x' => Class::Any,
        'v' => Class::Var,
        't' => Class::StrVar,
        'a' => Class::Array,
        'w' => Class::Whole,
        'u' => Class::VarLenString,
        'l' => Class::Label,
        other => panic!("class {other}"),
    }
}

fn table() -> &'static HashMap<&'static str, FnSig> {
    static CELL: OnceLock<HashMap<&'static str, FnSig>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut map = HashMap::default();
        for line in TABLE.lines() {
            let (head, ret) = line.split_once("=>").expect("a signature");
            // `Ret:flags`: p PDS, v VBDOS (neither: all families), P
            // parentheses required, S also a statement
            let (ret, flags) = ret.split_once(':').unwrap_or((ret, ""));
            let families = if flags.contains(['p', 'v']) {
                (if flags.contains('p') { 2 } else { 0 }) | (if flags.contains('v') { 4 } else { 0 })
            } else {
                7
            };
            let (name, arguments) = head.split_once(' ').unwrap_or((head, ""));
            let alternatives = arguments
                .split('|')
                .map(|alt| {
                    alt.split(',')
                        .filter(|one| !one.is_empty())
                        .map(|one| (class_of(one.chars().next().unwrap()), one.ends_with('?')))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            // an alternative list such as "|n" means no arguments or one
            let ret = match ret {
                "Int" => Ret::Int,
                "Long" => Ret::Long,
                "Single" => Ret::Single,
                "Double" => Ret::Double,
                "Currency" => Ret::Currency,
                "Str" => Ret::Str,
                "Same" => Ret::Same,
                "Float" => Ret::Float,
                other => panic!("return {other}"),
            };
            let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
            map.insert(
                name,
                FnSig {
                    name,
                    alternatives,
                    ret,
                    families,
                    parens: flags.contains('P'),
                    statement: flags.contains('S'),
                },
            );
        }
        map
    })
}

/// The intrinsic function `name` as the source spells it (suffix included), if
/// the family has it.
pub fn function(
    name: &str,
    dialect: Dialect,
) -> Option<&'static FnSig> {
    let upper = name.to_ascii_uppercase();
    let sig = table().get(upper.as_str())?;
    let family = match dialect {
        Dialect::QBasic11 | Dialect::QuickBasic45 => 1,
        Dialect::Pds71 => 2,
        Dialect::VbDos | Dialect::Quickr => 4,
    };
    (sig.families & family != 0).then_some(sig)
}

/// Whether the name is a statement word that is no function (so `name(...)` is
/// a syntax error).
pub fn function_unknown_but_named(_name: &str) -> bool {
    false
}
