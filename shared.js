(function(global){
  function $(selector, root=document){
    return root.querySelector(selector);
  }

  function el(html){
    const t = document.createElement('template');
    t.innerHTML = html.trim();
    return t.content.firstElementChild;
  }

  function typeInfo(type='int'){
    const clean = (type || 'int').trim();
    if (!clean) return {size:4, align:4};
    if (clean.includes('*')) return {size:8, align:8};
    return {size:4, align:4};
  }

  let nextAddr = null;
  function randAddr(type='int'){
    const {size, align} = typeInfo(type);
    if (nextAddr==null){
      const base = 64 + Math.floor(Math.random()*960);
      nextAddr = Math.max(align, Math.ceil(base/align)*align);
    }
    if (nextAddr % align !==0){
      nextAddr = Math.ceil(nextAddr/align)*align;
    }
    const addr = nextAddr;
    nextAddr = addr + size;
    return addr;
  }
  randAddr.reset = function(seed=null){
    nextAddr = seed;
  };

  function isEmptyVal(s){
    return s.trim()==='' || /^empty$/i.test(s.trim());
  }

  function txt(n){
    return (n?.textContent || '').trim();
  }

  function renderCodePane(root, lines, boundary, opts={}){
    root.innerHTML='';
    const code=el('<div class="codecol"></div>');
    root.appendChild(code);
    const addBoundary=()=>code.appendChild(el('<div class="boundary"></div>'));
    const progress = !!opts.progress;
    const progressIndex = (progress && boundary>0) ? boundary-1 : -1;
    if (boundary===0) addBoundary();
    for (let i=0;i<lines.length;i++){
      const lr=el('<div class="line"></div>');
      const ln=el(`<div class="ln">${i+1}</div>`);
      const src=el(`<div class="src">${lines[i]}</div>`);
      if (i<boundary) lr.classList.add('done');
      if (i===progressIndex) lr.classList.add('progress-mid');
      lr.appendChild(ln);
      lr.appendChild(src);
      code.appendChild(lr);
      if (i+1===boundary && i!==progressIndex) addBoundary();
    }
  }

  function renderCodePaneEditable(root, lines, boundary=null){
    root.innerHTML='';
    const code=el('<div class="codecol"></div>');
    root.appendChild(code);
    const addBoundary=()=>code.appendChild(el('<div class="boundary"></div>'));
    if (boundary===0) addBoundary();
    for (let i=0;i<lines.length;i++){
      const line = lines[i] || {};
      const text = line.text ?? '';
      const lr=el('<div class="line code-row"></div>');
      const ln=el(`<div class="ln">${i+1}</div>`);
      const src=el('<div class="src"></div>');
      src.textContent = text;
      if (line.editable){
        src.contentEditable = 'true';
        src.classList.add('code-editable');
        src.dataset.index = String(i);
        const placeholder = line.placeholder ?? '';
        if (!text.trim()){
          src.textContent = placeholder;
          if (placeholder) src.classList.add('placeholder','muted');
        }
        src.dataset.placeholder = placeholder;
      }
      if (boundary!=null && i<boundary) lr.classList.add('done');
      lr.appendChild(ln);
      lr.appendChild(src);
      code.appendChild(lr);
      if (boundary!=null && i+1===boundary) addBoundary();
    }
  }

  function readEditableCodeLines(root){
    if (!root) return {};
    const map={};
    root.querySelectorAll('.code-editable').forEach(el=>{
      const idx = Number(el.dataset.index || -1);
      if (idx>=0){
        const content = txt(el);
        map[idx] = content;
      }
    });
    return map;
  }

  function parseSimpleStatement(src=''){
    const raw = (src || '').replace(/\u00a0/g,' ');
    const s = raw.replace(/\s+/g,' ').trim();
    if (!s) return null;
    let m = s.match(/^int\s*(\*{0,2})\s*([A-Za-z_][A-Za-z0-9_]*)\s*;$/);
    if (m){
      const stars = m[1] || '';
      const type = stars==='**' ? 'int**' : (stars==='*' ? 'int*' : 'int');
      return {kind:'decl', name:m[2], type};
    }
    m = s.match(/^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(-?\d+)\s*;$/);
    if (m) return {kind:'assign', name:m[1], value:m[2], valueKind:'num'};
    m = s.match(/^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*&\s*([A-Za-z_][A-Za-z0-9_]*)\s*;$/);
    if (m) return {kind:'assignRef', name:m[1], ref:m[2]};
    m = s.match(/^\*\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(-?\d+)\s*;$/);
    if (m) return {kind:'assignDeref', name:m[1], value:m[2]};
    return null;
  }

  function applySimpleStatement(state=[], stmt, opts={}){
    const {
      alloc=type=>String(randAddr(type||'int')),
      allowRedeclare=true
    } = opts;
    if (!stmt) return cloneBoxes(state);
    const boxes = cloneBoxes(state);
    const by = Object.fromEntries(boxes.map(b=>[b.name,b]));
    if (stmt.kind==='decl'){
      const type = stmt.type || 'int';
      if (by[stmt.name] && !allowRedeclare) return null;
      if (!by[stmt.name]){
        boxes.push({name:stmt.name, type, value:'empty', address:alloc(type)});
      }
      return boxes;
    }
    if (stmt.kind==='assign' || stmt.kind==='assignRef'){
      const target = by[stmt.name];
      if (!target) return null;
      if (stmt.kind==='assign'){
        if (target.type!=='int') return null;
        target.value = String(stmt.value);
        return boxes;
      }
      // assignRef
      if (target.type!=='int*' && target.type!=='int**') return null;
      const refBox = by[stmt.ref];
      if (!refBox || !refBox.address) return null;
      target.value = String(refBox.address);
      const alias = `*${stmt.name}`;
      const names = refBox.names || [refBox.name].filter(Boolean);
      if (!names.includes(alias)) names.push(alias);
      refBox.names = names;
      return boxes;
    }
    if (stmt.kind==='assignDeref'){
      const ptr = by[stmt.name];
      if (!ptr || ptr.type!=='int*' || !ptr.value || ptr.value==='empty') return null;
      const targetBox = boxes.find(b=>b.address===String(ptr.value));
      if (!targetBox) return null;
      targetBox.value = String(stmt.value);
      const alias = `*${stmt.name}`;
      const names = targetBox.names || [targetBox.name].filter(Boolean);
      if (!names.includes(alias)) names.push(alias);
      targetBox.names = names;
      return boxes;
    }
    return boxes;
  }

  function vbox({addr='—', type='int', value='empty', name='', names=null, editable=false, allowNameAdd=false, allowNameDelete=false, allowNameEdit=false, allowTypeEdit=false, allowNameToggle=false}={}){
    const emptyDisplay = isEmptyVal(String(value||''));
    const displayValue = emptyDisplay ? '' : value;
    const resolvedNames = Array.isArray(names) && names.length ? names : (Array.isArray(name) ? name : [name]);
    const namesList = resolvedNames.filter(n=>n!==undefined && n!==null).map(n=>String(n));
    const canToggleNames = allowNameToggle && namesList.length>1;
    const valueClasses = `value ${editable?'editable':''} ${emptyDisplay?'placeholder muted':''}`;
    const typeClasses  = `type ${allowTypeEdit?'editable':''}`;
    const nameClasses  = `name-tag ${editable?'editable':''}`;
    const listClasses  = `name-list${canToggleNames ? ' collapsible' : ''}`;
    const toggleBtn = canToggleNames
      ? '<button class="name-toggle" type="button" aria-expanded="false">Show aliases</button>'
      : '';
    const addBtn = allowNameAdd ? '<button class="name-add" type="button" title="Add name">+</button>' : '';
    const nameTags = namesList.map((n, idx)=>{
      const extraClass = canToggleNames && idx>0 ? ' name-extra' : '';
      const cls = namesList.length>1 ? `${nameClasses}${extraClass}` : `${nameClasses} single`;
      const del = allowNameDelete ? `<button class="name-del" data-index="${idx}" type="button" title="Delete name">×</button>` : '';
      return `<span class="${cls}"><span class="name-text">${n}</span>${del}</span>`;
    }).join('');
    const namesHtml = `${nameTags}${addBtn}${toggleBtn}`;

    const node = el(`
      <div class="vbox ${editable?'is-editable':''}">
        <div class="lbl lbl-addr">address</div>
        <div class="addr">${addr}</div>
          <div class="cell">
          <div class="lbl lbl-value">value</div>
          <div class="${valueClasses}">${displayValue}</div>
        </div>
        <div class="lbl lbl-type">type</div>
        <div class="${typeClasses}">${type}</div>
        <div class="${listClasses}">
          <div class="name-list-inner">${namesHtml}</div>
        </div>
        <div class="lbl lbl-name">${namesList.length>1 ? 'name(s)' : 'name'}</div>
      </div>
    `);

    if (editable){
      node.querySelector('.value').setAttribute('contenteditable','true');
      if (allowTypeEdit){
        const typeEl=node.querySelector('.type');
        typeEl.setAttribute('contenteditable','true');
        typeEl.classList.add('editable');
      }
      // Names remain read-only for existing boxes; add only for new via allowNameAdd.
      const addBtn=node.querySelector('.name-add');
      if (addBtn){
        addBtn.onclick=()=>{
          const extraClass = canToggleNames ? ' name-extra' : '';
          const span=el(`<span class="${nameClasses}${extraClass}"><span class="name-text"></span>${allowNameDelete?'<button class="name-del" type="button" title="Delete name">×</button>':''}</span>`);
          addBtn.before(span);
          const textEl = span.querySelector('.name-text');
          if (textEl){
            if (allowNameEdit){
              textEl.setAttribute('contenteditable','true');
              textEl.classList.add('editable');
            }
            textEl.focus();
          }
          const delBtn = span.querySelector('.name-del');
          if (delBtn){
            delBtn.onclick=()=>span.remove();
          }
        };
      }
      node.querySelectorAll('.name-text').forEach(el=>{
        if (allowNameEdit){
          el.setAttribute('contenteditable','true');
          el.classList.add('editable');
        }
      });
    }
    if (canToggleNames){
      const list = node.querySelector('.name-list');
      const inner = node.querySelector('.name-list-inner');
      const toggle = node.querySelector('.name-toggle');
      if (list && toggle && inner){
        const clampNames = ()=>{
          inner.style.transform = '';
          if (!list.classList.contains('expanded')) return;
          if (!list.isConnected) return;
          const listRect = list.getBoundingClientRect();
          const innerRect = inner.getBoundingClientRect();
          if (innerRect.left < listRect.left){
            const shift = listRect.left - innerRect.left;
            inner.style.transform = `translateX(${shift}px)`;
          }
        };
        const setExpanded = expanded=>{
          list.classList.toggle('expanded', expanded);
          toggle.setAttribute('aria-expanded', expanded ? 'true' : 'false');
          toggle.textContent = expanded ? 'Hide aliases' : 'Show aliases';
          requestAnimationFrame(clampNames);
        };
        toggle.onclick=()=>setExpanded(!list.classList.contains('expanded'));
        setExpanded(false);
      }
    }
    return node;
  }

  function disableBoxEditing(root){
    if (!root) return;
    root.querySelectorAll('.value.editable, .type.editable, .name-text.editable').forEach(el=>{
      el.removeAttribute('contenteditable');
      el.classList.remove('editable');
    });
    root.querySelectorAll('.name-add, .name-del').forEach(btn=>btn.remove());
    root.classList.remove('is-editable');
  }

  function removeBoxDeleteButtons(root){
    const scope = root || document;
    scope.querySelectorAll('.vbox .delete').forEach(btn=>btn.remove());
  }

  function readBoxState(root){
    if (!root) return null;
    const names = [...root.querySelectorAll('.name-text')].map(el=>txt(el)).filter(Boolean);
    const valEl = root.querySelector('.value');
    const valText = txt(valEl);
    const value = (valEl?.classList?.contains('placeholder') && valText==='') ? 'empty' : valText;
    return {
      addr: txt(root.querySelector('.addr')),
      type: txt(root.querySelector('.type')),
      value,
      name: names[0] || '',
      names,
      nameEditable: !!root.querySelector('.name-text[contenteditable]'),
      typeEditable: !!root.querySelector('.type[contenteditable]')
    };
  }

  const addrPool = {free:[]};
  function nextPooledAddr(type='int'){
    if (addrPool.free.length) return addrPool.free.pop();
    return randAddr(type);
  }

  function makeAnswerBox({name='', names=null, type='', value='empty', address=null, editable=true, deletable=editable, allowNameAdd=false, allowNameToggle=false, allowNameEdit=null, allowTypeEdit=null, nameEditable=null, typeEditable=null}={}){
    const resolvedAddr = address==null ? String(nextPooledAddr(type || 'int')) : String(address);
    const resolvedNameEdit = (allowNameEdit!==null && allowNameEdit!==undefined)
      ? allowNameEdit
      : (nameEditable!==null && nameEditable!==undefined)
        ? nameEditable
        : (!name && !(Array.isArray(names) && names.length));
    const resolvedTypeEdit = (allowTypeEdit!==null && allowTypeEdit!==undefined)
      ? allowTypeEdit
      : (typeEditable!==null && typeEditable!==undefined)
        ? typeEditable
        : (!type);
    const node=vbox({
      addr:resolvedAddr,
      type,
      value,
      name,
      names,
      editable,
      allowNameAdd,
      allowNameToggle,
      allowNameDelete:allowNameAdd,
      allowNameEdit:resolvedNameEdit,
      allowTypeEdit:resolvedTypeEdit
    });
    if (deletable){
      const del=el('<button class="delete" title="delete">×</button>');
      node.appendChild(del);
      del.onclick=()=>{
        const addrTxt = txt(node.querySelector('.addr'));
        if (addrTxt) addrPool.free.push(addrTxt);
        node.remove();
      };
    }
    return node;
  }

  function cloneStateBoxes(state){
    if (!Array.isArray(state)) return [];
    return state
      .filter(Boolean)
      .map(st=>{
        const addr = st.addr ?? st.address ?? null;
        return {
          name:st.name || '',
          names: Array.isArray(st.names) ? [...st.names] : (st.names ? [st.names] : []),
          type:st.type || '',
          value:st.value ?? '',
          address: addr==null ? null : String(addr),
          nameEditable: st.nameEditable ?? st.allowNameEdit ?? null,
          typeEditable: st.typeEditable ?? st.allowTypeEdit ?? null
        };
      })
      .filter(st=>st.name);
  }

  function ensureBox(list, spec){
    const item = list.find(b=>b.name===spec.name);
    if (!item){
      list.push({
        name:spec.name,
        type:spec.type ?? '',
        value:spec.value ?? '',
        address: spec.address==null ? null : String(spec.address)
      });
      return;
    }
    if (spec.address!=null) item.address = String(spec.address);
    if (spec.type && !item.type) item.type = spec.type;
    if (spec.value!=null && (item.value==null || item.value==='')) item.value = spec.value;
  }

  function cloneBoxes(list){
    return Array.isArray(list)
      ? list.map(b=>({
          ...b,
          names: Array.isArray(b.names) ? [...b.names] : (b.names ? [b.names] : (b.name ? [b.name] : []))
        }))
      : [];
  }

  function firstNonEmptyClone(...states){
    for (const st of states){
      const clone = cloneStateBoxes(st);
      if (clone.length) return clone;
    }
    return [];
  }

  function serializeWorkspace(id){
    const ws = document.getElementById(id);
    if (!ws) return null;
    return [...ws.querySelectorAll('.vbox')].map(v=>readBoxState(v));
  }

  function restoreWorkspace(state, defaults, workspaceId, opts={}){
    const {editable=true,deletable=editable, allowNameAdd=false, allowNameToggle=false, allowNameEdit=null, allowTypeEdit=null} = opts;
    const wrap=el(`<div class="grid" id="${workspaceId}"></div>`);
    if (Array.isArray(state) && state.length){
      state.forEach(st=>{
        const node=makeAnswerBox({
          name:st.name,
          names:st.names,
          type:st.type,
          value:st.value,
          address:st.addr ?? st.address ?? null,
          editable,
          deletable,
          allowNameAdd,
          allowNameToggle,
          allowNameEdit: allowNameEdit ?? st.nameEditable ?? st.allowNameEdit,
          allowTypeEdit: allowTypeEdit ?? st.typeEditable ?? st.allowTypeEdit
        });
        if (isEmptyVal(st.value)) node.querySelector('.value').classList.add('placeholder','muted');
        wrap.appendChild(node);
      });
    } else if (Array.isArray(defaults)) {
      defaults.forEach(d=>{
        const node=makeAnswerBox({
          name:d.name,
          names:d.names,
          type:d.type,
          value:d.value,
          address:d.address ?? d.addr ?? null,
          editable,
          deletable,
          allowNameAdd,
          allowNameToggle,
          allowNameEdit: allowNameEdit ?? d.nameEditable ?? d.allowNameEdit,
          allowTypeEdit: allowTypeEdit ?? d.typeEditable ?? d.allowTypeEdit
        });
        if (isEmptyVal(d.value)) node.querySelector('.value').classList.add('placeholder','muted');
        wrap.appendChild(node);
      });
    }
    return wrap;
  }

  function setHintContent(panel, message){
    if (!panel) return;
    if (message && typeof message==='object' && Object.prototype.hasOwnProperty.call(message,'html')){
      panel.innerHTML = message.html || '';
    } else {
      panel.textContent = typeof message==='string' ? message : '';
    }
  }

  function resolveElement(ref){
    if (!ref) return null;
    if (typeof ref==='string'){
      if (ref.startsWith('#') || ref.startsWith('.')){
        return document.querySelector(ref);
      }
      return document.getElementById(ref);
    }
    return ref;
  }

  function createHintController({button, panel, build}={}){
    const buttonEl = resolveElement(button);
    const panelEl  = resolveElement(panel);

    function hide(){
      if (!panelEl) return;
      panelEl.textContent='';
      panelEl.classList.add('hidden');
    }

    function show(message){
      if (!panelEl) return;
      const content = (typeof message==='undefined' && typeof build==='function')
        ? build()
        : message;
      setHintContent(panelEl, content ?? '');
      panelEl.classList.remove('hidden');
      flashStatus(panelEl);
    }

    if (buttonEl){
      buttonEl.addEventListener('click', ()=>{
        show(typeof build==='function' ? build() : undefined);
      });
    }

    function setButtonHidden(hidden){
      if (!buttonEl) return;
      buttonEl.classList.toggle('hidden', !!hidden);
    }

    return {
      button: buttonEl,
      panel: panelEl,
      show,
      hide,
      setButtonHidden
    };
  }

  function createStepper({
    prefix,
    lines=[],
    nextPage=null,
    getBoundary,
    setBoundary,
    onBeforeChange,
    onAfterChange,
    isStepLocked
  }={}){
    if (!prefix) throw new Error('createStepper requires a prefix (e.g., "p1").');
    const prevBtn = document.getElementById(`${prefix}-prev`);
    const nextBtn = document.getElementById(`${prefix}-next`);
    const total = Array.isArray(lines) ? lines.length : Math.max(0, Number(lines) || 0);

    function clearPulse(){
      nextBtn?.classList.remove('pulse-success');
    }

    function boundary(){
      return typeof getBoundary==='function' ? getBoundary() : 0;
    }

    function setBoundaryValue(value){
      if (typeof setBoundary==='function') setBoundary(value);
    }

    function locked(at){
      return typeof isStepLocked==='function' ? !!isStepLocked(at, at===total) : false;
    }

    function update(){
      const current = boundary();
      if (prevBtn) prevBtn.disabled = (current===0);
      if (nextBtn){
        const atEnd = (current===total);
        nextBtn.textContent = atEnd ? 'Next Program ▶▶' : 'Next ▶';
        nextBtn.disabled = locked(current);
      }
    }

    function goTo(target){
      const current = boundary();
      const clamped = Math.max(0, Math.min(total, target));
      if (clamped===current) return;
      onBeforeChange?.(current);
      setBoundaryValue(clamped);
      onAfterChange?.(clamped);
      update();
    }

    prevBtn?.addEventListener('click', ()=>{
      if (boundary()===0) return;
      clearPulse();
      goTo(boundary()-1);
    });

    nextBtn?.addEventListener('click', ()=>{
      const current = boundary();
      clearPulse();
      if (current===total){
        if (!nextBtn?.disabled && nextPage) window.location.href = nextPage;
        return;
      }
      if (locked(current)) return;
      goTo(current+1);
    });

    update();

    return {
      update,
      goTo,
      boundary,
      clearPulse
    };
  }

  function pulseNextButton(prefix){
    const btn=document.getElementById(`${prefix}-next`);
    if (!btn) return;
    btn.classList.add('pulse-success');
  }

  document.addEventListener('focusin', e=>{
    const t=e.target;
    if (t.classList?.contains('code-editable') && t.classList.contains('placeholder')){
      const placeholder = t.dataset?.placeholder || '';
      if (txt(t)===placeholder){
        t.textContent='';
        t.classList.remove('placeholder','muted');
      }
    }
    if (t.classList?.contains('placeholder')){
      if (txt(t)===''){
        t.classList.add('muted');
      } else {
        t.classList.remove('muted');
      }
    }
  });

  document.addEventListener('input', e=>{
    const t=e.target;
    if (t.classList?.contains('placeholder')){
      if (txt(t)==='') t.classList.add('muted');
      else t.classList.remove('muted');
    }
  });

  document.addEventListener('keydown', e=>{
    if (e.key!=='Enter') return;
    const t=e.target;
    if (!t?.isContentEditable) return;
    if (t.classList?.contains('value') || t.classList?.contains('type') || t.classList?.contains('name-text')){
      e.preventDefault();
      t.blur();
    }
  });

  document.addEventListener('focusout', e=>{
    const t=e.target;
    if (t.classList?.contains('code-editable')){
      const placeholder = t.dataset?.placeholder || '';
      if (!txt(t)){
        t.textContent=placeholder;
        if (placeholder) t.classList.add('placeholder','muted');
      }
    }
    if (t.classList?.contains('placeholder') && txt(t)===''){
      t.textContent='';
      t.classList.add('muted');
    }
  });

  function initScrollHint(){
    const btn = el('<button class="scroll-down-btn hidden" aria-label="Scroll to bottom">↓</button>');
    document.body.appendChild(btn);

    const shouldShow = ()=>{
      const doc = document.documentElement;
      const scrollable = doc.scrollHeight - window.innerHeight;
      const nearBottom = window.scrollY > scrollable - 140;
      return scrollable > 200 && !nearBottom;
    };

    const update = ()=>{
      btn.classList.toggle('hidden', !shouldShow());
    };

    window.addEventListener('scroll', update, {passive:true});
    window.addEventListener('resize', update);
    const observer = new MutationObserver(update);
    observer.observe(document.body, {childList:true, subtree:true});
    btn.addEventListener('click', ()=>{
      window.scrollTo({top:document.documentElement.scrollHeight, behavior:'smooth'});
    });
    update();
  }

  if (document.readyState === 'loading'){
    document.addEventListener('DOMContentLoaded', initScrollHint);
  } else {
    initScrollHint();
  }

  function initInstructionWatcher(){
    const seen = new Map();
    document.querySelectorAll('.intro').forEach(el=>{
      seen.set(el, el.innerHTML);
      const obs = new MutationObserver(()=>{
        const prev = seen.get(el) || '';
        const curr = el.innerHTML;
        if (curr===prev) return;
        seen.set(el, curr);
        const text = (el.textContent || '').trim();
        if (text){
          window.scrollTo({top:0, behavior:'smooth'});
        }
      });
      obs.observe(el, {childList:true, characterData:true, subtree:true});
    });
  }

  if (document.readyState === 'loading'){
    document.addEventListener('DOMContentLoaded', initInstructionWatcher, {once:true});
  } else {
    initInstructionWatcher();
  }

  function flashStatus(el){
    if (!el) return;
    el.classList.remove('status-flash');
    // force reflow to restart animation
    void el.offsetWidth;
    el.classList.add('status-flash');
  }

  global.MB = {
    $, el, randAddr, isEmptyVal, txt,
    renderCodePane, renderCodePaneEditable, readEditableCodeLines,
    parseSimpleStatement, applySimpleStatement,
    vbox, readBoxState, makeAnswerBox,
    cloneStateBoxes, ensureBox, cloneBoxes, firstNonEmptyClone,
    serializeWorkspace, restoreWorkspace, setHintContent,
    createHintController, createStepper, pulseNextButton, flashStatus, disableBoxEditing, removeBoxDeleteButtons
  };
})(window);
