(function(MB){
  const {
    $, randAddr, renderCodePane, vbox, readBoxState, isEmptyVal,
    createHintController, createStepper, pulseNextButton, flashStatus,
    disableBoxEditing
  } = MB;

  const instructions = $('#p1-instructions');
  const NEXT_PAGE = 'program2.html';

  const p1 = {
    lines:['int m;','m = 3;','m = 5;'],
    boundary:0,
    addr:null,
    state3:null,
    passed:false
  };

  const hint = createHintController({
    button:'#p1-hint-btn',
    panel:'#p1-hint',
    build: buildHint
  });

  function resetHint(){
    hint.hide();
  }

  function buildHint(){
    const box = readBoxState($('#p1-stage .vbox'));
    if (isEmptyVal(box?.value||'')) return {html:'Line 3 assigns <code>m = 5;</code>, so the value cannot stay empty.'};
    if (box?.value!=='5') return {html:'The last line stores <code>5</code> in <code>m</code>.'};
    const ok = (p1.boundary===3 && box.type==='int' && box.value==='5');
    if (ok) return 'Looks good. Press Check.';
  }

  function updateInstructions(){
    if (!instructions) return;
    if (p1.boundary===3){
      instructions.innerHTML = 'Edit the box to what it will be after <code>m = 5;</code> is run, then press Check.';
    } else {
      instructions.textContent = 'Use the Prev/Next buttons to see how the program changes the boxes.';
    }
  }

  function updateStatus(){
    if (!instructions) return;
    if (p1.passed){
      instructions.textContent = 'Program solved!';
    } else {
      updateInstructions();
    }
  }

  function renderCodePane1(){
    renderCodePane($('#p1-code'), p1.lines, p1.boundary);
  }

  function p1Render(){
    renderCodePane1();
    const stage = $('#p1-stage');
    stage.innerHTML='';
    if (p1.passed){
      $('#p1-status').textContent='correct';
      $('#p1-status').className='ok';
    } else {
      $('#p1-status').textContent='';
      $('#p1-status').className='muted';
    }
    $('#p1-check').classList.add('hidden');

    resetHint();
    const editable = (p1.boundary===3 && !p1.passed);
    hint.setButtonHidden(!editable);

    updateInstructions();

    if (p1.boundary===1){
      if (p1.addr==null) p1.addr = randAddr('int');
      stage.appendChild(vbox({addr:String(p1.addr),type:'int',value:'empty',name:'m',editable:false}));
      stage.querySelector('.value').classList.add('placeholder','muted');
    } else if (p1.boundary===2){
      if (p1.addr==null) p1.addr = randAddr('int');
      stage.appendChild(vbox({addr:String(p1.addr),type:'int',value:'3',name:'m',editable:false}));
    } else if (p1.boundary===3){
      if (p1.addr==null) p1.addr = randAddr('int');
      const init = p1.state3 ?? {addr:String(p1.addr), type:'int', value:'3', name:'m'};
      const box=vbox({...init, editable});
      if (!editable) disableBoxEditing(box);
      stage.appendChild(box);
      if (editable) $('#p1-check').classList.remove('hidden');
    }

  }

  function p1Save(){
    if (p1.boundary===3){
      const st=readBoxState($('#p1-stage .vbox'));
      if (st) p1.state3=st;
    }
  }

  $('#p1-check').onclick=()=>{
    resetHint();
    if (p1.boundary!==3) return;
    const st=readBoxState($('#p1-stage .vbox'));
    const ok = (st.type==='int' && st.value==='5');
    $('#p1-status').textContent = ok ? 'correct' : 'incorrect';
    $('#p1-status').className = ok ? 'ok' : 'err';
    flashStatus($('#p1-status'));
    if (ok){
      $('#p1-stage .vbox')?.classList.remove('is-editable');
      disableBoxEditing($('#p1-stage .vbox'));
      MB.removeBoxDeleteButtons($('#p1-stage'));
      p1.passed=true;
      $('#p1-check').classList.add('hidden');
      hint.hide();
      $('#p1-hint-btn')?.classList.add('hidden');
      pulseNextButton('p1');
      updateStatus();
      pager.update();
    }
  };

  const pager = createStepper({
    prefix:'p1',
    lines:p1.lines,
    nextPage:NEXT_PAGE,
    getBoundary:()=>p1.boundary,
    setBoundary:val=>{p1.boundary=val;},
    onBeforeChange:p1Save,
    onAfterChange:()=>{
      p1Render();
      updateStatus();
    },
    isStepLocked:(boundary,isEnd)=>{
      return (!!isEnd) && !p1.passed;
    }
  });

  p1Render();
  pager.update();
})(window.MB);
