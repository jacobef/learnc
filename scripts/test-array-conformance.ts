// Rebuild Wasm and run tsc -p . first. Add --native to compare defined cases with Clang.
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
Object.assign(globalThis, { window: { location: { pathname: "/" } }, localStorage: { getItem: () => null, setItem: () => {} } });
const { runCFiles } = await import("../js/shared-c-interpreter.js");
type Case = { name: string; source: string; expected: string | "ub" };
const cases: Case[] = [];
const headers = "#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n#include <stddef.h>\n";
function defined(name: string, body: string, expected: string, declarations = "") {
  cases.push({name, source: `${headers}${declarations}\nint main(void) {\n${body}\nreturn 0;\n}\n`, expected});
}
function ub(name: string, body: string, declarations = "") {
  cases.push({name, source: `${headers}${declarations}\nint main(void) {\n${body}\nreturn 0;\n}\n`, expected: "ub"});
}
for (const rows of [2, 3]) for (const columns of [1, 2, 4]) {
  const sum = Array.from({length: rows}, (_, r) => Array.from({length: columns}, (_, c) => 10*r+c+1)).flat().reduce((a,b) => a+b, 0);
  const fill = `for(int r=0;r<${rows};r++) for(int c=0;c<${columns};c++) a[r][c]=10*r+c+1;`;
  for (const [storage, declaration, cleanup] of [
    ["local", `int a[${rows}][${columns}];`, ""],
    ["vla", `int rows=${rows},cols=${columns}; int a[rows][cols];`, ""],
    ["heap", `int (*a)[${columns}]=malloc(${rows}*sizeof *a); if(!a)return 2;`, "free(a);"],
    ["heap_vla", `int cols=${columns}; int (*a)[cols]=malloc(${rows}*sizeof(int[cols])); if(!a)return 2;`, "free(a);"],
  ]) {
    for (const [access, expression] of [["subscript", "a[r][c]"], ["pointer", "*(*(a+r)+c)"]]) {
      defined(`${storage}_${access}_${rows}_${columns}`, `${declaration} ${fill} int sum=0; for(int r=0;r<${rows};r++)for(int c=0;c<${columns};c++)sum+=${expression}; printf("%d",sum); ${cleanup}`, String(sum));
    }
  }
  for (const parameter of [`int a[][${columns}]`, `int a[][n]`, `int a[static ${rows}][n]`, "int (*a)[n]"]) {
    defined(`parameter_${cases.length}`, `int a[${rows}][${columns}]; ${fill} printf("%d", f(${columns},a));`, String(sum), `int f(int n, ${parameter}) {int sum=0; for(int r=0;r<${rows};r++)for(int c=0;c<n;c++)sum+=a[r][c];return sum;}`);
  }
  defined(`row_arithmetic_${rows}_${columns}`, `int a[${rows}][${columns}]={0}; int (*p)[${columns}]=a+${rows}; --p; (*p)[${columns-1}]=19; printf("%td %d",p-a,a[${rows-1}][${columns-1}]);`, `${rows-1} 19`);
  for(const allocation of [false,true]) {
    const declaration = allocation ? `int (*a)[${columns}]=calloc(${rows},sizeof *a);` : `int a[${rows}][${columns}]={0};`;
    ub(`cross_row_${allocation}_${rows}_${columns}`, `${declaration} volatile int c=${columns}; printf("%d",a[0][c]);`);
    ub(`negative_column_${allocation}_${rows}_${columns}`, `${declaration} volatile int c=-1; a[1][c]=4;`);
    ub(`outer_one_past_${allocation}_${rows}_${columns}`, `${declaration} volatile int r=${rows}; printf("%d",a[r][0]);`);
  }
}
for(const depth of [2,3,4]) {
  const dims="[2]".repeat(depth), indices="[1]".repeat(depth);
  defined(`depth_${depth}`, `int a${dims}={0}; a${indices}=41; printf("%d %zu", a${indices},sizeof a/sizeof(int));`, `41 ${2**depth}`);
  defined(`member_depth_${depth}`, `struct S {int before; int a${dims}; int after;}; struct S s={0}; s.before=7; s.after=8; s.a${indices}=41; printf("%d %d %d",s.before,s.a${indices},s.after);`, "7 41 8");
  defined(`memcpy_depth_${depth}`, `int a${dims}={0}, b${dims}={0}; a${indices}=41; memcpy(b,a,sizeof a); printf("%d",b${indices});`, "41");
}
defined("designated_continuation", 'int a[2][3]={[0][1]=2,3,4,5}; for(int i=0;i<2;i++)for(int j=0;j<3;j++)printf("%d ",a[i][j]);', "0 2 3 4 5 0 ");
defined("inferred_outer_bound", 'int a[][2]={[2][1]=9,[0][0]=3}; printf("%zu %d %d %d",sizeof a/sizeof a[0],a[2][1],a[0][0],a[1][1]);', "3 9 3 0");
defined("compound_literal_row", 'int (*a)[2]=(int[][2]){{1,2},{3,4}}; printf("%d",a[1][1]);', "4");
defined("typedef_row_array", 'typedef int Row[3]; Row a[2]={{1,2,3},{4,5,6}}; Row *p=a; p++; printf("%d %zu",(*p)[2],sizeof *p);', "6 12");
defined("vla_typedef_capture", 'int n=3; typedef int Row[n]; n=5; Row a[2]; a[1][2]=7; printf("%zu %d",sizeof a,a[1][2]);', "24 7");
defined("vla_pointer_cast", 'int n=3; int a[2][3]={{1,2,3},{4,5,6}}; void *v=a; int (*p)[n]=(int (*)[n])v; printf("%d %zu",p[1][2],sizeof *p);', "6 12");
defined("vla_parameter_mutated_bound", 'int a[2][3]={{1,2,3},{4,5,6}}; printf("%d",f(3,a));', "18", 'int f(int n,int a[][n]) {n=1; return a[1][2]+sizeof *a;}');
defined("return_row_pointer", 'int a[2][3]={{1,2,3},{4,5,6}}; printf("%d",f(a)[0][2]);', "6", 'int (*f(int a[][3]))[3] {return a+1;}');
defined("row_pointer_roundtrip", 'int a[2][3]={{1,2,3},{4,5,6}}; void *v=&a[1]; int (*p)[3]=v; printf("%d",(*p)[2]);', "6");
defined("whole_matrix_pointer", 'int a[2][3]={{1,2,3},{4,5,6}}; int (*p)[2][3]=&a; printf("%d %td",(*p)[1][2],(p+1)-p);', "6 1");
defined("row_one_past_equality", 'int a[2][3]={0}; printf("%d",a[0]+3==a[1]);', "1");
defined("row_byte_counted_copy", 'int a[2][3]={{1,2,3},{4,5,6}}, b[2][3]={0}; memcpy(b[0],a[0],sizeof a); printf("%d",b[1][2]);', "6");
defined("array_in_union", 'union U {int a[2][3]; int x;}; union U u={.a={{1,2,3},{4,5,6}}}; printf("%d",u.a[1][2]);', "6");
defined("array_struct_assignment", 'struct S {int a[2][3];}; struct S a={{{1,2,3},{4,5,6}}},b; b=a; printf("%d",b.a[1][2]);', "6");
defined("string_rows", 'char a[][4]={"ab","cde"}; printf("%s %s %zu",a[0],a[1],sizeof a);', "ab cde 8");
defined("global_static_rows", 'static int b[2][2]={{5,6},{7,8}}; printf("%d %d",a[1][1],b[1][1]);', "4 8", 'int a[][2]={{1,2},{3,4}};');
defined("vla_bound_side_effects", 'int n=2,m=3; int a[n++][m++]; a[1][2]=7; printf("%d %d %zu %d",n,m,sizeof a,a[1][2]);', "3 4 24 7");
defined("vla_parameter_3d", 'int a[2][3][4]={0}; a[1][2][3]=17; printf("%d",f(3,4,a));', "17", 'int f(int n,int m,int a[][n][m]) {return a[1][n-1][m-1];}');
defined("vla_type_name_sizeof", 'int n=2,m=3; printf("%zu",sizeof(int[n][m]));', "24");
defined("vla_sizeof_operand", 'int n=3,a[2][n]; int (*p)[n]=a; size_t s=sizeof *p++; printf("%zu %td",s,p-a);', "12 1");
defined("array_pointer_increment", 'int a[2][3]={{1,2,3},{4,5,6}}; int (*p)[3]=a; printf("%d ",(*p++)[2]); printf("%d",(*p)[1]);', "3 5");
defined("const_array_parameter", 'int a[2][3]={{1,2,3},{4,5,6}}; printf("%d",f(a));', "6", 'int f(int a[const][3]) {return a[1][2];}');
defined("typedef_vla_parameter", 'int a[2][3]={{1,2,3},{4,5,6}}; printf("%d",f(3,a));', "6", 'int f(int n,int a[][n]) {typedef int Row[n]; Row *p=a+1; return (*p)[n-1];}');
defined("array_realloc", 'int (*a)[3]=calloc(2,sizeof *a); a[1][2]=9; a=realloc(a,3*sizeof *a); a[2][2]=17; printf("%d %d",a[1][2],a[2][2]); free(a);', "9 17");
defined("memmove_rows", 'int a[3][2]={{1,2},{3,4},{5,6}}; memmove(a+1,a,2*sizeof *a); printf("%d %d",a[1][1],a[2][1]);', "2 4");
defined("nested_array_designator_reset", 'struct S {int a[2][2]; int z;}; struct S s={.a[1][0]=3,4,.z=8}; printf("%d %d %d %d",s.a[0][0],s.a[1][0],s.a[1][1],s.z);', "0 3 4 8");
defined("member_array_function", 'struct S {int a[2][3];}; struct S s={{{1,2,3},{4,5,6}}}; printf("%d",f(s.a));', "6", 'int f(int (*a)[3]) {return a[1][2];}');
defined("nested_array_brace_elision", 'int a[2][2][2]={1,2,3,4,5,6,7,8}; printf("%d %d %d",a[0][1][0],a[1][0][1],a[1][1][1]);', "3 6 8");
defined("inferred_3d_bound", 'int a[][2][2]={[1][1][0]=7,8}; printf("%zu %d %d",sizeof a,a[1][1][0],a[1][1][1]);', "32 7 8");
defined("partial_nested_designators", 'int a[2][3]={[1]={8},[0][2]=4}; printf("%d %d %d %d",a[0][0],a[0][2],a[1][0],a[1][1]);', "0 4 8 0");
defined("row_initializer_replacement", 'int a[2][3]={[0][1]=4,[0]={1}}; printf("%d %d %d",a[0][0],a[0][1],a[0][2]);', "1 0 0");
defined("row_initializer_partial_override", 'int a[2][3]={[0]={1,2,3},[0][1]=4}; printf("%d %d %d",a[0][0],a[0][1],a[0][2]);', "1 4 3");
defined("struct_initializer_replacement", 'struct P {int x,y;}; struct S {struct P p; int z;}; struct S s={.p.y=9,.z=5,.p={2}}; printf("%d %d %d",s.p.x,s.p.y,s.z);', "2 0 5");
defined("deep_initializer_replacement", 'int a[2][2][2]={[1][1][1]=9,[1]={{2}}}; printf("%d %d %d",a[1][0][0],a[1][1][1],a[0][0][0]);', "2 0 0");
defined("matrix_in_compound_struct", 'struct S {int a[2][2];}; struct S *s=&(struct S){.a={{1,2},{3,4}}}; printf("%d",s->a[1][1]);', "4");
defined("sizeof_unevaluated_index", 'int a[2][3],i=0; printf("%zu %d",sizeof a[i++],i);', "12 0");
defined("address_one_past_row", 'int a[2][3]={0}; int *p=&a[0][3]; printf("%td",p-a[0]);', "3");
defined("pointer_3d_stride", 'int a[2][3][4]={0}; int (*p)[3][4]=a; p++; (*p)[2][3]=19; printf("%d %td",a[1][2][3],p-a);', "19 1");
defined("flexible_member_rows", 'struct S {int n; int a[][3];}; struct S *s=malloc(sizeof *s+2*sizeof s->a[0]); s->n=2; s->a[1][2]=37; printf("%d",s->a[1][2]); free(s);', "37");
defined("flexible_member_rows_copy", 'struct S {int n; int a[][3];}; struct S *s=malloc(sizeof *s+2*sizeof s->a[0]); int a[2][3]={{1,2,3},{4,5,6}}; memcpy(s->a,a,sizeof a); printf("%d",s->a[1][2]); free(s);', "6");
defined("array_struct_element_decay", 'struct S {int a[2];} s[2]={{{1,2}},{{3,4}}}; int *p=s[1].a; printf("%d",p[1]);', "4");
defined("array_returned_struct", 'printf("%d",f().a[1][1]);', "4", 'struct S {int a[2][2];}; struct S f(void) {struct S s={{{1,2},{3,4}}}; return s;}');
defined("array_generic_lvalue", 'int a[2][2]={{1,2},{3,4}}; _Generic(0,int:a,default:a)[1][1]=9; printf("%d",a[1][1]);', "9");
defined("array_conditional_decay", 'int a[2][2]={{1,2},{3,4}}, b[2][2]={{5,6},{7,8}}; int select=0; printf("%d",(select?a:b)[1][1]);', "8");
defined("vla_bound_shadow", 'int n=3,a[2][n]; a[1][2]=7; {int n=2; printf("%zu %d",sizeof a,a[1][2]);}', "24 7");
defined("vla_parameter_function_pointer", 'int a[2][3]={{1,2,3},{4,5,6}}; int (*g)(int,int (*)[*])=f; printf("%d",g(3,a));', "6", 'int f(int n,int a[][n]) {return a[1][n-1];}');
defined("array_row_character_roundtrip", 'int a[2][3]={{1,2,3},{4,5,6}}; unsigned char *p=(unsigned char *)&a[1]; int (*q)[3]=(int (*)[3])p; printf("%d",(*q)[2]);', "6");
defined("static_array_parameter_slice", 'int a[3][3]={{1,2,3},{4,5,6},{7,8,9}}; printf("%d",f(a+1));', "9", 'int f(int a[static 2][3]) {return a[1][2];}');
defined("large_sizeof_type", 'printf("%zu %zu",sizeof(char[2147483648ULL]),sizeof(int[600000000]));', "2147483648 2400000000");
defined("large_member_offset", 'struct S {char a[2147483648ULL];int x;}; printf("%zu",offsetof(struct S,x));', "2147483648");
ub("flexible_member_cross_row", 'struct S {int n; int a[][3];}; struct S *s=calloc(1,sizeof *s+2*sizeof s->a[0]); volatile int c=3; s->a[0][c]=9;');
ub("array_of_struct_cross_member", 'struct S {int a[2];} s[2]={0}; volatile int i=2; printf("%d",s[0].a[i]);');
ub("vla_unused_argument_wrong_width", 'int a[2][3]={0}; f(2,a);', 'int f(int n,int a[][n]) {return 0;}');
ub("vla_function_pointer_wrong_width", 'int a[2][3]={0}; int (*g)(int,int (*)[*])=f; g(2,a);', 'int f(int n,int a[][n]) {return 0;}');
ub("vla_3d_wrong_inner_dimension", 'int a[2][3][4]={0}; f(3,2,a);', 'int f(int n,int m,int a[][n][m]) {return 0;}');
defined("vla_outer_bound_is_not_minimum", 'int a[2][3]={0}; printf("%d",f(7,a));', "0", 'int f(int n,int a[n][3]) {return 0;}');
defined("vla_fixed_to_runtime_pointer", 'int n=3,a[2][3]={{1,2,3},{4,5,6}}; int (*p)[n]=a; p=a+1; printf("%d",(*p)[2]);', "6");
defined("vla_cast_sizeof", 'int n=3,a[2][3]={{0}}; int (*p)[3]=a; size_t size=sizeof *(int (*)[n++])p; printf("%zu %d",size,n);', "12 4");
defined("vla_comma_sizeof", 'int n=3,a[2][n],i=0; size_t size=sizeof *(++i,a); printf("%zu %d",size,i);', "12 1");
defined("vla_conditional_sizeof", 'int n=3,a[2][n],i=0; size_t size=sizeof *(++i?a:a); printf("%zu %d",size,i);', "12 1");
defined("vla_mismatch_unreached", 'int n=2,a[2][3]; if(0) {int (*p)[n]=a;} printf("ok");', "ok");
defined("fixed_row_of_vla_sizeof", 'int n=2,a[n][3],i=0; size_t size=sizeof a[i++]; printf("%zu %d",size,i);', "12 0");
defined("fixed_cast_of_vla_sizeof", 'int n=3,a[2][n],i=0; size_t size=sizeof *(int (*)[3])(i++,a); printf("%zu %d",size,i);', "12 0");
defined("vla_generic_sizeof", 'int n=3,a[2][n],i=0; size_t size=sizeof *_Generic(0,int:(++i,a)); printf("%zu %d",size,i);', "12 1");
ub("vla_pointer_initializer_wrong_width", 'int n=2,a[2][3]; int (*p)[n]=a;');
ub("vla_pointer_assignment_wrong_width", 'int n=2,a[2][3]; int (*p)[n]=0; p=a;');
ub("vla_source_wrong_width", 'int n=3,a[2][n]; int (*p)[2]=a;');
ub("vla_typedef_wrong_width", 'int n=2; typedef int Row[n]; int a[2][3]; Row *p=a;');
ub("vla_return_wrong_width", 'int a[2][3]; f(3,a);', 'int (*f(int n,int a[][n]))[2] {return a;}');
for (const expression of ["1?a:b", "a-b", "a<b", "a==b"]) {
  ub(`vla_pointer_operand_${expression}`, `int n=2,m=3; int a[2][n],b[2][m]; (void)(${expression});`);
}
ub("inner_3d_one_past", 'int a[2][3][4]={0}; volatile int i=4; printf("%d",a[0][1][i]);');
ub("middle_3d_one_past", 'int a[2][3][4]={0}; volatile int i=3; printf("%d",a[0][i][0]);');
ub("cast_row_escape", 'int a[2][3]={0}; void *v=a[0]; int *p=v; volatile int i=3; printf("%d",p[i]);');
ub("row_pointer_escape", 'int a[2][3]={0}; volatile int i=3; int (*p)[3]=a+i;');
ub("pointer_cross_row", 'int a[2][3]={0}; int *p=a[0]; volatile int n=4; p+=n;');
ub("row_pointer_difference", 'int a[2][3]={0}; printf("%td",&a[1][1]-&a[0][1]);');
ub("uninitialized_nested_element", 'int a[2][3]; a[1][1]=7; printf("%d",a[1][2]);');
ub("freed_matrix", 'int (*a)[3]=calloc(2,sizeof *a); free(a); printf("%d",a[1][1]);');
ub("expired_row", 'int *p; {int a[2][2]={{1,2},{3,4}};p=a[1];} printf("%d",p[0]);');
ub("row_string_overread", 'char a[2][3]={{1,2,3},{4,0,0}}; printf("%zu",strlen(a[0]));');

