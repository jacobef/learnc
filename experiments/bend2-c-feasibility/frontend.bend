import Base

# A bounded ASCII C-subset experiment, not a validating C parser.
type Files is Type:
  Source{bytes: List<U32>}
  Branch{left: Files, right: Files}

type Token is Data:
  Ident{hash: U32}
  Number{value: U32}
  Punct{code: U32}

type LexState is Type:
  LS{mode: U32, value: U32, out: List<Token>}

def digit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

def letter(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122))), U32.is_eq(c, 95))

def emit(mode: U32, v: U32, out: List<Token>) -> List<Token>:
  match mode:
    case 1:
      Ident{v} <> out
    case 2:
      Number{v} <> out
    case other:
      out

def lex.num(mode: U32, v: U32, out: List<Token>, +c: U32) -> LexState:
  match mode:
    case 1:
      LS{1, U32.add(U32.mul(v, 33), c), out}
    case 2:
      LS{2, U32.add(U32.mul(v, 10), U32.sub(c, 48)), out}
    case other:
      LS{2, U32.sub(c, 48), out}

def lex.word(mode: U32, v: U32, out: List<Token>, c: U32) -> LexState:
  match mode:
    case 1:
      LS{1, U32.add(U32.mul(v, 33), c), out}
    case 2:
      LS{0, 0, Number{v} <> out} # generated literals have exactly one 'u'
    case other:
      LS{1, c, out}

def lex.punct(out: List<Token>, c: U32, space: Bool) -> LexState:
  match space:
    case True{}:
      LS{0, 0, out}
    case False{}:
      LS{0, 0, Punct{c} <> out}

def lex.step(mode: U32, v: U32, out: List<Token>, +c: U32, d: Bool, l: Bool) -> LexState:
  match d l:
    case True{} other:
      lex.num(mode, v, out, c)
    case False{} True{}:
      lex.word(mode, v, out, c)
    case False{} False{}:
      lex.punct(emit(mode, v, out), c, U32.is_le(c, 32))

# Two-pass tokenization preserves a real intermediate representation.
def lex(xs: List<U32>, state: LexState) -> List<Token>:
  match xs state:
    case Nil{} LS{mode, v, out}:
      List.reverse(&1, Token, emit(mode, v, out))
    case Con{+c, tail} LS{mode, v, out}:
      lex(tail, lex.step(mode, v, out, c, digit(c), letter(c)))

def tokcode(t: Token) -> U32:
  match t:
    case Ident{h}:
      U32.xor(h, 268435456)
    case Number{v}:
      U32.xor(v, 536870912)
    case Punct{c}:
      c

def digest(ts: List<Token>, h: U32) -> U32:
  match ts:
    case Nil{}:
      h
    case Con{t, tail}:
      digest(tail, U32.add(U32.mul(h, 16777619), tokcode(t)))

type Frame is Type:
  Frame{left: U32, op: U32}

type FoldState is Type:
  FS{active: Bool, acc: U32, op: U32, stack: List<Frame>, sum: U32}

def apply(op: U32, a: U32, b: U32) -> U32:
  match op:
    case 43:
      U32.add(a, b)
    case 42:
      U32.mul(a, b)
    case 94:
      U32.xor(a, b)
    case other:
      b

def fold.pop(stack: List<Frame>, acc: U32, sum: U32) -> FoldState:
  match stack:
    case Nil{}:
      FS{True{}, acc, 0, Nil{}, sum}
    case Con{Frame{a, op}, tail}:
      FS{True{}, apply(op, a, acc), 0, tail, sum}

def fold.punct(c: U32, acc: U32, op: U32, stack: List<Frame>, sum: U32) -> FoldState:
  match c:
    case 40:
      FS{True{}, 0, 0, Frame{acc, op} <> stack, sum}
    case 41:
      fold.pop(stack, acc, sum)
    case 59:
      FS{False{}, 0, 0, Nil{}, U32.add(sum, acc)}
    case other:
      FS{True{}, acc, other, stack, sum}

def fold.step(t: Token, state: FoldState) -> FoldState:
  match t state:
    case Ident{h} FS{active, acc, op, stack, sum}:
      FS{Bool.or(active, U32.is_eq(h, 290545504)), acc, op, stack, sum}
    case Number{v} FS{active, acc, op, stack, sum}:
      FS{active, apply(op, acc, v), 0, stack, sum}
    case Punct{c} FS{True{}, acc, op, stack, sum}:
      fold.punct(c, acc, op, stack, sum)
    case Punct{c} FS{False{}, acc, op, stack, sum}:
      FS{False{}, acc, op, stack, sum}

def fold.out(s: FoldState) -> U32:
  match s:
    case FS{active, acc, op, stack, sum}:
      sum

def fold(ts: List<Token>, state: FoldState) -> U32:
  match ts:
    case Nil{}:
      fold.out(state)
    case Con{t, tail}:
      fold(tail, fold.step(t, state))

def file.lex(xs: List<U32>) -> U32:
  digest(lex(xs, LS{0, 0, Nil{}}), 0)

def file.fold(xs: List<U32>) -> U32:
  fold(lex(xs, LS{0, 0, Nil{}}), FS{False{}, 0, 0, Nil{}, 0})

def run(~f: List<U32> -> U32, files: Files) -> U32:
  match files:
    case Source{xs}:
      f(xs)
    case Branch{l, r}:
      a b = run(~f, l) run(~f, r)
      U32.add(a, b)

def load() -> IO(Files):
  import "./loader.c"

def stamp() -> IO(Nat):
  import "./loader.c"
