(function(MB){
  const {
    $, renderCodePane, readBoxState, restoreWorkspace,
    serializeWorkspace, randAddr, cloneBoxes, isEmptyVal,
    createHintController, createStepper
  } = MB;

  const instructions = $('#p7-instructions');
  const NEXT_PAGE = 'program8.html';

  const p7 = {
    lines:[
      'int a;',
      'int b;',
      'int* ptr;',
      'ptr = &a;',
      'ptr = &b;',
      '*ptr = 11;',
      'int** pptr;',
      'pptr = &ptr;'
    ],
    boundary:0,
    aAddr:randAddr('int'),
    bAddr:randAddr('int'),
    ptrAddr:randAddr('int*'),
    pptrAddr:randAddr('int**'),
    ws:Array(9).fill(null),
    passes:Array(9).fill(false),
    aliasExplained:false
  };

  const editableSteps = new Set([6,8]);

  function ptrTarget(boundary){
    if (boundary<4) return 'empty';
    if (boundary<5) return String(p7.aAddr);
    return String(p7.bAddr);
  }

  function pptrTarget(boundary){
    if (boundary<8) return 'empty';
    return String(p7.ptrAddr);
  }

  function canonical(boundary){
    const state=[];
    if (boundary>=1){
      state.push({name:'a', names:['a'], type:'int', value:'empty', address:String(p7.aAddr)});
    }
    if (boundary>=2){
      state.push({name:'b', names:['b'], type:'int', value: boundary>=6 ? '11' : 'empty', address:String(p7.bAddr)});
    }
    if (boundary>=3){
      state.push({name:'ptr', names:['ptr'], type:'int*', value:ptrTarget(boundary), address:String(p7.ptrAddr)});
      const targetVal = ptrTarget(boundary);
      if (targetVal!=='empty'){
        const tgt = state.find(b=>b.address===String(targetVal));
        if (tgt){
          const names=tgt.names || [tgt.name];
          if (!names.includes('*ptr')) names.push('*ptr');
          tgt.names = names;
        }
      }
    }
    if (boundary>=7){
      state.push({name:'pptr', names:['pptr'], type:'int**', value:pptrTarget(boundary), address:String(p7.pptrAddr)});
      const pptrVal = pptrTarget(boundary);
      if (pptrVal!=='empty'){
        const ptrBox = state.find(b=>b.address===String(pptrVal));
        if (ptrBox){
          const ptrAlias = '*pptr';
          const ptrNames = ptrBox.names || [ptrBox.name];
          if (!ptrNames.includes(ptrAlias)) ptrNames.push(ptrAlias);
          ptrBox.names = ptrNames;
        }
        const ptrTargetAddr = ptrTarget(boundary);
        const targetBox = state.find(b=>b.address===String(ptrTargetAddr));
        if (targetBox){
          const derefAlias = '**pptr';
          const names = targetBox.names || [targetBox.name];
          if (!names.includes(derefAlias)) names.push(derefAlias);
          targetBox.names = names;
        }
      }
    }
    return state;
  }

  function normalizePtrValue(value){
    if (!value) return 'empty';
    const trimmed=value.trim();
    if (!trimmed || /^empty$/i.test(trimmed)) return 'empty';
    return trimmed;
  }

  function carriedState(boundary){
    for (let b=boundary-1; b>=0; b--){
      const st=p7.ws[b];
      if (Array.isArray(st) && st.length) return cloneBoxes(st);
    }
    return null;
  }

  function defaultsFor(boundary){
    if (!editableSteps.has(boundary)) return canonical(boundary);
    const carried = carriedState(boundary);
    if (carried) return carried;
    const prev = canonical(boundary-1);
    return cloneBoxes(prev);
  }

  function updateStatus(){
    if (!instructions) return;
    if (p7.boundary===4){
      instructions.innerHTML = 'When ptr is assigned to &a, ptr&#8217;s value becomes a&#8217;s address. Also, a gains an additional name: *ptr.<br><br>We say that ptr now "points to" a.';
      return;
    }
    if (p7.boundary===5){
      instructions.innerHTML = 'When ptr is re-assigned to &b, its value becomes b&#8217;s address. Also, the *ptr name moves with it—b takes the *ptr name from a. We say that ptr now "points to" b.<br><br>In general, if a pointer "points to" a variable, then that variable gains an additional name, which is the name of the pointer preceded by an asterisk.';
      return;
    }
    if (p7.boundary===8){
      instructions.textContent = 'pptr = &ptr adds an alias to ptr, but if you think about it, it also adds one to b. Can you figure out what they should be?';
      return;
    }
    instructions.textContent = '';
  }

  function puzzleComplete(){
    return !!p7.passes[p7.lines.length];
  }

  const hint = createHintController({
    button:'#p7-hint-btn',
    panel:'#p7-hint',
    build: buildHint
  });

  function resetHint(){
    hint.hide();
  }

  function buildHint(){
    if (!editableSteps.has(p7.boundary)) return 'This line is already filled in.';
    const ws=document.getElementById('p7workspace');
    if (!ws) return 'Step forward to begin editing.';
    const boxes=[...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
    if (!boxes.length) return 'The boxes should still be visible—use reset if they disappeared.';
    const expected=canonical(p7.boundary);
    const byName=(name)=>boxes.find(b=>b.name===name || (b.names||[]).includes(name));
    for (const need of expected){
      const box=byName(need.name);
      if (!box) return {html:`You still need the <code>${need.name}</code> box on this line.`};
    }
    const extra = boxes.find(b=>!expected.find(e=>e.name===b.name || (b.names||[]).includes(e.name)));
    if (extra) return {html:`Only keep the boxes declared in the code. Remove <code>${extra.name}</code>.`};

    const filledInt = ['a','b'].map(name=>byName(name)).find(box=>box && !isEmptyVal(box.value||''));
    if (filledInt && p7.boundary<6) return {html:`<code>${filledInt.name}</code> never stores a value—leave it empty.`};

    const ptr=byName('ptr');
    const pptr=byName('pptr');
    if (ptr && ptr.type!=='int*') return {html:'<code>ptr</code> is a pointer, so keep its type <code>int*</code>.'};
    if (pptr && pptr.type!=='int**') return {html:'<code>pptr</code> is declared as <code>int**</code>.'};
    if (ptr && ptr.addr!==String(p7.ptrAddr)) return {html:`<code>ptr</code> itself stays at address ${p7.ptrAddr}.`};
    if (pptr && pptr.addr!==String(p7.pptrAddr)) return {html:`<code>pptr</code> stays at address ${p7.pptrAddr}.`};

    if (p7.boundary===5){
      const normalized=ptr ? normalizePtrValue(ptr.value||'') : 'empty';
      if (normalized==='empty') return {html:`Store the address of <code>a</code> or <code>b</code> (${p7.aAddr} or ${p7.bAddr}) in <code>ptr</code>.`};
      if (normalized!==String(p7.bAddr)) return {html:`After this line <code>ptr</code> should point to <code>b</code>, so enter ${p7.bAddr}.`};
    } else if (p7.boundary===6){
      const normalized=ptr ? normalizePtrValue(ptr.value||'') : 'empty';
      if (normalized!=='empty'){
        const bBox=byName('b');
        if (bBox && bBox.value!=='11') return {html:'<code>*ptr</code> writes into <code>b</code>; set <code>b</code> to <code>11</code>.'};
      }
    } else if (p7.boundary===8){
      const normalized=pptr ? normalizePtrValue(pptr.value||'') : 'empty';
      if (normalized==='empty') return {html:'<code>pptr</code> should store <code>ptr</code>\'s address.'};
      if (normalized!==String(p7.ptrAddr)) return {html:`Remember: <code>ptr</code>'s address is ${p7.ptrAddr}.`};
    }
    const verdict = validateWorkspace(p7.boundary, boxes);
    if (verdict.ok) return 'Looks good. Press Check.';
    const hasReset = !!document.getElementById('p7-reset');
    return hasReset
      ? 'Your program has a problem that isn\'t covered by a hint. Try starting over on this step by clicking Reset.'
      : 'Your program has a problem that isn\'t covered by a hint. Sorry.';
  }

  function render(){
    renderCodePane($('#p7-code'), p7.lines, p7.boundary);
    const stage=$('#p7-stage');
    stage.innerHTML='';
    if (editableSteps.has(p7.boundary) && p7.passes[p7.boundary]){
      $('#p7-status').textContent='correct';
      $('#p7-status').className='ok';
    } else {
      $('#p7-status').textContent='';
      $('#p7-status').className='muted';
    }
    $('#p7-check').classList.add('hidden');
    $('#p7-reset').classList.add('hidden');
    hint.setButtonHidden(true);
    resetHint();

    if (p7.boundary>0){
      const editable = editableSteps.has(p7.boundary) && !p7.passes[p7.boundary];
      const defaults = defaultsFor(p7.boundary);
      const wrap = restoreWorkspace(p7.ws[p7.boundary], defaults, 'p7workspace', {editable, deletable:false, allowNameAdd:false, allowNameEdit:false, allowTypeEdit:false});
      stage.appendChild(wrap);
      if (editable){
        $('#p7-check').classList.remove('hidden');
        $('#p7-reset').classList.remove('hidden');
        hint.setButtonHidden(false);
      } else {
        p7.ws[p7.boundary]=cloneBoxes(defaults);
        p7.passes[p7.boundary]=true;
      }
    }
  }

  function save(){
    if (p7.boundary>=1 && p7.boundary<=p7.lines.length){
      p7.ws[p7.boundary] = serializeWorkspace('p7workspace');
    }
  }

  $('#p7-reset').onclick=()=>{
    if (p7.boundary>=1){
      p7.ws[p7.boundary]=null;
      render();
    }
  };

  $('#p7-check').onclick=()=>{
    resetHint();
    if (!editableSteps.has(p7.boundary)) return;
    const ws=document.getElementById('p7workspace');
    if (!ws) return;
    const boxes=[...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
    const verdict = validateWorkspace(p7.boundary, boxes);
    $('#p7-status').textContent = verdict.ok ? 'correct' : 'incorrect';
    $('#p7-status').className   = verdict.ok ? 'ok' : 'err';
    MB.flashStatus($('#p7-status'));
    if (verdict.ok){
      p7.passes[p7.boundary]=true;
      p7.ws[p7.boundary]=boxes;
      ws.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
      MB.removeBoxDeleteButtons(ws);
      updateStatus();
      $('#p7-check').classList.add('hidden');
      $('#p7-reset').classList.add('hidden');
      hint.hide();
      $('#p7-hint-btn')?.classList.add('hidden');
      MB.pulseNextButton('p7');
      pager.update();
    }
  };

  function validateWorkspace(boundary, boxes){
    const expected=canonical(boundary);
    const findBox = need=>{
      return boxes.find(b=>{
        if (b.name===need.name) return true;
        const names=b.names||[];
        return names.includes(need.name);
      });
    };
    for (const need of expected){
      const box=findBox(need);
      if (!box) return {ok:false, message:`Missing the ${need.name} box.`};
      if (need.type==='int' && box.type!=='int') return {ok:false, message:`${need.name} must stay type int.`};
      if (need.type==='int*' && box.type!=='int*') return {ok:false, message:'ptr must be type int*.'};
      if (need.type==='int**' && box.type!=='int**') return {ok:false, message:'pptr must be type int**.'};
      if (box.addr!==need.address) return {ok:false, message:`${need.name} should remain at address ${need.address}.`};
      if (need.type==='int'){
        const val = box.value||'';
        if (need.value==='empty'){
          if (!isEmptyVal(val)) return {ok:false, message:`${need.name} should stay empty here.`};
        } else {
          if (val!==need.value) return {ok:false, message:`${need.name} should be ${need.value}.`};
        }
        if (Array.isArray(need.names)){
          const names = box.names || [];
          if (!need.names.every(n=>names.includes(n))) return {ok:false, message:`Include alias ${need.names.find(n=>!names.includes(n))}.`};
        }
      } else {
        const normalized=normalizePtrValue(box.value||'');
        if (need.value==='empty' && normalized!=='empty') return {ok:false, message:`${need.name} has not been set yet—leave it empty.`};
        if (need.value!=='empty' && normalized!==need.value){
          if (need.name==='ptr'){
            const targetAddr = need.value;
            const target = targetAddr===String(p7.aAddr) ? 'a' : 'b';
            return {ok:false, message:`ptr should point to ${target} at address ${targetAddr}.`};
          }
          if (need.name==='pptr'){
            return {ok:false, message:'pptr should hold ptr\'s address.'};
          }
        }
      }
    }
    return {ok:true};
  }

  const pager = createStepper({
    prefix:'p7',
    lines:p7.lines,
    nextPage:NEXT_PAGE,
    getBoundary:()=>p7.boundary,
    setBoundary:val=>{p7.boundary=val;},
    onBeforeChange:save,
    onAfterChange:()=>{
      render();
      updateStatus();
    },
    isStepLocked:boundary=>{
      if (editableSteps.has(boundary)) return !p7.passes[boundary];
      if (boundary===p7.lines.length) return !p7.passes[p7.lines.length];
      return false;
    }
  });

  render();
  updateStatus();
  pager.update();
})(window.MB);