const native = process.argv.includes("--native");
const scratch = mkdtempSync(join(tmpdir(), "cboxes-arrays-"));
let passed=0;
const failures: object[]=[];
try {
  for(const item of cases) {
    try {
      if(native && item.expected!=="ub") {
        const source=join(scratch,"case.c"),binary=join(scratch,"case");
        writeFileSync(source,item.source);
        const build=spawnSync("clang",["-std=c11","-O0",source,"-o",binary],{encoding:"utf8",timeout:15000});
        assert.equal(build.status,0,build.stderr);
        const run=spawnSync(binary,[],{encoding:"utf8",timeout:5000});
        assert.equal(run.status,0,run.stderr);
        assert.equal(run.stdout,item.expected,"native expectation");
      }
      const result=runCFiles([{path:"program.c",source:item.source}],4096,"",false,{stepLimit:100000,followingTraceLimit:10000,traceLimit:10000});
      if(item.expected==="ub") assert.equal(result.kind,"ub",JSON.stringify(result));
      else {
        assert.equal(result.kind,"ok",JSON.stringify(result));
        assert.equal(result.executionLimit,null);
        assert.equal(result.exitStatus,0);
        assert.equal(result.stdout,item.expected);
      }
      passed++;
    } catch(error) {
      failures.push({name:item.name,error:String(error),source:item.source});
      console.error(`FAIL ${item.name}: ${error}`);
    }
  }
} finally {rmSync(scratch,{recursive:true,force:true});}
console.log(`${passed}/${cases.length} array checks passed (${cases.filter(c=>c.expected!=="ub").length} defined, ${cases.filter(c=>c.expected==="ub").length} UB); Clang: ${native}`);
if(failures.length) {writeFileSync("/tmp/cboxes-array-failures.json",JSON.stringify(failures,null,2));process.exitCode=1;}
