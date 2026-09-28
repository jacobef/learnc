#!/usr/bin/env python3
"""Reproducible Bend 2 feasibility probes; standard-library-only harness."""
import argparse, concurrent.futures, hashlib, json, os, pathlib, random, re, statistics, struct, subprocess, time
ROOT = pathlib.Path(__file__).resolve().parent
WORK = ROOT / '.work'
RESULTS = ROOT / 'results'
MASK = 2**32-1
PIN = 'e6676b080f25b1bc1bf5b5b7d7a17e22f8022599'

def command(args, **kw):
    try:
        return subprocess.run([str(x) for x in args], check=True, capture_output=True, text=True, **kw)
    except subprocess.CalledProcessError as error:
        raise RuntimeError(f"Command failed: {args}\n{error.stdout}\n{error.stderr}") from error

def array_source(base):
    base=base.replace('Source{bytes: List<U32>}', 'Source{bytes: Array<U32>, length: Nat}')
    start=base.index('def lex(xs:');end=base.index('def tokcode',start)
    base=base[:start]+'def lex(n: Nat, av: Array<U32> & U32, +index: U32, state: LexState) -> List<Token>:\n  match n av state:\n    case 0n other LS{mode, v, out}:\n      List.reverse(&1, Token, emit(mode, v, out))\n    case 1n+p Tuple{xs, +c} LS{mode, v, out}:\n      lex(p, xs[index], U32.inc(index), lex.step(mode, v, out, c, digit(c), letter(c)))\n\n'+base[end:]
    base=base.replace('def file.lex(xs: List<U32>)','def file.lex(xs: Array<U32>, n: Nat)').replace('def file.fold(xs: List<U32>)','def file.fold(xs: Array<U32>, n: Nat)')
    base=base.replace('lex(xs, LS{','lex(n, xs[0], 1, LS{').replace('~f: List<U32> -> U32','~f: Array<U32> -> Nat -> U32').replace('case Source{xs}:\n      f(xs)','case Source{xs, n}:\n      f(xs, n)')
    return base.replace('../loader.c','../array-loader.c')

def build(toolchain,arrays=False):
    WORK.mkdir(exist_ok=True)
    assert command(['git','-C',toolchain,'rev-parse','HEAD']).stdout.strip()==PIN
    base=(ROOT/'frontend.bend').read_text().replace('./loader.c','../loader.c')
    if arrays:base=array_source(base)
    prefix='array-' if arrays else ''
    rows=[]
    for mode in ['lex','fold']:
        main=f'''
def main() -> IO(Unit):
  do IO<Unit>:
    warm_files : Files <- load()
    warm_result : U32 <- IO.pure(U32, run!(~file.{mode}, warm_files))
    Unit <- IO.print(U32.show(warm_result))
    t0 : Nat <- stamp()
    files : Files <- load()
    +t1 : Nat <- stamp()
    result : U32 <- IO.pure(U32, run!(~file.{mode}, files))
    Unit <- IO.print(U32.show(result))
    t2 : Nat <- stamp()
    Unit <- IO.print(Nat.show(Nat.sub(t1, t0)))
    IO.print(Nat.show(Nat.sub(t2, t1)))
'''
        path=WORK/f'{prefix}{mode}.bend';path.write_text(base+main)
        t=time.perf_counter()
        out=command(['node',pathlib.Path(toolchain)/'bend2/main.ts',path,'-o',WORK/(prefix+mode)],env={**os.environ,'CLANG_MODULE_CACHE_PATH':str(WORK/'clang-cache')})
        rows.append(dict(mode=mode,seconds=time.perf_counter()-t,stdout=out.stdout,stderr=out.stderr))
    command(['clang','-O3','-std=c11','-pthread',ROOT/'native.c','-o',WORK/'native'])
    (RESULTS/('build-array.json' if arrays else 'build.json')).write_text(json.dumps(rows,indent=2)+'\n')

def expr(rng,d):
    if not d:
        v=rng.randrange(65536);return str(v)+'u',v
    a,av=expr(rng,d-1);b,bv=expr(rng,d-1);op=rng.choice(['+','*','^'])
    v=av+bv if op=='+' else av*bv if op=='*' else av^bv
    return f'({a} {op} {b})',v&MASK

def tokens(src):
    for token in re.findall(rb'[A-Za-z_][A-Za-z_0-9]*|[0-9]+u?|[^\s]',src):
        if token[0] in b'0123456789':yield 2,int(token.rstrip(b'u'))&MASK
        elif token[0] in b'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_':
            h=0
            for c in token:h=(h*33+c)&MASK
            yield 1,h
        else:yield 3,token[0]

