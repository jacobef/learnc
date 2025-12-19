(function(MB){
  const {
    $, renderCodePane, readBoxState, restoreWorkspace,
    serializeWorkspace, randAddr, cloneBoxes, isEmptyVal,
    createHintController, createStepper, makeAnswerBox
  } = MB;

  const instructions = $('#p7a-instructions');
  const NEXT_PAGE = 'program7b.html';

  const p7a = {
    lines:[
      'int a;',
      'int b;',
      'int* ptr;',
      'ptr = &a;',
      'ptr = &b;',
      'int** pptr;',
      'pptr = &ptr;',
      'int* spare;',
      'spare = ptr;'
    ],
    boundary:0,
    aAddr:randAddr('int'),
    bAddr:randAddr('int'),
    ptrAddr:randAddr('int*'),
    pptrAddr:randAddr('int**'),
    spareAddr:randAddr('int*'),
    ws:Array(10).fill(null),
    passes:Array(10).fill(false)
  };

  const editableSteps = new Set([5,6,7,9]);

  function ptrTarget(boundary){
    if (boundary<4) return 'empty';
    if (boundary<5) return String(p7a.aAddr);
    return String(p7a.bAddr);
  }

  function pptrTarget(boundary){
    if (boundary<7) return 'empty';
    return String(p7a.ptrAddr);
  }

  function canonical(boundary){
    const state=[];
    if (boundary>=1){
      state.push({name:'a', names:['a'], type:'int', value:'empty', address:String(p7a.aAddr)});
    }
    if (boundary>=2){
      state.push({name:'b', names:['b'], type:'int', value:'empty', address:String(p7a.bAddr)});
    }
    if (boundary>=3){
      state.push({name:'ptr', names:['ptr'], type:'int*', value:ptrTarget(boundary), address:String(p7a.ptrAddr)});
    }
    if (boundary>=6){
      state.push({name:'pptr', names:['pptr'], type:'int**', value:pptrTarget(boundary), address:String(p7a.pptrAddr)});
    }
    if (boundary>=8){
      state.push({name:'spare', names:['spare'], type:'int*', value: boundary>=9 ? String(p7a.bAddr) : 'empty', address:String(p7a.spareAddr)});
    }
    return state;
  }

  function carriedState(boundary){
    for (let b=boundary-1; b>=0; b--){
      const st=p7a.ws[b];
      if (Array.isArray(st) && st.length) return cloneBoxes(st);
    }
    return null;
  }

  function defaultsFor(boundary){
    if (!editableSteps.has(boundary)) return canonical(boundary);
    const carried = carriedState(boundary);
    const base = carried ? cloneBoxes(carried) : cloneBoxes(canonical(boundary-1));
    return base;
  }

  function updateStatus(){
    if (!instructions) return;
    instructions.textContent = '';
  }

  const hint = createHintController({
    button:'#p7a-hint-btn',
    panel:'#p7a-hint',
    build: buildHint
  });

  function resetHint(){
    hint.hide();
  }

  function buildHint(){
    const ws=document.getElementById('p7aworkspace');
    if (!ws) return 'Step forward to reach the editable line.';
    const boxes=[...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
    const by=Object.fromEntries(boxes.map(b=>[b.name,b]));
    if (!by.a || !by.b || !by.ptr) return {html:'You still need boxes for <code>a</code>, <code>b</code>, and <code>ptr</code>.'};
    if (p7a.boundary>=6 && !by.pptr) return {html:'You also need <code>pptr</code> for this line.'};
    if (by.a.type!=='int' || by.b.type!=='int') return {html:'<code>a</code> and <code>b</code> are <code>int</code>s.'};
    if (by.ptr.type!=='int*') return {html:'<code>ptr</code>\'s type is <code>int*</code>.'};
    if (p7a.boundary>=6 && by.pptr && by.pptr.type!=='int**') return {html:'<code>pptr</code>\'s type is <code>int**</code>.'};
    if (p7a.boundary>=8 && by.spare && by.spare.type!=='int*') return {html:'<code>spare</code> is an <code>int*</code>.'};
    // Skip address validation; addresses are generated for the user.
    const ptrVal = (by.ptr.value||'').trim();
    if (!ptrVal) return {html:'<code>ptr</code>\'s value should not be empty.'};
    if (ptrVal!==String(p7a.bAddr)) return {html:'This line sets <code>ptr</code>\'s value to <code>b</code>\'s address.'};
    if (p7a.boundary>=6){
      if (!by.pptr) return {html:'Add <code>pptr</code> for this line.'};
    }
    if (p7a.boundary===6){
      const pptrVal = (by.pptr?.value||'').trim();
      if (pptrVal && pptrVal!=='empty' && pptrVal!==String(p7a.ptrAddr)) return {html:'<code>pptr</code> starts out empty here.'};
    } else if (p7a.boundary===7){
      const pptrVal = (by.pptr?.value||'').trim();
      if (!pptrVal) return {html:'Set <code>pptr</code> to store <code>ptr</code>\'s address.'};
      if (pptrVal!==String(p7a.ptrAddr)) return {html:'<code>pptr</code> should hold <code>ptr</code>\'s address.'};
    }
    if (p7a.boundary===9){
      if (!by.spare) return {html:'Add <code>spare</code> for this line.'};
      const spareVal = (by.spare.value||'').trim();
      if (spareVal===String(p7a.ptrAddr)) return {html:'<code>spare</code> is being assigned to <code>ptr</code>, not <code>&ptr</code>, so it should be set to <code>ptr</code>\'s value, not <code>ptr</code>\'s address.'};
      if (spareVal!==String(p7a.bAddr)) return {html:'<code>spare</code>\'s value should be set to <code>ptr</code>\'s value.'};
    }

    if (isStepCorrect(boxes)) return 'Looks good. Press Check.';
    const hasReset = !!document.getElementById('p7a-reset');
    return hasReset
      ? 'Your program has a problem that isn\'t covered by a hint. Try starting over on this step by clicking Reset.'
      : 'Your program has a problem that isn\'t covered by a hint. Sorry.';
  }

  function render(){
    renderCodePane($('#p7a-code'), p7a.lines, p7a.boundary);
    const stage=$('#p7a-stage');
    stage.innerHTML='';
    if (editableSteps.has(p7a.boundary) && p7a.passes[p7a.boundary]){
      $('#p7a-status').textContent='correct';
      $('#p7a-status').className='ok';
    } else {
      $('#p7a-status').textContent='';
      $('#p7a-status').className='muted';
    }
    $('#p7a-check').classList.add('hidden');
    $('#p7a-reset').classList.add('hidden');
    $('#p7a-add').classList.add('hidden');
    hint.setButtonHidden(true);
    resetHint();

    if (p7a.boundary>0){
      const editable = editableSteps.has(p7a.boundary) && !p7a.passes[p7a.boundary];
      const defaults = defaultsFor(p7a.boundary);
      const wrap = restoreWorkspace(p7a.ws[p7a.boundary], defaults, 'p7aworkspace', {editable, deletable:editable, allowNameAdd:false, allowNameEdit:false, allowTypeEdit:false});
      stage.appendChild(wrap);
      if (editable){
        $('#p7a-check').classList.remove('hidden');
        $('#p7a-reset').classList.remove('hidden');
        $('#p7a-add').classList.remove('hidden');
        hint.setButtonHidden(false);
      } else {
        p7a.ws[p7a.boundary]=cloneBoxes(defaults);
        p7a.passes[p7a.boundary]=true;
      }
    }
  }

  function save(){
    if (p7a.boundary>=1 && p7a.boundary<=p7a.lines.length){
      p7a.ws[p7a.boundary] = serializeWorkspace('p7aworkspace');
    }
  }

  $('#p7a-reset').onclick=()=>{
    if (p7a.boundary>=1){
      p7a.ws[p7a.boundary]=null;
      render();
    }
  };

  $('#p7a-add').onclick=()=>{
    const ws=document.getElementById('p7aworkspace');
    if (ws) ws.appendChild(makeAnswerBox({}));
  };

  $('#p7a-check').onclick=()=>{
    resetHint();
    if (!editableSteps.has(p7a.boundary)) return;
    const ws=document.getElementById('p7aworkspace');
    if (!ws) return;
    const boxes=[...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
    const ok = isStepCorrect(boxes);
    $('#p7a-status').textContent = ok ? 'correct' : 'incorrect';
    $('#p7a-status').className   = ok ? 'ok' : 'err';
    MB.flashStatus($('#p7a-status'));
    if (ok){
      p7a.passes[p7a.boundary]=true;
      p7a.ws[p7a.boundary]=boxes;
      ws.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
      MB.removeBoxDeleteButtons(ws);
      $('#p7a-check').classList.add('hidden');
      $('#p7a-reset').classList.add('hidden');
      $('#p7a-add').classList.add('hidden');
      hint.hide();
      $('#p7a-hint-btn')?.classList.add('hidden');
      MB.pulseNextButton('p7a');
      pager.update();
    }
  };

  function isStepCorrect(boxes){
    const expected=canonical(p7a.boundary);
    const by=Object.fromEntries(boxes.map(b=>[b.name,b]));
    const ptr=by.ptr;
    const pptr=by.pptr;
    const spare=by.spare;
    if (boxes.length!==expected.length) return false;
    if (!by.a || !by.b || !ptr) return false;
    if (by.a.type!=='int' || by.b.type!=='int' || ptr.type!=='int*') return false;
    if (!isEmptyVal(by.a.value||'') || !isEmptyVal(by.b.value||'')) return false;
    if ((ptr.value||'').trim()!==String(p7a.bAddr)) return false;
    if (p7a.boundary>=6){
      if (!pptr || pptr.type!=='int**') return false;
      const pval=(pptr.value||'').trim();
      if (p7a.boundary<7){
        if (pval && pval!=='empty') return false;
      } else {
        if (pval!==String(p7a.ptrAddr)) return false;
      }
    }
    if (p7a.boundary>=8){
      if (!spare || spare.type!=='int*') return false;
      const sval=(spare.value||'').trim();
      if (p7a.boundary<9){
        if (sval && sval!=='empty') return false;
      } else {
        if (sval!==String(p7a.bAddr)) return false;
      }
    }
    return true;
  }

  const pager = createStepper({
    prefix:'p7a',
    lines:p7a.lines,
    nextPage:NEXT_PAGE,
    getBoundary:()=>p7a.boundary,
    setBoundary:val=>{p7a.boundary=val;},
    onBeforeChange:save,
    onAfterChange:()=>{
      render();
      updateStatus();
    },
    isStepLocked:boundary=>{
      if (editableSteps.has(boundary)) return !p7a.passes[boundary];
      if (boundary===p7a.lines.length) return !p7a.passes[p7a.lines.length];
      return false;
    }
  });

  render();
  updateStatus();
  pager.update();
})(window.MB);
