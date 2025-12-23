(function(MB){
  const { $, randAddr, vbox, isEmptyVal, createSimpleSimulator } = MB;

  const instructions = $('#sandbox-instructions');
  const editor = $('#sandbox-editor');
  const lineNumbers = $('#sandbox-line-numbers');
  const errorGutter = $('#sandbox-error-gutter');
  const errorDetail = $('#sandbox-error-detail');
  const measureEl = (() => {
    if (!editor || !editor.parentElement) return null;
    const el = document.createElement('div');
    el.className = 'code-textarea-measure';
    el.setAttribute('aria-hidden', 'true');
    editor.parentElement.appendChild(el);
    return el;
  })();

  const sandbox = {
    text: editor ? editor.value : '',
    allocBase:null,
    errorDetailLine:null,
    errorDetailMessage:'',
    errorDetailHtml:'',
    errorDetailKind:''
  };

  const simulator = createSimpleSimulator({
    allowVarAssign:true,
    allowDeclAssign:true,
    allowDeclAssignVar:true,
    requireSourceValue:true,
    allowPointers:true
  });

  function allocFactory(){
    if (sandbox.allocBase==null) sandbox.allocBase = randAddr('int');
    let next = sandbox.allocBase;
    return ()=>{
      const addr = next;
      next += 4;
      return String(addr);
    };
  }

  function updateInstructions(){
    if (!instructions) return;
    instructions.textContent = 'Write C declarations and assignments. The program state updates as you type.';
  }

  function parseErrorMessage(message){
    if (message && typeof message === 'object'){
      return {
        text: String(message.text || ''),
        html: message.html ? String(message.html) : ''
      };
    }
    return {text: String(message || ''), html:''};
  }

  function combineMessages(primary, secondary){
    if (!secondary) return primary;
    const p = parseErrorMessage(primary);
    const s = parseErrorMessage(secondary);
    const text = [p.text, s.text].filter(Boolean).join(' ');
    const htmlParts = [p.html || p.text, s.html || s.text].filter(Boolean);
    const html = htmlParts.join(' ');
    return {text, html};
  }

  function showErrorDetail(message, kind){
    if (!errorDetail) return;
    const parsed = parseErrorMessage(message);
    errorDetail.innerHTML = '';
    if (kind === 'ub'){
      const base = document.createElement('span');
      if (parsed.html){
        base.innerHTML = parsed.html;
      } else {
        base.textContent = parsed.text;
      }
      base.appendChild(document.createTextNode(' This line causes undefined behavior. '));
      errorDetail.appendChild(base);
      const link = document.createElement('button');
      link.type = 'button';
      link.className = 'ub-explain-link';
      link.textContent = '[What is undefined behavior?]';
      const explain = document.createElement('div');
      explain.className = 'ub-explain hidden';
      explain.textContent = 'Undefined behavior means the C standard does not define what happens. The program might crash, act strangely, or appear to work.';
      link.addEventListener('click', (e)=>{
        e.preventDefault();
        e.stopPropagation();
        explain.classList.toggle('hidden');
      });
      errorDetail.appendChild(link);
      errorDetail.appendChild(explain);
    } else if (parsed.html){
      errorDetail.innerHTML = parsed.html;
    } else {
      errorDetail.textContent = parsed.text;
    }
    errorDetail.classList.remove('hidden');
    sandbox.errorDetailMessage = parsed.text;
    sandbox.errorDetailHtml = parsed.html;
    sandbox.errorDetailKind = kind || 'compile';
  }

  function hideErrorDetail(){
    if (!errorDetail) return;
    errorDetail.textContent = '';
    errorDetail.classList.add('hidden');
    sandbox.errorDetailLine = null;
    sandbox.errorDetailMessage = '';
    sandbox.errorDetailHtml = '';
    sandbox.errorDetailKind = '';
  }

  function getEditorText(){
    return editor ? editor.value : (sandbox.text || '');
  }

  function getRawLines(){
    return getEditorText().split(/\r?\n/);
  }

  function classifyLineStatuses(lines){
    return simulator.classifyLineStatuses(lines, {alloc: allocFactory()});
  }

  function applyUserProgram(){
    const text = getEditorText();
    sandbox.text = text;
    return simulator.applyProgram(text, {alloc: allocFactory()});
  }

  function getProgramOutcome(){
    const lines = getRawLines();
    const status = classifyLineStatuses(lines);
    let hasCompile = status.incomplete.size > 0;
    let hasUb = false;
    if (status.errorKinds){
      for (const kind of status.errorKinds.values()){
        if (kind === 'ub') hasUb = true;
        else hasCompile = true;
      }
    }
    if (hasCompile) return {kind:'compile', state:null};
    if (hasUb) return {kind:'ub', state:null};
    const state = applyUserProgram();
    if (!state) return {kind:'compile', state:null};
    return {kind:'ok', state};
  }

  function renderStage(){
    const stage=$('#sandbox-stage');
    stage.innerHTML='';
    const outcome = getProgramOutcome();
    stage.appendChild(renderState('', outcome.state, outcome.kind));
  }

  function renderState(title, boxes, status='ok'){
    const wrap=document.createElement('div');
    wrap.className='state-panel';
    if (title){
      const heading=document.createElement('div');
      heading.className='state-heading';
      heading.textContent=title;
      wrap.appendChild(heading);
    }
    const grid=document.createElement('div');
    grid.className='grid';
    if (status === 'compile'){
      const msg=document.createElement('div');
      msg.className='muted state-status';
      msg.style.padding='8px';
      msg.textContent='(this code is not valid)';
      grid.appendChild(msg);
    } else if (status === 'ub'){
      const msg=document.createElement('div');
      msg.className='muted state-status';
      msg.style.padding='8px';
      const label=document.createElement('span');
      label.textContent='Kaboom! ';
      msg.appendChild(label);
      const link=document.createElement('button');
      link.type='button';
      link.className='ub-explain-link';
      link.textContent='[What is undefined behavior?]';
      const explain=document.createElement('div');
      explain.className='ub-explain hidden';
      explain.textContent='Undefined behavior means the C standard does not define what happens. The program might crash, act strangely, or appear to work.';
      link.addEventListener('click', (e)=>{
        e.preventDefault();
        e.stopPropagation();
        explain.classList.toggle('hidden');
      });
      msg.appendChild(link);
      msg.appendChild(explain);
      grid.appendChild(msg);
    } else if (boxes===null){
      const msg=document.createElement('div');
      msg.className='muted state-status';
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

  function getLineHeightPx(){
    if (!editor) return 32;
    const style = window.getComputedStyle(editor);
    const lh = parseFloat(style.lineHeight);
    return Number.isFinite(lh) ? lh : 32;
  }

  function autoSizeEditor(){
    if (!editor) return;
    editor.style.height = 'auto';
    editor.style.height = `${editor.scrollHeight}px`;
  }

  function measureWrapCounts(lines){
    if (!editor || !measureEl) return lines.map(()=>1);
    const style = window.getComputedStyle(editor);
    const paddingLeft = parseFloat(style.paddingLeft) || 0;
    const paddingRight = parseFloat(style.paddingRight) || 0;
    const contentWidth = Math.max(1, editor.clientWidth - paddingLeft - paddingRight);
    measureEl.style.width = `${contentWidth}px`;
    measureEl.style.fontFamily = style.fontFamily;
    measureEl.style.fontSize = style.fontSize;
    measureEl.style.fontWeight = style.fontWeight;
    measureEl.style.letterSpacing = style.letterSpacing;
    measureEl.style.lineHeight = style.lineHeight;
    const lineHeight = getLineHeightPx();
    return lines.map(line=>{
      measureEl.textContent = line === '' ? ' ' : line;
      const h = measureEl.scrollHeight;
      return Math.max(1, Math.ceil(h / lineHeight - 0.01));
    });
  }

  function updateLineGutters(){
    autoSizeEditor();
    const lines = getRawLines();
    const count = Math.max(lines.length, 1);
    const lineHeight = getLineHeightPx();
    const wraps = measureWrapCounts(lines);
    if (lineNumbers){
      const frag = document.createDocumentFragment();
      for (let i=1;i<=count;i++){
        const num = document.createElement('div');
        num.className = 'code-line-number';
        num.style.height = `${(wraps[i-1] || 1) * lineHeight}px`;
        num.textContent = String(i);
        frag.appendChild(num);
      }
      lineNumbers.innerHTML = '';
      lineNumbers.appendChild(frag);
      if (editor) lineNumbers.style.height = `${editor.clientHeight}px`;
    }
    if (errorGutter){
      const {invalid, incomplete, errors, errorKinds, info} = classifyLineStatuses(lines);
      const frag = document.createDocumentFragment();
      for (let i=0;i<count;i++){
        const cell = document.createElement('div');
        cell.className = 'code-error-line';
        cell.style.height = `${(wraps[i] || 1) * lineHeight}px`;
        if (invalid.has(i)){
          cell.classList.add('is-invalid');
          const icon = document.createElement('span');
          const kind = errorKinds?.get(i) || 'compile';
          icon.textContent = kind === 'ub' ? '💣' : '🚫';
          icon.title = kind === 'ub' ? 'Line causes undefined behavior' : 'Line does not compile';
          cell.appendChild(icon);
          const baseMessage = errors.get(i) || 'Line has an error.';
          const message = (errorKinds?.get(i) || 'compile') === 'compile' && info?.has(i)
            ? combineMessages(baseMessage, info.get(i))
            : baseMessage;
          const infoBtn = document.createElement('button');
          infoBtn.type = 'button';
          infoBtn.className = 'error-info-btn';
          infoBtn.textContent = 'i';
          infoBtn.setAttribute('aria-label', 'Explain error');
          infoBtn.addEventListener('click', (e)=>{
            e.preventDefault();
            e.stopPropagation();
            const parsed = parseErrorMessage(message);
            if (sandbox.errorDetailLine === i &&
              sandbox.errorDetailMessage === parsed.text &&
              sandbox.errorDetailHtml === parsed.html &&
              sandbox.errorDetailKind === kind){
              hideErrorDetail();
            } else {
              sandbox.errorDetailLine = i;
              showErrorDetail(message, kind);
            }
          });
          cell.appendChild(infoBtn);
        } else if (incomplete.has(i)){
          cell.classList.add('is-incomplete');
          cell.textContent = '...';
          cell.title = 'Line is incomplete';
          if (info?.has(i)){
            const infoMsg = info.get(i);
            const parsed = parseErrorMessage(infoMsg);
            const btn = document.createElement('button');
            btn.type = 'button';
            btn.className = 'error-info-btn';
            btn.textContent = 'i';
            btn.setAttribute('aria-label', 'Explain statement');
            btn.addEventListener('click', (e)=>{
              e.preventDefault();
              e.stopPropagation();
              if (sandbox.errorDetailLine === i &&
                sandbox.errorDetailMessage === parsed.text &&
                sandbox.errorDetailHtml === parsed.html &&
                sandbox.errorDetailKind === 'info'){
                hideErrorDetail();
              } else {
                sandbox.errorDetailLine = i;
                showErrorDetail(infoMsg, 'info');
              }
            });
            cell.appendChild(btn);
          }
        } else if (info?.has(i)){
          const infoMsg = info.get(i);
          const parsed = parseErrorMessage(infoMsg);
          const btn = document.createElement('button');
          btn.type = 'button';
          btn.className = 'error-info-btn';
          btn.textContent = 'i';
          btn.setAttribute('aria-label', 'Explain statement');
          btn.addEventListener('click', (e)=>{
            e.preventDefault();
            e.stopPropagation();
            if (sandbox.errorDetailLine === i &&
              sandbox.errorDetailMessage === parsed.text &&
              sandbox.errorDetailHtml === parsed.html &&
              sandbox.errorDetailKind === 'info'){
              hideErrorDetail();
            } else {
              sandbox.errorDetailLine = i;
              showErrorDetail(infoMsg, 'info');
            }
          });
          cell.appendChild(btn);
        }
        frag.appendChild(cell);
      }
      errorGutter.innerHTML = '';
      errorGutter.appendChild(frag);
      if (editor) errorGutter.style.height = `${editor.clientHeight}px`;
    }
    if (editor){
      if (lineNumbers) lineNumbers.scrollTop = editor.scrollTop;
      if (errorGutter) errorGutter.scrollTop = editor.scrollTop;
    }
  }

  if (editor){
    editor.addEventListener('input', ()=>{
      sandbox.text = editor.value;
      hideErrorDetail();
      updateLineGutters();
      renderStage();
    });
    if (lineNumbers){
      editor.addEventListener('scroll', ()=>{
        lineNumbers.scrollTop = editor.scrollTop;
      });
    }
    if (errorGutter){
      editor.addEventListener('scroll', ()=>{
        errorGutter.scrollTop = editor.scrollTop;
      });
    }
    editor.addEventListener('mouseup', updateLineGutters);
    window.addEventListener('resize', updateLineGutters);
    if (typeof ResizeObserver !== 'undefined'){
      const ro = new ResizeObserver(()=>updateLineGutters());
      ro.observe(editor);
    }
  }

  updateInstructions();
  renderStage();
  updateLineGutters();
})(window.MB);
