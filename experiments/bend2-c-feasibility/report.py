#!/usr/bin/env python3
"""Aggregate raw samples without treating unlike tasks as compiler speedups."""
import collections,json,pathlib,statistics
ROOT=pathlib.Path(__file__).resolve().parent
R=ROOT/'results'

def rows(name):return [json.loads(x) for x in (R/name).read_text().splitlines()]
def main():
    groups=collections.defaultdict(list)
    for x in rows('kernels.jsonl'):groups[x['case'],x['mode'],x['backend'],x['threads'],x['order']].append(x)
    out=['# Measured results','', 'All kernel times are milliseconds. Medians of five fresh processes, each with one in-process warm-up followed by one timed pass. Input read/conversion is outside the kernel timer. Bend kernel timing includes printing one checksum. Wall time includes BOTH passes and runtime startup, and is not a one-pass compile time.','', '| Corpus | Stage | Backend | Threads | Order | Load median | Kernel median | Kernel min–max | Samples |','|---|---|---|---:|---|---:|---:|---:|---:|']
    summary=[]
    for key,rs in sorted(groups.items()):
        case,mode,backend,threads,order=key
        load=statistics.median(x['load_ms'] for x in rs);times=[x['kernel_ms'] for x in rs];med=statistics.median(times)
        out.append(f'| {case} | {mode} | {backend} | {threads} | {order} | {load:.3f} | {med:.3f} | {min(times):.3f}–{max(times):.3f} | {len(rs)} |')
        summary.append(dict(case=case,stage=mode,backend=backend,threads=threads,order=order,load_median_ms=load,kernel_median_ms=med,kernel_min_ms=min(times),kernel_max_ms=max(times),samples=len(rs)))
    if (R/'clang.jsonl').exists():
        groups=collections.defaultdict(list)
        for x in rows('clang.jsonl'):groups[x['case'],x['mode'],x['workers']].append(x['seconds'])
        out+=['','## Clang process-per-file reference','','Seconds. Full C parsing/semantics and, in object mode, code generation/object writing. No linking. These do more work than the Bend experiment. Filesystem caches are warm; all compiler invocations are fresh.','', '| Corpus | Mode | Workers | Median seconds | Min–max | Samples |','|---|---|---:|---:|---:|---:|']
        for (case,mode,w),xs in sorted(groups.items()):out.append(f'| {case} | {mode} | {w} | {statistics.median(xs):.3f} | {min(xs):.3f}–{max(xs):.3f} | {len(xs)} |')
    if (R/'unity.jsonl').exists():
        out+=['','## Clang unity-build control','','Same generated definitions concatenated into one translation unit. This corpus has unique symbols and no headers, so concatenation is safe here; it is not generally safe for arbitrary C projects.','', '| Corpus | Mode | Median seconds | Samples |','|---|---|---:|---:|']
        groups=collections.defaultdict(list)
        for x in rows('unity.jsonl'):groups[x['case'],x['mode']].append(x['seconds'])
        for (case,mode),xs in sorted(groups.items()):out.append(f'| {case} | {mode} | {statistics.median(xs):.3f} | {len(xs)} |')
    if (R/'packing.jsonl').exists():
        out+=['','## Reading and packing individual source files','','Five samples after one warm-up, cached filesystem. Python reads every individual file and writes the identical packed source corpus; SHA-256 is checked outside the timer. A practical input-cost reference, not an optimized Bend I/O implementation.','', '| Corpus | Median milliseconds | Min–max |','|---|---:|---:|']
        groups=collections.defaultdict(list)
        for x in rows('packing.jsonl'):groups[x['case']].append(x['seconds']*1000)
        for case,xs in sorted(groups.items()):out.append(f'| {case} | {statistics.median(xs):.3f} | {min(xs):.3f}–{max(xs):.3f} |')
    (R/'SUMMARY.md').write_text('\n'.join(out)+'\n');(R/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
if __name__=='__main__':main()
