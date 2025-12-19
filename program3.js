(function(MB){
  const {
    $, el, randAddr, renderCodePane, vbox, readBoxState,
    isEmptyVal, makeAnswerBox, serializeWorkspace,
    restoreWorkspace, cloneStateBoxes, ensureBox,
    createHintController, createStepper
  } = MB;

  const solvedMsg = $('#p3-complete');
  const NEXT_PAGE = 'program4.html';

  const p3 = {
    lines:['int north;','int south;','north = 5;','int east;','east = 9;'],
    boundary:0,
    aAddr:randAddr('int'),
    bAddr:randAddr('int'),
    ws3:null,
    ws4:null,
    ws5:null,
    pass3:false,
    pass4:false,
    pass5:false
  };

  const hint = createHintController({
    button:'#p3-hint-btn',
    panel:'#p3-hint',
    build: buildHint
  });

  function resetHint(){
    hint.hide();
  }

  function buildHint(){
    const ws=document.getElementById('p3workspace');
    if (!ws) return 'Use “+ New box” to add the variables you need for this step.';
    const boxes=[...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
    if (!boxes.length) return 'Add boxes for the variables before checking.';

    const by=Object.fromEntries(boxes.map(b=>[b.name,b]));
    const required = (p3.boundary===3) ? ['north','south'] : ['north','south','east'];
    const missing = required.filter(name=>!by[name]);
    if (missing.length) return {html:`You still need a box for <code>${missing[0]}</code>.`};
    if (boxes.length>required.length){
      const extra = boxes.find(b=>!required.includes(b.name));
      return {html:`Only keep boxes for <code>${required.join('</code>, <code>')}</code>. Remove <code>${extra?.name || 'the extra box'}</code>.`};
    }
    const wrongType = boxes.find(b=>b.type!=='int');
    if (wrongType){
      const label = wrongType.name ? `<code>${wrongType.name}</code>` : 'This box';
      const typeLabel = wrongType.type ? `<code>${wrongType.type}</code>` : '<code>unknown</code>';
      return {html:`${label} should be an <code>int</code>, not a ${typeLabel}.`};
    }

    if (p3.boundary===3){
      if (by.north.value!=='5') return {html:'Line 3 assigns <code>north = 5;</code>.'};
      if (!isEmptyVal(by.south.value||'')) return {html:'<code>south</code> has not been assigned yet—leave its value empty.'};
    } else if (p3.boundary===4){
      if (!isEmptyVal(by.south.value||'')) return {html:'<code>south</code> is still empty at this point.'};
      if (!isEmptyVal(by.east.value||'')) return {html:'<code>east</code> was just declared, so its value should be empty.'};
      if (by.north.value!=='5') return {html:'<code>north</code> keeps the value <code>5</code>.'};
    } else if (p3.boundary===5){
      if (by.east.value!=='9') return {html:'Line 5 stores <code>9</code> in <code>east</code>.'};
      if (!isEmptyVal(by.south.value||'')) return {html:'<code>south</code>\'s value should still be empty.'};
      if (by.north.value!=='5') return {html:'<code>north</code>\'s value should remain 5.'};
    }
    const allTypesOk = boxes.every(b=>b.type==='int');
    const ok =
      (p3.boundary===3 && boxes.length===2 && allTypesOk &&
       by.north && by.south && by.north.value==='5' && isEmptyVal(by.south.value||'')) ||
      (p3.boundary===4 && boxes.length===3 && allTypesOk &&
       by.north && by.south && by.east &&
       by.north.value==='5' && isEmptyVal(by.south.value||'') && isEmptyVal(by.east.value||'')) ||
      (p3.boundary===5 && boxes.length===3 && allTypesOk &&
       by.north && by.south && by.east &&
       by.north.value==='5' && isEmptyVal(by.south.value||'') && by.east.value==='9');
    if (ok) return 'Looks good. Press Check.';
    const hasReset = !!document.getElementById('p3-reset');
    return hasReset
      ? 'Your program has a problem that isn\'t covered by a hint. Try starting over on this step by clicking Reset.'
      : 'Your program has a problem that isn\'t covered by a hint. Sorry.';
  }

  function renderCodePane3(){
    renderCodePane($('#p3-code'), p3.lines, p3.boundary);
  }

  function restoreDefaults(state, defaults){
    return restoreWorkspace(state, defaults, 'p3workspace');
  }

  function p3Render(){
    renderCodePane3();
    const stage=$('#p3-stage');
    stage.innerHTML='';
    const instructions = $('#p3-instructions');
    const solved =
      (p3.boundary===3 && p3.pass3) ||
      (p3.boundary===4 && p3.pass4) ||
      (p3.boundary===5 && p3.pass5);
    if (solved){
      $('#p3-status').textContent='correct';
      $('#p3-status').className='ok';
    } else {
      $('#p3-status').textContent='';
      $('#p3-status').className='muted';
    }
    $('#p3-add').classList.add('hidden');
    $('#p3-check').classList.add('hidden');
    $('#p3-reset').classList.add('hidden');

    resetHint();
    if (instructions){
      instructions.textContent = '';
      if (p3.boundary===4 && !p3.pass4){
        instructions.textContent = 'Make a new box for east.';
        instructions.classList.remove('hidden');
      } else {
        instructions.classList.add('hidden');
      }
    }
    hint.setButtonHidden(true);

    if (p3.boundary===0){
      // blank
    } else if (p3.boundary===1){
      const a=vbox({addr:String(p3.aAddr),type:'int',value:'empty',name:'north',editable:false});
      a.querySelector('.value').classList.add('placeholder','muted');
      stage.appendChild(a);
    } else if (p3.boundary===2){
      const wrap=el('<div class="grid"></div>');
      const a=vbox({addr:String(p3.aAddr),type:'int',value:'empty',name:'north',editable:false});
      const b=vbox({addr:String(p3.bAddr),type:'int',value:'empty',name:'south',editable:false});
      a.querySelector('.value').classList.add('placeholder','muted');
      b.querySelector('.value').classList.add('placeholder','muted');
      wrap.appendChild(a);
      wrap.appendChild(b);
      stage.appendChild(wrap);
    } else if (p3.boundary===3){
      const editable = !p3.pass3;
      const wrap = restoreWorkspace(p3.ws3, [
        {name:'north', type:'int', value:'empty', address:String(p3.aAddr)},
        {name:'south', type:'int', value:'empty', address:String(p3.bAddr)}
      ], 'p3workspace', {editable, deletable:editable});
      if (!editable) wrap.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
      stage.appendChild(wrap);
      if (editable){
        $('#p3-add').classList.remove('hidden');
        $('#p3-reset').classList.remove('hidden');
        $('#p3-check').classList.remove('hidden');
        hint.setButtonHidden(false);
      }
    } else if (p3.boundary===4){
      const defaults = (()=>{
        const base = cloneStateBoxes(p3.ws3);
        if (!base.length){
          base.push(
            {name:'north', type:'int', value:'empty', address:String(p3.aAddr)},
            {name:'south', type:'int', value:'empty', address:String(p3.bAddr)}
          );
        }
        ensureBox(base, {name:'north', type:'int', address:String(p3.aAddr)});
        ensureBox(base, {name:'south', type:'int', address:String(p3.bAddr)});
        return base;
      })();
      const editable = !p3.pass4;
      const wrap = restoreWorkspace(p3.ws4, defaults, 'p3workspace', {editable, deletable:editable});
      if (!editable) wrap.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
      stage.appendChild(wrap);
      if (editable){
        $('#p3-add').classList.remove('hidden');
        $('#p3-reset').classList.remove('hidden');
        $('#p3-check').classList.remove('hidden');
        hint.setButtonHidden(false);
      }
    } else if (p3.boundary===5){
      const defaults = (()=>{
        const base = cloneStateBoxes(p3.ws4);
        if (!base.length){
          const prior = cloneStateBoxes(p3.ws3);
          base.push(...prior);
        }
        if (!base.length){
          base.push(
            {name:'north', type:'int', value:'empty', address:String(p3.aAddr)},
            {name:'south', type:'int', value:'empty', address:String(p3.bAddr)}
          );
        }
        ensureBox(base, {name:'north', type:'int', address:String(p3.aAddr)});
        ensureBox(base, {name:'south', type:'int', address:String(p3.bAddr)});
        return base;
      })();
      const editable = !p3.pass5;
      const wrap = restoreWorkspace(p3.ws5, defaults, 'p3workspace', {editable, deletable:editable});
      if (!editable) wrap.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
      stage.appendChild(wrap);
      if (editable){
        $('#p3-add').classList.remove('hidden');
        $('#p3-reset').classList.remove('hidden');
        $('#p3-check').classList.remove('hidden');
        hint.setButtonHidden(false);
      }
    }

  }

  function saveWorkspace(){
    if (p3.boundary===3) p3.ws3 = serializeWorkspace('p3workspace');
    if (p3.boundary===4) p3.ws4 = serializeWorkspace('p3workspace');
    if (p3.boundary===5) p3.ws5 = serializeWorkspace('p3workspace');
  }

  $('#p3-add').onclick=()=>{
    const ws=document.getElementById('p3workspace');
    if (ws) ws.appendChild(makeAnswerBox({}));
  };

  $('#p3-reset').onclick=()=>{
    if (p3.boundary===3) p3.ws3=null;
    if (p3.boundary===4) p3.ws4=null;
    if (p3.boundary===5) p3.ws5=null;
    p3Render();
  };

  $('#p3-check').onclick=()=>{
    resetHint();
    const ws=$('#p3workspace');
    if (!ws) return;
    const boxes=[...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
    const by=Object.fromEntries(boxes.map(b=>[b.name,b]));
    const allTypesOk = boxes.every(b=>b.type==='int');

    if (p3.boundary===3){
      const ok = boxes.length===2 && allTypesOk &&
                 by.north && by.south &&
                 by.north.value==='5' && isEmptyVal(by.south.value||'');
      $('#p3-status').textContent = ok ? 'correct' : 'incorrect';
      $('#p3-status').className   = ok ? 'ok' : 'err';
      MB.flashStatus($('#p3-status'));
      if (ok){
        ws.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
        MB.removeBoxDeleteButtons(ws);
        $('#p3-check').classList.add('hidden');
        $('#p3-add').classList.add('hidden');
        $('#p3-reset').classList.add('hidden');
        hint.hide();
        $('#p3-hint-btn')?.classList.add('hidden');
        p3.pass3=true;
        MB.pulseNextButton('p3');
        pager.update();
      }
      return;
    }

    if (p3.boundary===4){
      const ok = boxes.length===3 && allTypesOk &&
                 by.north && by.south && by.east &&
                 by.north.value==='5' && isEmptyVal(by.south.value||'') && isEmptyVal(by.east.value||'');
      $('#p3-status').textContent = ok ? 'correct' : 'incorrect';
      $('#p3-status').className   = ok ? 'ok' : 'err';
      MB.flashStatus($('#p3-status'));
      if (ok){
        ws.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
        MB.removeBoxDeleteButtons(ws);
        $('#p3-check').classList.add('hidden');
        $('#p3-add').classList.add('hidden');
        $('#p3-reset').classList.add('hidden');
        hint.hide();
        $('#p3-hint-btn')?.classList.add('hidden');
        p3.pass4=true;
        MB.pulseNextButton('p3');
        pager.update();
      }
      return;
    }

    if (p3.boundary===5){
      const ok = boxes.length===3 && allTypesOk &&
                 by.north && by.south && by.east &&
                 by.north.value==='5' && isEmptyVal(by.south.value||'') && by.east.value==='9';
      $('#p3-status').textContent = ok ? 'correct' : 'incorrect';
      $('#p3-status').className   = ok ? 'ok' : 'err';
      MB.flashStatus($('#p3-status'));
      if (ok){
        ws.querySelectorAll('.vbox').forEach(v=>MB.disableBoxEditing(v));
        MB.removeBoxDeleteButtons(ws);
        $('#p3-check').classList.add('hidden');
        $('#p3-add').classList.add('hidden');
        $('#p3-reset').classList.add('hidden');
        hint.hide();
        $('#p3-hint-btn')?.classList.add('hidden');
        p3.pass5=true;
        MB.pulseNextButton('p3');
        pager.update();
      }
    }
  };

  const pager = createStepper({
    prefix:'p3',
    lines:p3.lines,
    nextPage:NEXT_PAGE,
    getBoundary:()=>p3.boundary,
    setBoundary:val=>{p3.boundary=val;},
    onBeforeChange:saveWorkspace,
    onAfterChange:p3Render,
    isStepLocked:boundary=>{
      if (boundary===3) return !p3.pass3;
      if (boundary===4) return !p3.pass4;
      if (boundary===5) return !p3.pass5;
      return false;
    }
  });

  p3Render();
  pager.update();
})(window.MB);
