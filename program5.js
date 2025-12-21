(function(MB){
  const {
    $, txt, renderCodePaneEditable, readEditableCodeLines, parseSimpleStatement,
    applySimpleStatement, randAddr, vbox, isEmptyVal,
    createHintController, createStepper, pulseNextButton, flashStatus, el
  } = MB;

  const instructions = $('#p5-instructions');
  const NEXT_PAGE = 'program6.html';

  const p5 = {
    lines:[{text:'', editable:true, placeholder:''}],
    boundary:0,
    baseState:[],
    expected:[
      {name:'apple', type:'int', value:'10', address:'<i>(any)</i>'},
      {name:'berry', type:'int', value:'5', address:'<i>(any)</i>'}
    ],
    userLines:{},
    pass:false,
    allocBase:null
  };

  function allocFactory(){
    if (p5.allocBase==null) p5.allocBase = randAddr('int');
    let next = p5.allocBase;
    return ()=>{
      const addr = next;
      next += 4;
      return String(addr);
    };
  }

  function initBaseState(){
    if (!p5.baseState.length){
      p5.baseState = [];
    }
  }

  function statesMatch(actual, expected){
    if (!Array.isArray(actual) || !Array.isArray(expected)) return false;
    if (actual.length!==expected.length) return false;
    const byName = Object.fromEntries(expected.map(b=>[b.name, b]));
    for (const b of actual){
      const exp = byName[b.name];
      if (!exp) return false;
      if (exp.type!==b.type) return false;
      if (String(exp.value||'')!==String(b.value||'')) return false;
    }
    return true;
  }

  const hint = createHintController({
    button:'#p5-hint-btn',
    panel:'#p5-hint',
    build: buildHint
  });

  function resetHint(){
    hint.hide();
  }

  function updateInstructions(){
    if (!instructions) return;
    if (p5.pass){
      instructions.textContent = 'Program solved!';
      return;
    }
    instructions.textContent = 'Write the code yourself.';
  }

  function normalizedLines(){
    const map = readEditableCodeLines($('#p5-code'));
    const lines = p5.lines.map((line, idx)=>{
      const val = map[idx];
      return {
        ...line,
        text: (val!=null ? val : line.text) || ''
      };
    });
    return lines;
  }

  function buildHint(){
    const currentState = applyUserProgram();
    const expected = p5.expected;
    const match = statesMatch(currentState, expected);
    if (match) return {html:'Looks good. Press <span class="btn-ref">Check</span>.'};

    const rawLines = normalizedLines();
    const lines = rawLines
      .map(l=>l.text.trim());
    const nonEmpty = lines
      .map((t,idx)=>({t,idx}))
      .filter(({t})=>t && t!==';');
    const missingSemicolon = nonEmpty.find(({t})=>!t.endsWith(';'));
    if (missingSemicolon) return {html:`You forgot the semicolon on line ${missingSemicolon.idx+1}.`};
    const redecl = firstRedeclaration(lines);
    if (redecl) return {html:`You declared <code class="tok-name">${redecl}</code> more than once.`};
    if (!nonEmpty.length || !lines.some(l=>/int\s+apple\s*;/.test(l))) return {html:'Declare apple: try <code class="tok-line">int apple;</code>.'};
    if (!lines.some(l=>/int\s+berry\s*;/.test(l))) return {html:'Declare berry: try <code class="tok-line">int berry;</code>.'};
    if (Array.isArray(currentState)){
      const byName = Object.fromEntries(currentState.map(b=>[b.name, b]));
      for (const exp of expected){
        const actual = byName[exp.name];
        if (!actual) continue;
        const actualVal = String(actual.value ?? '');
        const expectedVal = String(exp.value ?? '');
        if (!isEmptyVal(actualVal) && actualVal !== expectedVal){
          return {html:`<code class="tok-name">${exp.name}</code> should store <code class="tok-value">${expectedVal}</code>, not <code class="tok-value">${actualVal}</code>.`};
        }
      }
    }
    const assignsTotal = lines.filter(l=>/apple/.test(l));
    if (!assignsTotal.some(l=>/apple\s*=\s*10\s*;/.test(l))) return {html:'Store 10 in apple with <code class="tok-line">apple = 10;</code>.'};
    const assignsCount = lines.filter(l=>/berry/.test(l));
    if (!assignsCount.some(l=>/berry\s*=\s*5\s*;/.test(l))) return {html:'Store 5 in berry with <code class="tok-line">berry = 5;</code>.'};
    return {html:'Keep lines to simple declarations or assignments ending with semicolons.'};
  }

  function applyUserProgram(){
    initBaseState();
    const map = readEditableCodeLines($('#p5-code'));
    const lines = p5.lines.map((line, idx)=>{
      const text = (map[idx]!=null ? map[idx] : line.text) || '';
      return {...line, text};
    });
    p5.userLines = map;
    let state = [];
    const alloc = allocFactory();
    const seenDecl = new Set();
    for (const line of lines){
      const parsed = parseSimpleStatement(line.text);
      const trimmed = (line.text || '').trim();
      if (!parsed){
        if (!trimmed || trimmed===';') continue;
        return null;
      }
      if (parsed.kind==='decl'){
        if (seenDecl.has(parsed.name)) return null;
        seenDecl.add(parsed.name);
      }
      const next = applySimpleStatement(state, parsed, {alloc, allowRedeclare:false});
      if (!next) return null;
      state = next;
    }
    return state;
  }

  function renderStage(){
    const stage=$('#p5-stage');
    stage.innerHTML='';
    const expected = p5.expected;
    const actual = applyUserProgram();
    const group=document.createElement('div');
    group.className='state-group two-col';
    group.appendChild(renderState('Expected final state', expected));
    group.appendChild(renderState('Your final state', actual));
    stage.appendChild(group);
  }

  function renderState(title, boxes){
    const wrap=document.createElement('div');
    wrap.className='state-panel';
    const heading=document.createElement('div');
    heading.className='state-heading';
    heading.textContent=title;
    wrap.appendChild(heading);
    const grid=document.createElement('div');
    grid.className='grid';
    if (boxes===null){
      const msg=document.createElement('div');
      msg.className='muted';
      msg.style.padding='8px';
      msg.textContent='(this code is not valid)';
      grid.appendChild(msg);
    } else if (boxes.length===0){
      const msg=document.createElement('div');
      msg.className='muted';
      msg.style.padding='8px';
      msg.textContent='(no variables yet)';
      grid.appendChild(msg);
    } else {
      boxes.forEach(b=>{
        const node=vbox({addr:b.address, type:b.type, value:b.value, name:b.name, editable:false});
        if (isEmptyVal(b.value||'')) node.querySelector('.value').classList.add('placeholder','muted');
        grid.appendChild(node);
      });
    }
    wrap.appendChild(grid);
    return wrap;
  }

  function linesForRender(){
    return p5.lines.map((line,idx)=>({
      text: (p5.userLines[idx]!=null ? p5.userLines[idx] : line.text) || '',
      placeholder: line.placeholder,
      editable:true,
      deletable: p5.lines.length>1
    }));
  }

  function invalidLineIndexes(){
    const map = readEditableCodeLines($('#p5-code'));
    const set=new Set();
    let state=[];
    const alloc = allocFactory();
    const seenDecl=new Set();
    for (let i=0;i<p5.lines.length;i++){
      const text = (map[i]!=null ? map[i] : p5.lines[i].text) || '';
      const trimmed = text.trim();
      if (!trimmed || trimmed===';') continue;
      const parsed = parseSimpleStatement(text);
      if (!parsed){ set.add(i); continue; }
      if (parsed.kind==='decl'){
        if (seenDecl.has(parsed.name)){
          set.add(i);
          continue;
        }
        seenDecl.add(parsed.name);
      }
      const next = applySimpleStatement(state, parsed, {alloc, allowRedeclare:false});
      if (!next){ set.add(i); continue; }
      state = next;
    }
    return set;
  }

  function firstRedeclaration(lines){
    const seen=new Set();
    for (const t of lines){
      const parsed=parseSimpleStatement(t);
      if (parsed && parsed.kind==='decl'){
        if (seen.has(parsed.name)) return parsed.name;
        seen.add(parsed.name);
      }
    }
    return null;
  }

  function markInvalidLines(set){
    const code=$('#p5-code .codecol');
    if (!code) return;
    code.querySelectorAll('.line-invalid').forEach(n=>n.remove());
    code.querySelectorAll('.line').forEach((line,idx)=>{
      if (!set.has(idx)) return;
      const icon=el('<span class=\"line-invalid\" aria-label=\"Invalid line\" title=\"Line would not compile\">🚫</span>');
      line.appendChild(icon);
    });
  }

  function render(){
    renderCodePaneEditable($('#p5-code'), linesForRender(), null);
    markInvalidLines(invalidLineIndexes());
    renderStage();
    resetHint();
    if (p5.pass){
      $('#p5-status').textContent='correct';
      $('#p5-status').className='ok';
    } else {
      $('#p5-status').textContent='';
      $('#p5-status').className='muted';
    }
    const editable = !p5.pass;
    const checkBtn=$('#p5-check');
    const hintBtn=$('#p5-hint-btn');
    const addBtn=$('#p5-add-line');
    if (editable){
      checkBtn?.classList.remove('hidden');
      hintBtn?.classList.remove('hidden');
      addBtn?.classList.remove('hidden');
    } else {
      checkBtn?.classList.add('hidden');
      hintBtn?.classList.add('hidden');
      addBtn?.classList.add('hidden');
    }
    updateInstructions();
    bindLineAdd();
  }

  function bindLineAdd(){
    const code = $('#p5-code');
    if (!code) return;
    code.querySelectorAll('.code-editable').forEach(el=>{
      el.oninput = ()=>{
        renderStage();
        markInvalidLines(invalidLineIndexes());
      };
      el.onkeydown = e=>{
        if (e.key==='Enter'){ e.preventDefault(); addLine(); }
        if (e.key==='Backspace'){
          const t=txt(el);
          if (t===''){
            const idx=Number(el.dataset.index||-1);
            if (idx>=0 && p5.lines.length>1){
              removeLine(idx, {focusPrev:true});
              e.preventDefault();
            }
          }
        }
      };
    });
    code.querySelectorAll('.code-delete').forEach(btn=>{
      btn.onclick=()=>{
        const idx=Number(btn.dataset.index||-1);
        if (idx>=0) removeLine(idx);
      };
    });
  }

  function addLine(){
    p5.lines.push({text:'', editable:true, placeholder:''});
    render();
    focusLastLine();
  }

  $('#p5-add-line').onclick=addLine;

  function focusLastLine(){
    const code=$('#p5-code');
    if (!code) return;
    const nodes=code.querySelectorAll('.code-editable');
    const last=nodes[nodes.length-1];
    if (last){
      last.focus();
      const range=document.createRange();
      range.selectNodeContents(last);
      range.collapse(false);
      const sel=window.getSelection();
      sel.removeAllRanges();
      sel.addRange(range);
    }
  }

  function removeLine(idx, {focusPrev=false}={}){
    if (p5.lines.length<=1) return;
    p5.lines.splice(idx,1);
    const newMap={};
    Object.entries(p5.userLines||{}).forEach(([k,v])=>{
      const i=Number(k);
      if (i<idx) newMap[i]=v;
      else if (i>idx) newMap[i-1]=v;
    });
    p5.userLines=newMap;
    render();
    if (focusPrev){
      const code=$('#p5-code');
      if (code){
        const nodes=code.querySelectorAll('.code-editable');
        const target=nodes[Math.max(0, idx-1)] || nodes[0];
        if (target){
          target.focus();
          const range=document.createRange();
          range.selectNodeContents(target);
          range.collapse(false);
          const sel=window.getSelection();
          sel.removeAllRanges();
          sel.addRange(range);
        }
      }
    }
  }

  $('#p5-check').onclick=()=>{
    const state=applyUserProgram();
    const expected=p5.expected;
    const match = statesMatch(state, expected);
    $('#p5-status').textContent = match ? 'correct' : 'incorrect';
    $('#p5-status').className = match ? 'ok' : 'err';
    flashStatus($('#p5-status'));
    if (match){
      const ws=document.getElementById('p5-stage');
      ws?.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
      MB.removeBoxDeleteButtons(ws);
      p5.pass=true;
      const code=$('#p5-code');
      code?.querySelectorAll('.code-editable').forEach(el=>{
        el.removeAttribute('contenteditable');
        el.classList.remove('code-editable');
      });
      $('#p5-check').classList.add('hidden');
      $('#p5-add-line')?.classList.add('hidden');
      hint.hide();
      $('#p5-hint-btn')?.classList.add('hidden');
      pulseNextButton('p5');
      pager.update();
    }
  };

  const pager = createStepper({
    prefix:'p5',
    lines:0,
    nextPage:NEXT_PAGE,
    getBoundary:()=>0,
    setBoundary:()=>{},
    onAfterChange:render,
    isStepLocked:()=>!p5.pass
  });

  render();
  pager.update();
})(window.MB);
