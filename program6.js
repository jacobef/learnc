(function(MB){
  const {
    $, cloneBoxes, firstNonEmptyClone,
    renderCodePane, restoreWorkspace,
    serializeWorkspace, readBoxState, isEmptyVal,
    createHintController, createStepper
  } = MB;

  const solvedMsg = $('#p5-complete');
  const NEXT_PAGE = 'program7a.html';

  const p5 = {
    lines:['int hammer;','int drill;','drill = 1;','hammer = drill;','drill = 2;','drill = hammer;'],
    boundary:0,
    xAddr:MB.randAddr('int'),
    yAddr:MB.randAddr('int'),
    ws:Array(7).fill(null),
    snaps:Array(7).fill(null),
    passes:Array(7).fill(false)
  };

  const hint = createHintController({
    button:'#p5-hint-btn',
    panel:'#p5-hint',
    build: buildHint
  });

  function resetHint(){
    hint.hide();
  }

  function buildHint(){
    const ws=document.getElementById('p5workspace');
    const boxes=[...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
    const by=Object.fromEntries(boxes.map(b=>[b.name,b]));
    if (isEmptyVal(by.hammer.value||'')) return 'hammer already equals 1 from the earlier assignments.';
    if (by.hammer.value!=='1') return {html:'<code>drill = hammer;</code> should modify drill, not hammer.'};
    if (isEmptyVal(by.drill.value||'')) return 'hammer = drill; put drill\'s value into hammer. What should drill = hammer; do?';
    if (by.drill.value==='2' && by.hammer.value==='2') return 'Remember: this line does not change hammer. Only drill should change here.';
    if (by.drill.value!=='1') return 'hammer = drill; put drill\'s value into hammer. What should drill = hammer; do?';
    const ok = boxes.length===2 && by.hammer && by.drill &&
               by.hammer.type==='int' && by.drill.type==='int' &&
               by.hammer.value==='1' && by.drill.value==='1';
    if (ok) return 'Looks good. Press Check.';
    const hasReset = !!document.getElementById('p5-reset');
    return hasReset
      ? 'Your program has a problem that isn\'t covered by a hint. Try starting over on this step by clicking Reset.'
      : 'Your program has a problem that isn\'t covered by a hint. Sorry.';
  }

  function renderCodePane5(){
    renderCodePane($('#p5-code'), p5.lines, p5.boundary);
  }

  function canonical(boundary){
    const xAddr = String(p5.xAddr);
    const yAddr = String(p5.yAddr);
    const states = {
      1:[
        {name:'hammer', type:'int', value:'empty', address:xAddr}
      ],
      2:[
        {name:'hammer', type:'int', value:'empty', address:xAddr},
        {name:'drill', type:'int', value:'empty', address:yAddr}
      ],
      3:[
        {name:'hammer', type:'int', value:'empty', address:xAddr},
        {name:'drill', type:'int', value:'1', address:yAddr}
      ],
      4:[
        {name:'hammer', type:'int', value:'1', address:xAddr},
        {name:'drill', type:'int', value:'1', address:yAddr}
      ],
      5:[
        {name:'hammer', type:'int', value:'1', address:xAddr},
        {name:'drill', type:'int', value:'2', address:yAddr}
      ],
      6:[
        {name:'hammer', type:'int', value:'1', address:xAddr},
        {name:'drill', type:'int', value:'1', address:yAddr}
      ]
    };
    return cloneBoxes(states[boundary] || []);
  }

  function stateFor(boundary){
    if (boundary<=0) return [];
    const stored = firstNonEmptyClone(p5.ws[boundary], p5.snaps[boundary]);
    if (stored.length) return cloneBoxes(stored);
    if (boundary===6){
      const prev = firstNonEmptyClone(p5.ws[5], p5.snaps[5]);
      if (prev.length) return cloneBoxes(prev);
    }
    return canonical(boundary);
  }

  function render(){
    renderCodePane5();
    const stage=$('#p5-stage');
    stage.innerHTML='';
    const boundary = p5.boundary;
    const atSolved = boundary===6 && p5.passes[6];
    if (atSolved){
      $('#p5-status').textContent='correct';
      $('#p5-status').className='ok';
    } else {
      $('#p5-status').textContent='';
      $('#p5-status').className='muted';
    }
    $('#p5-check').classList.add('hidden');
    $('#p5-reset').classList.add('hidden');

    resetHint();
    hint.setButtonHidden(boundary!==6 || p5.passes[6]);
    if (boundary>0){
      const editable = (boundary===6) && !p5.passes[6];
      const state = stateFor(boundary);
      const wrap = restoreWorkspace(p5.ws[boundary], state, 'p5workspace', {editable, deletable:false});
      stage.appendChild(wrap);
      if (editable){
        $('#p5-check').classList.remove('hidden');
        $('#p5-reset').classList.remove('hidden');
      } else if (state.length){
        const snapshot = cloneBoxes(state);
        p5.ws[boundary] = snapshot;
        p5.snaps[boundary] = snapshot;
      }
    }

  }

  function save(){
    if (p5.boundary>=1 && p5.boundary<=p5.lines.length){
      p5.ws[p5.boundary] = serializeWorkspace('p5workspace');
    }
  }

  $('#p5-reset').onclick=()=>{
    if (p5.boundary===6){
      p5.ws[6]=null;
      p5.snaps[6]=null;
      p5.passes[6]=false;
      render();
    }
  };

  $('#p5-check').onclick=()=>{
    resetHint();
    if (p5.boundary!==6) return;
    const ws=document.getElementById('p5workspace');
    if (!ws) return;
    const boxes=[...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
    const by=Object.fromEntries(boxes.map(b=>[b.name,b]));
    const hammer=by.hammer;
    const drill=by.drill;
    const allTypesOk = boxes.every(b=>b.type==='int');
    const ok = boxes.length===2 && hammer && drill && allTypesOk &&
               hammer.value==='1' && drill.value==='1';

    $('#p5-status').textContent = ok ? 'correct' : 'incorrect';
    $('#p5-status').className   = ok ? 'ok' : 'err';
    MB.flashStatus($('#p5-status'));
      if (ok){
        const snap = serializeWorkspace('p5workspace');
        if (Array.isArray(snap)){
          p5.ws[6]=snap;
      p5.snaps[6]=snap;
    } else {
      p5.ws[6]=boxes;
      p5.snaps[6]=boxes;
    }
    ws.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
    MB.removeBoxDeleteButtons(ws);
    p5.passes[6]=true;
    $('#p5-check').classList.add('hidden');
    $('#p5-reset').classList.add('hidden');
    hint.hide();
    $('#p5-hint-btn')?.classList.add('hidden');
    // no instructions for this program
    MB.pulseNextButton('p5');
    pager.update();
      }
  };
  const pager = createStepper({
    prefix:'p5',
    lines:p5.lines,
    nextPage:NEXT_PAGE,
    getBoundary:()=>p5.boundary,
    setBoundary:val=>{p5.boundary=val;},
    onBeforeChange:save,
    onAfterChange:render,
    isStepLocked:boundary=>boundary===6 && !p5.passes[6]
  });

  render();
  pager.update();
})(window.MB);
