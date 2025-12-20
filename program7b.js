(function(MB){
  const {
    $, renderCodePane, readBoxState, restoreWorkspace,
    serializeWorkspace, randAddr, cloneBoxes, isEmptyVal,
    createHintController, createStepper
  } = MB;

  const instructions = $('#p7b-instructions');
  const NEXT_PAGE = 'index.html';

  const p7 = {
    lines:[
      'int deer;',
      'int hare;',
      'int* wolf;',
      'wolf = &deer;',
      'wolf = &hare;',
      'int** bear;',
      'bear = &wolf;',
      'int* fox;',
      'fox = wolf;',
      '*wolf = 11;',
      '**bear = 22;',
      'int elk;',
      'elk = *wolf;'
    ],
    boundary:0,
    aAddr:randAddr('int'),
    bAddr:randAddr('int'),
    ptrAddr:randAddr('int*'),
    pptrAddr:randAddr('int**'),
    spareAddr:randAddr('int*'),
    pulledAddr:randAddr('int'),
    ws:Array(14).fill(null),
    passes:Array(14).fill(false),
    aliasExplained:false
  };

  const editableSteps = new Set([9,10,11,13]);

  function ptrTarget(boundary){
    if (boundary<4) return 'empty';
    if (boundary<5) return String(p7.aAddr);
    return String(p7.bAddr);
  }

  function pptrTarget(boundary){
    if (boundary<7) return 'empty';
    return String(p7.ptrAddr);
  }

  function canonical(boundary){
    const state=[];
    if (boundary>=1){
      state.push({name:'deer', names:['deer'], type:'int', value:'empty', address:String(p7.aAddr)});
    }
    if (boundary>=2){
      const bVal = boundary>=11 ? '22' : (boundary>=10 ? '11' : 'empty');
      state.push({name:'hare', names:['hare'], type:'int', value: bVal, address:String(p7.bAddr)});
    }
    if (boundary>=3){
      state.push({name:'wolf', names:['wolf'], type:'int*', value:ptrTarget(boundary), address:String(p7.ptrAddr)});
      const targetVal = ptrTarget(boundary);
      if (targetVal!=='empty'){
        const tgt = state.find(b=>b.address===String(targetVal));
        if (tgt){
          const names=tgt.names || [tgt.name];
          if (!names.includes('*wolf')) names.push('*wolf');
          tgt.names = names;
        }
      }
    }
    if (boundary>=6){
      state.push({name:'bear', names:['bear'], type:'int**', value:pptrTarget(boundary), address:String(p7.pptrAddr)});
      const pptrVal = pptrTarget(boundary);
      if (pptrVal!=='empty'){
        const ptrBox = state.find(b=>b.address===String(pptrVal));
        if (ptrBox){
          const ptrAlias = '*bear';
          const ptrNames = ptrBox.names || [ptrBox.name];
          if (!ptrNames.includes(ptrAlias)) ptrNames.push(ptrAlias);
          ptrBox.names = ptrNames;
        }
        const ptrTargetAddr = ptrTarget(boundary);
        const targetBox = state.find(b=>b.address===String(ptrTargetAddr));
        if (targetBox){
          const derefAlias = '**bear';
          const names = targetBox.names || [targetBox.name];
          if (!names.includes(derefAlias)) names.push(derefAlias);
          targetBox.names = names;
        }
      }
    }
    if (boundary>=8){
      state.push({name:'fox', names:['fox'], type:'int*', value: boundary>=9 ? String(ptrTarget(boundary)) : 'empty', address:String(p7.spareAddr)});
    }
    if (boundary>=12){
      state.push({name:'elk', names:['elk'], type:'int', value: boundary>=13 ? (state.find(b=>b.name==='hare')?.value || 'empty') : 'empty', address:String(p7.pulledAddr)});
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
    if (p7.boundary===0){
      instructions.textContent = 'Let’s revisit 7A, but with some lines added at the end. To understand these new lines, we need a better understanding of what was going on in 7A. Click Next to continue.';
      return;
    }
    if (p7.boundary===4){
      instructions.innerHTML = 'When wolf is assigned to &deer, wolf&#8217;s value becomes deer&#8217;s address. Also, deer gains an additional name. Use the "Show aliases" toggle under deer to reveal this name.<br><br>We say that wolf now "points to" deer.';
      return;
    }
    if (p7.boundary===5){
      instructions.innerHTML = 'When wolf is re-assigned to &hare, its value becomes hare&#8217;s address. Also, the *wolf name moves from deer to hare. Use the "Show aliases" toggle under hare to reveal *wolf. We say that wolf now "points to" hare.<br><br>In general, if some box X points to another box Y, then *X refers to Y. In this case, wolf points to hare, so *wolf refers to hare.<br><br>We&#8217;ll see the relevance of this alternate name later in the program.';
      return;
    }
    if (p7.boundary===7){
      instructions.textContent = 'bear = &wolf; makes *bear refer to wolf, but it also adds another name to hare. Use the "Show aliases" toggle under hare to reveal it.\nSo, what is that new name? Recall that *wolf refers to hare. Replace "wolf" with "*bear": *{wolf} -> *{*bear}. So, "*wolf" becomes "**bear". Therefore, **bear also refers to hare.';
      return;
    }
    instructions.textContent = '';
  }

  function puzzleComplete(){
    return !!p7.passes[p7.lines.length];
  }

  const hint = createHintController({
    button:'#p7b-hint-btn',
    panel:'#p7b-hint',
    build: buildHint
  });

  function resetHint(){
    hint.hide();
  }

  function buildHint(){
    const ws=document.getElementById('p7bworkspace');
    if (!ws) return 'Step forward to begin editing.';
    const boxes=[...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
    const expected=canonical(p7.boundary);
    const byName=(name)=>boxes.find(b=>b.name===name || (b.names||[]).includes(name));

    const filledInt = ['deer','hare'].map(name=>byName(name)).find(box=>box && !isEmptyVal(box.value||''));
    if (filledInt && p7.boundary<6) return {html:`<code>${filledInt.name}</code> hasn't stored a value—leave it empty.`};

    const wolf=byName('wolf');
    const bear=byName('bear');
    const fox=byName('fox');

    if (p7.boundary===9){
      const normalized=bear ? normalizePtrValue(bear.value||'') : 'empty';
      if (normalized!=='empty' && normalized!==String(p7.ptrAddr)) return {html:'<code>bear</code> should store <code>wolf</code>\'s address.'};
      const fox=byName('fox');
      const spareVal = fox ? normalizePtrValue(fox.value||'') : 'empty';
      const expectedPtr = wolf ? normalizePtrValue(wolf.value||'') : 'empty';
      if (fox && spareVal!==expectedPtr) return {html:'<code>fox</code> should copy <code>wolf</code> (the address of <code>hare</code>).'};
    } else if (p7.boundary===10){
      const bBox=byName('hare');
      if (bBox && bBox.value!=='11') return {html:'<code>*wolf</code> and <code>hare</code> are both names for the same box, so <code>*wolf = 11;</code> would be equivalent to <code>hare = 11;</code>.'};
    } else if (p7.boundary===11){
      const bBox=byName('hare');
      if (bBox && bBox.value!=='22') return {html:'<code>**bear</code> and <code>hare</code> are both names for the same box, so <code>**bear = 22;</code> would be equivalent to <code>hare = 22;</code>.'};
    } else if (p7.boundary===13){
      const elk = byName('elk');
      const bBox=byName('hare');
      const bVal = bBox?.value || '';
      if (!elk) return {html:'Make sure <code>elk</code> exists for this line.'};
      if (elk.value!==bVal) return {html:'<code>elk</code>\'s value should be set to <code>*wolf</code>\'s value.'};
    }
    const verdict = validateWorkspace(p7.boundary, boxes);
    if (verdict.ok) return 'Looks good. Press Check.';
    const hasReset = !!document.getElementById('p7b-reset');
    return hasReset
      ? 'Your program has a problem that isn\'t covered by a hint. Try starting over on this step by clicking Reset.'
      : 'Your program has a problem that isn\'t covered by a hint. Sorry.';
  }

  function renderCode(){
    const progress = editableSteps.has(p7.boundary) && !p7.passes[p7.boundary];
    renderCodePane($('#p7b-code'), p7.lines, p7.boundary, {progress});
  }

  function render(){
    renderCode();
    const stage=$('#p7b-stage');
    stage.innerHTML='';
    if (editableSteps.has(p7.boundary) && p7.passes[p7.boundary]){
      $('#p7b-status').textContent='correct';
      $('#p7b-status').className='ok';
    } else {
      $('#p7b-status').textContent='';
      $('#p7b-status').className='muted';
    }
    $('#p7b-check').classList.add('hidden');
    $('#p7b-reset').classList.add('hidden');
    hint.setButtonHidden(true);
    resetHint();

    if (p7.boundary>0){
      const editable = editableSteps.has(p7.boundary) && !p7.passes[p7.boundary];
      const defaults = defaultsFor(p7.boundary);
      const wrap = restoreWorkspace(p7.ws[p7.boundary], defaults, 'p7bworkspace', {editable, deletable:false, allowNameAdd:false, allowNameToggle:true, allowNameEdit:false, allowTypeEdit:false});
      stage.appendChild(wrap);
      if (editable){
        $('#p7b-check').classList.remove('hidden');
        $('#p7b-reset').classList.remove('hidden');
        hint.setButtonHidden(false);
      } else {
        p7.ws[p7.boundary]=cloneBoxes(defaults);
        p7.passes[p7.boundary]=true;
      }
    }
  }

  function save(){
    if (p7.boundary>=1 && p7.boundary<=p7.lines.length){
      p7.ws[p7.boundary] = serializeWorkspace('p7bworkspace');
    }
  }

  $('#p7b-reset').onclick=()=>{
    if (p7.boundary>=1){
      p7.ws[p7.boundary]=null;
      render();
    }
  };

  $('#p7b-check').onclick=()=>{
    resetHint();
    if (!editableSteps.has(p7.boundary)) return;
    const ws=document.getElementById('p7bworkspace');
    if (!ws) return;
    const boxes=[...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
    const verdict = validateWorkspace(p7.boundary, boxes);
    $('#p7b-status').textContent = verdict.ok ? 'correct' : 'incorrect';
    $('#p7b-status').className   = verdict.ok ? 'ok' : 'err';
    MB.flashStatus($('#p7b-status'));
    if (verdict.ok){
      p7.passes[p7.boundary]=true;
      p7.ws[p7.boundary]=boxes;
      ws.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
      MB.removeBoxDeleteButtons(ws);
      updateStatus();
      $('#p7b-check').classList.add('hidden');
      $('#p7b-reset').classList.add('hidden');
      hint.hide();
      $('#p7b-hint-btn')?.classList.add('hidden');
      MB.pulseNextButton('p7b');
      renderCode();
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
      if (need.type==='int*' && box.type!=='int*') return {ok:false, message:`${need.name} must be type int*.`};
      if (need.type==='int**' && box.type!=='int**') return {ok:false, message:'bear must be type int**.'};
      // Addresses are generated; no need to validate them here.
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
          if (need.name==='wolf'){
            const targetAddr = need.value;
            const target = targetAddr===String(p7.aAddr) ? 'deer' : 'hare';
            return {ok:false, message:`wolf should point to ${target} at address ${targetAddr}.`};
          }
          if (need.name==='bear'){
            return {ok:false, message:'bear should hold wolf\'s address.'};
          }
        }
      }
    }
    return {ok:true};
  }

  const pager = createStepper({
    prefix:'p7b',
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
