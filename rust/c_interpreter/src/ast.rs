use std::collections::HashMap;

use crate::source::Span;
use crate::types::{CType, EnumType, RecordType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageClass {
    Auto,
    Register,
    Static,
    Extern,
}

#[derive(Debug, Clone)]
pub struct TranslationUnit {
    pub externals: Vec<ExternalDeclaration>,
    pub functions: Vec<FunctionDef>,
    pub function_declarations: Vec<FunctionDecl>,
    pub globals: Vec<Declaration>,
    pub global_definitions: Vec<Declaration>,
    pub records: HashMap<usize, RecordType>,
    pub enums: HashMap<usize, EnumType>,
    pub enum_constants: HashMap<String, i128>,
}

#[derive(Debug, Clone)]
pub enum ExternalDeclaration {
    Function(FunctionDef),
    FunctionDeclaration(FunctionDecl),
    ObjectDeclaration(Declaration),
}

#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: String,
    pub return_type: CType,
    pub params: Vec<Parameter>,
    pub is_variadic: bool,
    pub storage_class: Option<StorageClass>,
    pub is_inline: bool,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: String,
    pub return_type: CType,
    pub params: Vec<Parameter>,
    pub is_variadic: bool,
    pub storage_class: Option<StorageClass>,
    pub is_inline: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: Option<String>,
    pub ty: CType,
    pub vla_bounds: Vec<Option<Expr>>,
    pub static_array_bound: Option<Expr>,
    pub storage_class: Option<StorageClass>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub items: Vec<BlockItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum BlockItem {
    Declaration(Declaration),
    FunctionDeclaration(FunctionDecl),
    Statement(Statement),
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub name: String,
    pub ty: CType,
    pub vla_bounds: Vec<Option<Expr>>,
    pub storage_class: Option<StorageClass>,
    pub init: Option<Initializer>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Initializer {
    Expr(Expr),
    List {
        items: Vec<InitializerItem>,
        span: Span,
    },
}

impl Initializer {
    pub fn span(&self) -> Span {
        match self {
            Initializer::Expr(expr) => expr.span(),
            Initializer::List { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InitializerItem {
    pub designators: Vec<Designator>,
    pub initializer: Initializer,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Designator {
    Member(String, Span),
    Index(usize, Span),
}

#[derive(Debug, Clone)]
pub enum Statement {
    Block(Block),
    Break(Span),
    Continue(Span),
    DoWhile {
        body: Box<Statement>,
        condition: Expr,
        span: Span,
    },
    Expression(Option<Expr>, Span),
    For {
        init: Option<ForInit>,
        condition: Option<Expr>,
        step: Option<Expr>,
        body: Box<Statement>,
        span: Span,
    },
    Goto {
        label: String,
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Box<Statement>,
        else_branch: Option<Box<Statement>>,
        span: Span,
    },
    Labeled {
        label: SwitchLabel,
        statement: Box<Statement>,
        span: Span,
    },
    Return(Option<Expr>, Span),
    Switch {
        expr: Expr,
        body: Block,
        span: Span,
    },
    UserLabeled {
        label: String,
        statement: Box<Statement>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Box<Statement>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum SwitchLabel {
    Case { expr: Expr, span: Span },
    Default { span: Span },
}

#[derive(Debug, Clone)]
pub enum ForInit {
    Declarations(Vec<Declaration>),
    Expression(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(String, Span),
    CharLiteral(i64, Span),
    WideCharLiteral(i64, Span),
    StringLiteral(String, Span),
    WideStringLiteral(String, Span),
    Variable(String, Span),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Postfix {
        op: PostfixOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Assign {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    CompoundAssign {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    SizeofType {
        ty: CType,
        vla_bounds: Vec<Option<Expr>>,
        span: Span,
    },
    SizeofExpr {
        expr: Box<Expr>,
        span: Span,
    },
    OffsetOf {
        ty: CType,
        designators: Vec<Designator>,
        span: Span,
    },
    Cast {
        ty: CType,
        vla_bounds: Vec<Option<Expr>>,
        expr: Box<Expr>,
        span: Span,
    },
    CompoundLiteral {
        ty: CType,
        vla_bounds: Vec<Option<Expr>>,
        initializer: Box<Initializer>,
        span: Span,
    },
    GenericSelection {
        control: Box<Expr>,
        associations: Vec<GenericAssociation>,
        default: Option<Box<Expr>>,
        span: Span,
    },
    VaArg {
        ap: Box<Expr>,
        ty: CType,
        span: Span,
    },
    Conditional {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Member {
        base: Box<Expr>,
        member: String,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct GenericAssociation {
    pub ty: CType,
    pub expr: Expr,
    pub span: Span,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Number(_, span)
            | Expr::CharLiteral(_, span)
            | Expr::WideCharLiteral(_, span)
            | Expr::StringLiteral(_, span)
            | Expr::WideStringLiteral(_, span)
            | Expr::Variable(_, span)
            | Expr::Unary { span, .. }
            | Expr::Postfix { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Assign { span, .. }
            | Expr::CompoundAssign { span, .. }
            | Expr::SizeofType { span, .. }
            | Expr::SizeofExpr { span, .. }
            | Expr::OffsetOf { span, .. }
            | Expr::Cast { span, .. }
            | Expr::CompoundLiteral { span, .. }
            | Expr::GenericSelection { span, .. }
            | Expr::VaArg { span, .. }
            | Expr::Conditional { span, .. }
            | Expr::Call { span, .. }
            | Expr::Member { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    AddressOf,
    Dereference,
    Plus,
    Minus,
    LogicalNot,
    BitNot,
    PreIncrement,
    PreDecrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    ShiftLeft,
    ShiftRight,
    BitAnd,
    BitXor,
    BitOr,
    LogicalAnd,
    LogicalOr,
    Comma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostfixOp {
    PostIncrement,
    PostDecrement,
}