def generate(name,n,functions,skew=False):
    directory=WORK/name;directory.mkdir(exist_ok=True)
    rng=random.Random(20260917);records=[];expected_fold=expected_lex=0
    verify=[];calls=[]
    for i in range(n):
        nf=(64 if i<n//16 else 1) if skew else functions
        text='';value=0
        for j in range(nf):
            ex,v=expr(rng,4 if j%4 else 3)
            fname=f'f_{i}_{j}';text+=f'unsigned {fname}(void) {{ return {ex}; }}\n';value=(value+v)&MASK
            if i<4 or i in {n//16,n//2,n-1}:
                verify.append(f'unsigned {fname}(void) {{ return {ex}; }}\n');calls.append(f'  sum += {fname}();\n')
        raw=text.encode();path=directory/f'f{i:05}.c';path.write_bytes(raw)
        h=0
        for k,v in tokens(raw):h=(h*16777619+(v^({1:268435456,2:536870912,3:0}[k])))&MASK
        expected_fold=(expected_fold+value)&MASK;expected_lex=(expected_lex+h)&MASK
        records.append((path.name,raw,value,h))
    def pack(order,suffix=''):
        path=directory/('corpus'+suffix+'.bin')
        with path.open('wb') as f:
            f.write(struct.pack('<I',n))
            for i in order:
                raw=records[i][1];f.write(struct.pack('<I',len(raw)));f.write(raw)
        return hashlib.sha256(path.read_bytes()).hexdigest()
    hashes={'clustered':pack(range(n))}
    if skew:
        # Same files: heavy jobs interleaved evenly among consecutive groups.
        heavy=list(range(n//16));light=list(range(n//16,n));order=[]
        for i in heavy:order.extend([i]+light[i*15:(i+1)*15])
        hashes['interleaved']=pack(order,'-interleaved')
    checksrc=''.join(verify)+'\n#include <stdio.h>\nint main(void) { unsigned sum=0;\n'+''.join(calls)+'printf("%u\\n",sum);}\n'
    (directory/'verify.c').write_text(checksrc)
    sample_ids={0,1,2,3,n//16,n//2,n-1}
    meta=dict(name=name,files=n,bytes=sum(len(r[1]) for r in records),functions=sum((64 if i<n//16 else 1) if skew else functions for i in range(n)),lex=expected_lex,fold=expected_fold,sha256=hashes,clang_sample_expected=sum(records[i][2] for i in sample_ids)&MASK)
    (directory/'manifest.json').write_text(json.dumps(meta,indent=2)+'\n')
    (RESULTS/f'{name}-manifest.json').write_text(json.dumps(meta,indent=2)+'\n')
    return meta

def one(case,mode,backend,threads,interleaved=False):
    corpus=WORK/case/('corpus-interleaved.bin' if interleaved else 'corpus.bin')
    env={**os.environ,'CORPUS':str(corpus)}
    args=([WORK/'native',corpus,mode,str(threads)] if backend=='c' else [WORK/(('array-' if backend.endswith('-array') else '')+mode),'--threads',str(threads),'--gpu','on' if backend.startswith('gpu') else 'off'])
    t=time.perf_counter();p=command(args,env=env,timeout=180);wall=time.perf_counter()-t
    values=[int(x) for x in p.stdout.strip().splitlines()]
    assert len(values)==4,values
    warm_checksum=values.pop(0)
    assert warm_checksum==values[0]
    if len(values)!=3:raise RuntimeError(p.stdout+'\n'+p.stderr)
    meta=json.loads((WORK/case/'manifest.json').read_text())
    assert values[0]==meta[mode],(case,mode,backend,values[0],meta[mode])
    return dict(case=case,mode=mode,backend=backend,threads=threads,order='interleaved' if interleaved else 'clustered',checksum=values[0],load_ms=values[1]/1000,kernel_ms=values[2]/1000,wall_ms=wall*1000,wall_passes=2,stderr=p.stderr)

def validate():
    rows=[]
    for case in ['smoke','uniform','many','chunky','skew']:
        d=WORK/case
        if not d.exists():continue
        command(['clang','-O2',d/'verify.c','-o',d/'verify'])
        observed=int(command([d/'verify']).stdout)
        expected=json.loads((d/'manifest.json').read_text())['clang_sample_expected'];assert observed==expected
        rows.append(dict(case=case,clang_sample=observed))
    for mode in ['lex','fold']:
        for backend,threads in [('c',1),('c',10),('bend',1),('bend',10),('gpu',10)]:rows.append(one('smoke',mode,backend,threads))
    (RESULTS/'validation.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(rows,indent=2),flush=True)

def bench(cases,repeats,arrays=False):
    path=RESULTS/'kernels.jsonl'
    for case in cases:
        for mode in ['lex','fold']:
            configs=([('bend-array',1),('bend-array',10),('gpu-array',10)] if arrays else [('c',1),('c',10),('bend',1),('bend',4),('bend',10),('gpu',10)])
            configs=[(b,t,o) for b,t in configs for o in ([False,True] if case=='skew' else [False])]
            for b,t,o in configs:one(case,mode,b,t,o) # warm-up, discarded
            # Shuffle within each round to reduce fixed ordering/thermal bias.
            for rep in range(repeats):
                random.Random(400+rep).shuffle(configs)
                for b,t,o in configs:
                    row=one(case,mode,b,t,o);row['repeat']=rep
                    with path.open('a') as f:f.write(json.dumps(row)+'\n')
                    print(json.dumps(row),flush=True)

def clang_bench(cases,repeats,workers):
    for case in cases:
        directory=WORK/case
        sources=sorted(directory.glob('f*.c'))
        for mode in ['syntax','object']:
            for threads in workers:
                for rep in range(repeats):
                    def compile_file(path):
                        args=['clang','-std=c11','-O0']+(['-fsyntax-only'] if mode=='syntax' else ['-c','-o',directory/(path.stem+'.o')])+[path]
                        command(args)
                    t=time.perf_counter()
                    with concurrent.futures.ThreadPoolExecutor(max_workers=threads) as pool:list(pool.map(compile_file,sources))
                    row=dict(case=case,mode=mode,workers=threads,repeat=rep,seconds=time.perf_counter()-t)
                    with (RESULTS/'clang.jsonl').open('a') as f:f.write(json.dumps(row)+'\n')
                    print(json.dumps(row),flush=True)

def unity(cases,repeats):
    for case in cases:
        d=WORK/case
        path=d/'unity.c'
        path.write_bytes(b''.join(p.read_bytes() for p in sorted(d.glob('f*.c'))))
        for mode in ['syntax','object']:
            args=['clang','-std=c11','-O0']+(['-fsyntax-only'] if mode=='syntax' else ['-c','-o',d/'unity.o'])+[path]
            command(args) # untimed warm-up
            for rep in range(repeats):
                t=time.perf_counter();command(args)
                row=dict(case=case,mode=mode,repeat=rep,seconds=time.perf_counter()-t)
                with (RESULTS/'unity.jsonl').open('a') as f:f.write(json.dumps(row)+'\n')
                print(json.dumps(row),flush=True)

def pack_bench(cases,repeats):
    for case in cases:
        d=WORK/case;sources=sorted(d.glob('f*.c'));meta=json.loads((d/'manifest.json').read_text())
        for rep in range(repeats+1):
            t=time.perf_counter()
            with (d/'repacked.bin').open('wb') as f:
                f.write(struct.pack('<I',len(sources)))
                for path in sources:
                    b=path.read_bytes();f.write(struct.pack('<I',len(b)));f.write(b)
            elapsed=time.perf_counter()-t
            digest=hashlib.sha256((d/'repacked.bin').read_bytes()).hexdigest()
            assert digest==meta['sha256']['clustered']
            if rep:
                row=dict(case=case,repeat=rep-1,seconds=elapsed,sha256=digest)
                with (RESULTS/'packing.jsonl').open('a') as f:f.write(json.dumps(row)+'\n')
                print(json.dumps(row),flush=True)

def main():
    p=argparse.ArgumentParser();p.add_argument('action',choices=['build','generate','validate','bench','clang','unity','pack']);p.add_argument('--toolchain',default='/private/tmp/bend2-feasibility-toolchain');p.add_argument('--cases',nargs='+',default=['uniform','many','chunky','skew']);p.add_argument('--arrays',action='store_true');p.add_argument('--workers',nargs='+',type=int,default=[1,10]);p.add_argument('--repeats',type=int,default=3);a=p.parse_args();RESULTS.mkdir(exist_ok=True)
    if a.action=='build':build(a.toolchain,a.arrays)
    elif a.action=='generate':
        for cfg in [('smoke',16,4,False),('uniform',4096,4,False),('many',16384,1,False),('chunky',1024,32,False),('skew',4096,1,True)]:print(generate(*cfg),flush=True)
    elif a.action=='validate':validate()
    elif a.action=='bench':bench(a.cases,a.repeats,a.arrays)
    elif a.action=='pack':pack_bench(a.cases,a.repeats)
    elif a.action=='unity':unity(a.cases,a.repeats)
    else:clang_bench(a.cases,a.repeats,a.workers)
if __name__=='__main__':main()
