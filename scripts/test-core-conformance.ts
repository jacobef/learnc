// Focused checks beyond arrays. Rebuild Wasm and run tsc -p . first.
import assert from "node:assert/strict";
Object.assign(globalThis, {window:{location:{pathname:"/"}},localStorage:{getItem:()=>null,setItem:()=>{}}});
const {runCFiles}=await import("../js/shared-c-interpreter.js");
const defined = [
  ["unsigned wrap", 'unsigned x=UINT_MAX; x++; printf("%u",x);', "0"],
  ["negative division", 'printf("%d %d",-7/3,-7%3);', "-2 -1"],
  ["integer promotions", 'unsigned char c=255; printf("%d",c+1);', "256"],
  ["mixed signed comparison", 'printf("%d",-1<1u);', "0"],
  ["unsigned shifts", 'unsigned x=1u<<31; printf("%u",x>>31);', "1"],
  ["short circuit", 'int i=0; 0 && ++i; 1 || ++i; printf("%d",i);', "0"],
  ["comma sequencing", 'int i=0; int x=(i++,i+1); printf("%d %d",i,x);', "1 2"],
  ["overlapping memmove", 'char s[8]="abcdef"; memmove(s+1,s,5); printf("%s",s);', "aabcde"],
  ["bounded nonterminated source", 'char src[3]={1,2,3},dst[3]; memcpy(dst,src,3); printf("%d",dst[2]);', "3"],
  ["snprintf sizing", 'printf("%d",snprintf(NULL,0,"abc%d",42));', "5"],
  ["snprintf truncation", 'char s[3]; int n=snprintf(s,sizeof s,"abcd"); printf("%s %d",s,n);', "ab 4"],
  ["scanf width", 'char s[4]; int x; int n=sscanf("abc 17","%3s%d",s,&x); printf("%s %d %d",s,x,n);', "abc 17 2"],
  ["realloc preservation", 'int *p=malloc(2*sizeof *p); p[0]=7;p[1]=8;p=realloc(p,4*sizeof *p);printf("%d %d",p[0],p[1]);free(p);', "7 8"],
  ["free null", 'free(NULL);printf("ok");', "ok"],
  ["zeroed allocation", 'int *p=calloc(3,sizeof *p);printf("%d",p[2]);free(p);', "0"],
  ["bitfield neighbors", 'struct S {unsigned x:3,y:3;} s={0}; s.y=5;s.x=9;printf("%u %u",s.x,s.y);', "1 5"],
  ["struct copying", 'struct S {int x;double y;} a={3,2.5},b;memcpy(&b,&a,sizeof a);printf("%d %.1f",b.x,b.y);', "3 2.5"],
  ["switch fallthrough", 'int x=0;switch(2){case 1:x=9;break;case 2:x=3;case 3:x++;}printf("%d",x);', "4"],
  ["loop continue", 'int n=0;for(int i=0;i<4;i++){if(i==2)continue;n+=i;}printf("%d",n);', "4"],
];
const undefinedCases = [
  ["signed overflow", 'volatile int x=INT_MAX;x++;'],
  ["division overflow", 'volatile int x=INT_MIN,y=-1;printf("%d",x/y);'],
  ["remainder overflow", 'volatile int x=INT_MIN,y=-1;printf("%d",x%y);'],
  ["negative shift", 'volatile int n=-1;printf("%u",1u<<n);'],
  ["oversized shift", 'volatile int n=32;printf("%u",1u<<n);'],
  ["signed left shift overflow", 'volatile int n=31;printf("%d",1<<n);'],
  ["float to int overflow", 'volatile double x=1e30;int n=(int)x;printf("%d",n);'],
  ["use after free", 'int *p=malloc(sizeof *p);*p=7;free(p);printf("%d",*p);'],
  ["double free", 'int *p=malloc(sizeof *p);free(p);free(p);'],
  ["interior free", 'int *p=malloc(2*sizeof *p);free(p+1);'],
  ["overlapping memcpy", 'char s[8]="abcdef";memcpy(s+1,s,5);'],
  ["source overread", 'char src[2]={1,2},dst[4];memcpy(dst,src,4);'],
  ["destination overwrite", 'char src[4]={1,2,3,4},dst[2];memcpy(dst,src,4);'],
  ["unterminated string", 'char s[2]={1,2};printf("%zu",strlen(s));'],
  ["variadic type mismatch", 'printf("%d",1.0);'],
  ["unsequenced modification", 'int i=0;printf("%d",i+++i);'],
];
const headers="#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n#include <limits.h>\n";
let passed=0;
for(const [name,body,expected] of [...defined,...undefinedCases]) {
  const result=runCFiles([{path:"program.c",source:`${headers}int main(void){${body}return 0;}\n`}],4096,"",false);
  assert.equal(result.kind,expected===undefined?"ub":"ok",`${name}: ${JSON.stringify(result)}`);
  if(result.kind==="ok") {
    assert.equal(result.stdout,expected,name);
    assert.equal(result.exitStatus,0,name);
    assert.equal(result.executionLimit,null,name);
  } else if(name==="source overread") {
    assert.doesNotMatch(result.diagnostic.message,/destination/);
  }
  passed++;
}
console.log(`${passed} core checks passed (${defined.length} defined, ${undefinedCases.length} UB).`);
