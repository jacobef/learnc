(function(MB){
  const {
    $, randAddr, renderCodePane, vbox, readBoxState, isEmptyVal,
    createHintController, createStepper, flashStatus,
    pulseNextButton, disableBoxEditing, removeBoxDeleteButtons
  } = MB;

  const instructions = $('#p2-instructions');
  const NEXT_PAGE = 'program3.html';

  const p2 = {
    lines:['int def;','int ghi;','def = 28;'],
    boundary:0,
    defAddr:randAddr('int'),
    ghiAddr:randAddr('int'),
    stateGhi:null,
    stateAssign:null,
    passGhi:false,
    passAssign:false
  };

  const hint = createHintController({
    button:'#p2-hint-btn',
    panel:'#p2-hint',
    build: buildHint
  });

  function resetHint(){
    hint.hide();
  }

  function isGhiCorrect(box){
    return box && box.name==='ghi' && box.type==='int' && isEmptyVal(box.value||'');
  }

  function isAssignCorrect(defBox, ghiBox){
    return defBox &&
      defBox.name==='def' &&
      defBox.type==='int' &&
      defBox.value==='28' &&
      (!ghiBox || isEmptyVal(ghiBox.value||''));
  }

  function buildHint(){
    const boxes=[...document.querySelectorAll('#p2-stage .vbox')].map(v=>readBoxState(v));
    if (p2.boundary===2){
      const ghi = boxes.find(b=>b.name==='ghi') || boxes.find(b=>b.name && b.name!=='def') || boxes.find(b=>b);
      if (!ghi) return 'Your program has a problem that isn\'t covered by a hint. Sorry.';
      if (!ghi.name) return {html:'Give the box a name. The variable is called <code>ghi</code>.'};
      if (ghi.name!=='ghi') return {html:'The variable here is <code>ghi</code>, so the name should read <code>ghi</code>.'};
      if (ghi.type!=='int') return {html:'<code>ghi</code> was declared as an <code>int</code>.'};
      if (!isEmptyVal(ghi.value||'')) return {html:'Right after declaration the value should still be empty.'};
      if (isGhiCorrect(ghi)) return 'Looks good. Press Check.';
      return 'Your program has a problem that isn\'t covered by a hint. Sorry.';
    }
    if (p2.boundary===3){
      const def = boxes.find(b=>b.name==='def') || boxes[0];
      const ghi = boxes.find(b=>b.name==='ghi');
      if (!def) return 'Your program has a problem that isn\'t covered by a hint. Sorry.';
      if (!def.name) return {html:'Give the box a name. The variable is called <code>def</code>.'};
      if (def.name!=='def') return {html:'The variable here is <code>def</code>, so the name should read <code>def</code>.'};
      if (def.type!=='int') return {html:'<code>def</code> was declared as an <code>int</code>.'};
      if (ghi && ghi.value==='28' && def.value!=='28') return {html:'<code>28</code> belongs in <code>def</code>\'s value, not <code>ghi</code>.'};
      if (ghi && !isEmptyVal(ghi.value||'')) return {html:'This line doesn\'t change <code>ghi</code>. Leave its value empty.'};
      if (isEmptyVal(def.value||'')) return {html:'Set <code>def</code>\'s value to <code>28</code>.'};
      if (def.value!=='28') return {html:'Line 3 assigns <code>def = 28;</code>.'};
      if (isAssignCorrect(def, ghi)) return 'Looks good. Press Check.';
      return 'Your program has a problem that isn\'t covered by a hint. Sorry.';
    }
    return 'Your program has a problem that isn\'t covered by a hint. Sorry.';
  }

  function updateInstructions(){
    if (!instructions) return;
    if (p2.boundary===2){
      instructions.innerHTML = '<code>int ghi;</code> creates a new box. What should that new box look like?';
    } else if (p2.boundary===3){
      instructions.innerHTML = 'What does <code>def = 28;</code> do?';
    } else {
      instructions.textContent = 'Use Prev/Next to step through the program.';
    }
  }

  function setStatus(text, cls='muted'){
    const status=$('#p2-status');
    if (!status) return;
    status.textContent = text;
    status.className = cls;
  }

  function renderCodePane2(){
    renderCodePane($('#p2-code'), p2.lines, p2.boundary);
  }

  function addPlaceholderIfEmpty(box){
    if (!box) return;
    const valEl = box.querySelector('.value');
    if (valEl && isEmptyVal(valEl.textContent || '')){
      valEl.classList.add('placeholder','muted');
    }
  }

  function p2Render(){
    renderCodePane2();
    const stage=$('#p2-stage');
    stage.innerHTML='';
    resetHint();
    hint.setButtonHidden(true);
    $('#p2-check').classList.add('hidden');

    const solved = (p2.boundary===2 && p2.passGhi) || (p2.boundary===3 && p2.passAssign);
    if (solved){
      setStatus('correct','ok');
    } else {
      setStatus('', 'muted');
    }

    updateInstructions();

    if (p2.boundary===0){
      return;
    }

    const wrap=document.createElement('div');
    wrap.className='grid';

    if (p2.boundary===1 || p2.boundary===2){
      const defStatic=vbox({
        addr:String(p2.defAddr),
        type:'int',
        value:'empty',
        name:'def',
        editable:false
      });
      addPlaceholderIfEmpty(defStatic);
      wrap.appendChild(defStatic);
    }

    if (p2.boundary===1){
      stage.appendChild(wrap);
      return;
    }

    if (p2.boundary===2){
      if (p2.ghiAddr==null) p2.ghiAddr = randAddr('int');
      const seed = p2.stateGhi ?? {addr:String(p2.ghiAddr), type:'', value:'empty', name:''};
      const editable = !p2.passGhi;
      const ghi=vbox({
        addr:String(p2.ghiAddr),
        type: seed.type || '',
        value: seed.value ?? 'empty',
        name: seed.name || '',
        editable,
        allowNameEdit:true,
        allowTypeEdit:true
      });
      if (!editable) disableBoxEditing(ghi);
      addPlaceholderIfEmpty(ghi);
      wrap.appendChild(ghi);
      stage.appendChild(wrap);
      if (editable){
        $('#p2-check').classList.remove('hidden');
        hint.setButtonHidden(false);
      }
      return;
    }

    if (p2.boundary===3){
      // def for assignment
      const seedDef = p2.stateAssign ?? {addr:String(p2.defAddr), type:'int', value:'empty', name:'def'};
      const editable = !p2.passAssign;
      const defBox=vbox({
        addr:String(p2.defAddr),
        type: seedDef.type || 'int',
        value: seedDef.value ?? 'empty',
        name:'def',
        names: seedDef.names,
        editable,
        allowNameEdit:false,
        allowTypeEdit:false
      });
      addPlaceholderIfEmpty(defBox);
      wrap.appendChild(defBox);

      // carry over ghi for context
      const ghiSeed = p2.stateGhi ?? {addr:String(p2.ghiAddr ?? randAddr('int')), type:'int', value:'empty', name:'ghi'};
      const ghi=vbox({
        addr:String(p2.ghiAddr ?? ghiSeed.addr),
        type: ghiSeed.type || 'int',
        value: ghiSeed.value ?? 'empty',
        name: ghiSeed.name || 'ghi',
        names: ghiSeed.names,
        editable
      });
      addPlaceholderIfEmpty(ghi);
      wrap.appendChild(ghi);

      stage.appendChild(wrap);
      if (!editable){
        disableBoxEditing(defBox);
      } else {
        hint.setButtonHidden(false);
        $('#p2-check').classList.remove('hidden');
      }
    }
  }

  function p2Save(){
    const boxes=[...document.querySelectorAll('#p2-stage .vbox')].map(v=>readBoxState(v));
    if (!boxes.length) return;
    if (p2.boundary===2){
      const ghi = boxes.find(b=>b.name==='ghi') || boxes.find(b=>b.name && b.name!=='def');
      if (ghi){
        p2.stateGhi = {...ghi, addr:String(p2.ghiAddr)};
      }
    }
    if (p2.boundary===3){
      const def = boxes.find(b=>b.name==='def');
      if (def){
        p2.stateAssign = {...def, addr:String(p2.defAddr)};
      }
    }
  }

  $('#p2-check').onclick=()=>{
    resetHint();
    const boxes=[...document.querySelectorAll('#p2-stage .vbox')].map(v=>readBoxState(v));

    if (p2.boundary===2){
      const ghi = boxes.find(b=>b.name==='ghi') || boxes.find(b=>b.name && b.name!=='def');
      const ok = isGhiCorrect(ghi);
      setStatus(ok ? 'correct' : 'incorrect', ok ? 'ok' : 'err');
      flashStatus($('#p2-status'));
      if (ok){
        document.querySelectorAll('#p2-stage .vbox').forEach(v=>disableBoxEditing(v));
        removeBoxDeleteButtons(document.getElementById('p2-stage'));
        p2.passGhi=true;
        p2.stateGhi={...ghi, addr:String(p2.ghiAddr)};
        $('#p2-check').classList.add('hidden');
        hint.hide();
        $('#p2-hint-btn')?.classList.add('hidden');
        pulseNextButton('p2');
        pager.update();
      }
      return;
    }

    if (p2.boundary===3){
      const def = boxes.find(b=>b.name==='def');
      const ghi = boxes.find(b=>b.name==='ghi');
      const ok = isAssignCorrect(def, ghi);
      setStatus(ok ? 'correct' : 'incorrect', ok ? 'ok' : 'err');
      flashStatus($('#p2-status'));
      if (ok){
        document.querySelectorAll('#p2-stage .vbox').forEach(v=>disableBoxEditing(v));
        removeBoxDeleteButtons(document.getElementById('p2-stage'));
        p2.passAssign=true;
        p2.stateAssign={...def, addr:String(p2.defAddr)};
        $('#p2-check').classList.add('hidden');
        hint.hide();
        $('#p2-hint-btn')?.classList.add('hidden');
        pulseNextButton('p2');
        pager.update();
      }
    }
  };

  const pager = createStepper({
    prefix:'p2',
    lines:p2.lines,
    nextPage:NEXT_PAGE,
    getBoundary:()=>p2.boundary,
    setBoundary:val=>{p2.boundary=val;},
    onBeforeChange:p2Save,
    onAfterChange:()=>{
      p2Render();
      updateInstructions();
    },
    isStepLocked:boundary=>{
      if (boundary===2) return !p2.passGhi;
      if (boundary===3) return !p2.passAssign;
      return false;
    }
  });

  p2Render();
  updateInstructions();
  pager.update();
})(window.MB);
