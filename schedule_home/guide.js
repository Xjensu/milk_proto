(() => {
  const $ = (selector, root = document) => root?.querySelector(selector);
  const $$ = (selector, root = document) => [...(root?.querySelectorAll(selector) || [])];

  const book = $('#guideBook');
  const sheetsRoot = $('#bookSheets');
  const pageIndex = $('#bookPageIndex');
  const progressRoot = $('#bookProgress');
  const pageCount = 6;
  const sheetCount = Math.ceil(pageCount / 2);
  const sheets = [];
  const sheetState = [];

  let spread = 0; // number of sheets already turned to the left
  let active = null;
  let locked = false;
  let dragFrame = 0;

  const clamp01 = (value) => Math.max(0, Math.min(1, value));
  const easeInOutCubic = (t) => t < .5 ? 4 * t * t * t : 1 - ((-2 * t + 2) ** 3) / 2;
  const easeOutQuart = (t) => 1 - ((1 - t) ** 4);

  function cloneTemplate(page) {
    const template = $(`#guidePage${page}`);
    if (!template) return document.createTextNode('');
    return template.content.cloneNode(true);
  }

  function makeSheet(index) {
    const sheet = document.createElement('div');
    sheet.className = 'book-sheet';
    sheet.dataset.sheet = String(index);

    const front = document.createElement('section');
    front.className = 'book-sheet-face book-sheet-front';
    front.dataset.page = String(index * 2);
    front.appendChild(cloneTemplate(index * 2));

    const back = document.createElement('section');
    back.className = 'book-sheet-face book-sheet-back';
    back.dataset.page = String(index * 2 + 1);
    if (index * 2 + 1 < pageCount) back.appendChild(cloneTemplate(index * 2 + 1));

    const edge = document.createElement('i');
    edge.className = 'book-sheet-edge';
    edge.setAttribute('aria-hidden', 'true');

    sheet.append(front, back, edge);
    sheetsRoot.appendChild(sheet);
    sheets.push(sheet);
    sheetState.push({ progress: 0, animationFrame: 0, animationToken: 0 });
    applySheetVisual(index, 0);
  }

  function renderProgress() {
    progressRoot.replaceChildren();
    for (let i = 0; i < pageCount; i += 1) {
      const mark = document.createElement('i');
      mark.dataset.page = String(i);
      progressRoot.appendChild(mark);
    }
  }

  function visiblePages() {
    if (spread === 0) return [0];
    if (spread === sheetCount) return [pageCount - 1];
    return [spread * 2 - 1, spread * 2];
  }

  function updateMeta() {
    const visible = visiblePages();
    pageIndex.textContent = visible.length === 1
      ? String(visible[0] + 1).padStart(2, '0')
      : `${String(visible[0] + 1).padStart(2, '0')}—${String(visible[1] + 1).padStart(2, '0')}`;

    const visibleSet = new Set(visible);
    $$('#bookProgress i', progressRoot).forEach((mark, index) => {
      mark.classList.toggle('is-active', visibleSet.has(index));
    });

    sheets.forEach((sheet) => {
      const front = $('.book-sheet-front', sheet);
      const back = $('.book-sheet-back', sheet);
      const frontPage = Number(front?.dataset.page);
      const backPage = Number(back?.dataset.page);
      front?.setAttribute('aria-hidden', visibleSet.has(frontPage) ? 'false' : 'true');
      back?.setAttribute('aria-hidden', visibleSet.has(backPage) ? 'false' : 'true');
    });
  }

  function applyBookFold(progress = 0) {
    const p = clamp01(progress);
    const arc = Math.sin(Math.PI * p);
    book?.style.setProperty('--book-fold', arc.toFixed(4));
    book?.style.setProperty('--book-fold-side', p < .5 ? '1' : '-1');
    book?.style.setProperty('--spine-width', `${(18 + arc * 10).toFixed(2)}px`);
    book?.style.setProperty('--spine-z', `${(5 + arc * 3).toFixed(2)}px`);
    book?.style.setProperty('--spine-blur', `${(arc * .35).toFixed(2)}px`);
  }

  function applySheetVisual(sheetIndex, progress) {
    const sheet = sheets[sheetIndex];
    if (!sheet) return;

    const p = clamp01(progress);
    const arc = Math.sin(Math.PI * p);
    const phase = Math.sin(Math.PI * 2 * p);
    const edgeCurl = Math.sin(Math.PI * Math.min(1, p * 1.35)) * Math.sin(Math.PI * Math.min(1, (1 - p) * 1.35));
    const angle = -180 * p;
    const lift = arc * 24;
    const skew = phase * 1.15;
    const tilt = phase * .42;
    const scaleX = 1 - arc * .018;
    const shiftY = -arc * 2.4;
    const shade = Math.pow(arc, .82);
    const foldX = 100 - (p * 100);
    const contentShift = phase * 1.7;
    const shadowX = -6 - arc * 22;
    const shadowY = 7 + arc * 6;
    const shadowBlur = 9 + arc * 18;

    sheetState[sheetIndex].progress = p;
    sheet.style.setProperty('--angle', `${angle.toFixed(3)}deg`);
    sheet.style.setProperty('--lift', `${lift.toFixed(2)}px`);
    sheet.style.setProperty('--skew', `${skew.toFixed(3)}deg`);
    sheet.style.setProperty('--tilt', `${tilt.toFixed(3)}deg`);
    sheet.style.setProperty('--scale-x', scaleX.toFixed(5));
    sheet.style.setProperty('--shift-y', `${shiftY.toFixed(2)}px`);
    sheet.style.setProperty('--shade', shade.toFixed(4));
    sheet.style.setProperty('--curl', edgeCurl.toFixed(4));
    sheet.style.setProperty('--fold-x', `${foldX.toFixed(2)}%`);
    sheet.style.setProperty('--content-shift', `${contentShift.toFixed(2)}px`);
    sheet.style.setProperty('--content-shift-back', `${(-contentShift).toFixed(2)}px`);
    sheet.style.setProperty('--content-scale', (1 - arc * .008).toFixed(5));
    sheet.style.setProperty('--face-overlay-opacity', (.10 + shade * .72).toFixed(4));
    sheet.style.setProperty('--edge-overlay-opacity', (.08 + shade * .84).toFixed(4));
    sheet.style.setProperty('--corner-radius', `${(edgeCurl * 13).toFixed(2)}px`);
    sheet.style.setProperty('--corner-radius-small', `${(edgeCurl * 10).toFixed(2)}px`);
    sheet.style.setProperty('--edge-width', `${(2 + edgeCurl * 4).toFixed(2)}px`);
    sheet.style.setProperty('--edge-blur', `${(edgeCurl * 8).toFixed(2)}px`);
    sheet.style.setProperty('--cast-shift', `${(-edgeCurl * 8).toFixed(2)}px`);
    sheet.style.setProperty('--cast-blur', `${(5 + shade * 13).toFixed(2)}px`);
    sheet.style.setProperty('--cast-opacity', (shade * .55).toFixed(4));
    sheet.style.setProperty('--shadow-x', `${shadowX.toFixed(2)}px`);
    sheet.style.setProperty('--shadow-y', `${shadowY.toFixed(2)}px`);
    sheet.style.setProperty('--shadow-blur', `${shadowBlur.toFixed(2)}px`);
    applyBookFold(p);
  }

  function cancelSheetAnimation(sheetIndex) {
    const state = sheetState[sheetIndex];
    if (!state) return;
    state.animationToken += 1;
    if (state.animationFrame) cancelAnimationFrame(state.animationFrame);
    state.animationFrame = 0;
  }

  function animateSheet(sheetIndex, target, options = {}) {
    const state = sheetState[sheetIndex];
    const sheet = sheets[sheetIndex];
    if (!state || !sheet) return Promise.resolve();

    cancelSheetAnimation(sheetIndex);
    const token = state.animationToken;
    const from = state.progress;
    const distance = Math.abs(target - from);
    const duration = options.duration ?? Math.max(280, 630 * distance + 160);
    const easing = options.easing || easeInOutCubic;
    const start = performance.now();

    sheet.classList.remove('is-dragging');
    sheet.classList.add('is-settling');
    book?.classList.add('is-turning');

    return new Promise((resolve) => {
      const tick = (now) => {
        if (state.animationToken !== token) {
          resolve(false);
          return;
        }
        const elapsed = now - start;
        const t = clamp01(elapsed / duration);
        const eased = easing(t);
        applySheetVisual(sheetIndex, from + (target - from) * eased);

        if (t < 1) {
          state.animationFrame = requestAnimationFrame(tick);
          return;
        }

        state.animationFrame = 0;
        applySheetVisual(sheetIndex, target);
        sheet.classList.remove('is-settling');
        book?.classList.remove('is-turning');
        applyBookFold(0);
        resolve(true);
      };
      state.animationFrame = requestAnimationFrame(tick);
    });
  }

  function updateStacking(activeIndex = -1) {
    sheets.forEach((sheet, index) => {
      if (index === activeIndex) {
        sheet.style.zIndex = '60';
        return;
      }
      if (index < spread) {
        sheet.style.zIndex = String(18 + index);
      } else {
        sheet.style.zIndex = String(45 - index);
      }
    });
  }

  async function turnForward() {
    if (locked || active || spread >= sheetCount) return;
    locked = true;
    const sheetIndex = spread;
    updateStacking(sheetIndex);
    await animateSheet(sheetIndex, 1, { duration: 720, easing: easeOutQuart });
    spread += 1;
    updateStacking();
    updateMeta();
    locked = false;
  }

  async function turnBackward() {
    if (locked || active || spread <= 0) return;
    locked = true;
    const sheetIndex = spread - 1;
    updateStacking(sheetIndex);
    await animateSheet(sheetIndex, 0, { duration: 720, easing: easeOutQuart });
    spread -= 1;
    updateStacking();
    updateMeta();
    locked = false;
  }

  function pointerMode(event) {
    if (!book || locked) return null;
    const rect = book.getBoundingClientRect();
    const x = event.clientX - rect.left;
    const center = rect.width / 2;
    if (x >= center && spread < sheetCount) {
      return { direction: 'forward', sheetIndex: spread, rect, x, center };
    }
    if (x < center && spread > 0) {
      return { direction: 'backward', sheetIndex: spread - 1, rect, x, center };
    }
    return null;
  }

  function beginDrag(event) {
    if ((event.pointerType === 'mouse' && event.button !== 0) || locked || active) return;
    const mode = pointerMode(event);
    if (!mode) return;

    cancelSheetAnimation(mode.sheetIndex);
    const sheet = sheets[mode.sheetIndex];
    const initialProgress = mode.direction === 'forward' ? 0 : 1;
    applySheetVisual(mode.sheetIndex, initialProgress);

    active = {
      ...mode,
      sheet,
      pointerId: event.pointerId,
      startX: event.clientX,
      lastX: event.clientX,
      lastTime: performance.now(),
      velocity: 0,
      progress: initialProgress,
      pendingProgress: initialProgress,
      moved: 0,
    };

    updateStacking(mode.sheetIndex);
    sheet.classList.add('is-dragging');
    book.classList.add('is-dragging');
    document.body.classList.add('book-dragging');
    book.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  }

  function flushDragFrame() {
    dragFrame = 0;
    if (!active) return;
    active.progress = active.pendingProgress;
    applySheetVisual(active.sheetIndex, active.progress);
  }

  function moveDrag(event) {
    if (!active || event.pointerId !== active.pointerId) return;
    const samples = event.getCoalescedEvents?.() || [event];
    const sample = samples[samples.length - 1] || event;
    const now = performance.now();
    const dt = Math.max(1, now - active.lastTime);
    const instantVelocity = (sample.clientX - active.lastX) / dt;
    active.velocity = active.velocity * .68 + instantVelocity * .32;
    active.lastX = sample.clientX;
    active.lastTime = now;

    const dx = sample.clientX - active.startX;
    active.moved = Math.max(active.moved, Math.abs(dx));

    if (active.direction === 'forward') {
      const travel = Math.max(120, active.startX - (active.rect.left + active.center));
      active.pendingProgress = clamp01(-dx / travel);
    } else {
      const travel = Math.max(120, (active.rect.left + active.center) - active.startX);
      active.pendingProgress = clamp01(1 - (dx / travel));
    }

    if (!dragFrame) dragFrame = requestAnimationFrame(flushDragFrame);
    event.preventDefault();
  }

  async function finishDrag(event, cancelled = false) {
    if (!active || (event && event.pointerId !== active.pointerId)) return;

    if (dragFrame) {
      cancelAnimationFrame(dragFrame);
      dragFrame = 0;
      active.progress = active.pendingProgress;
      applySheetVisual(active.sheetIndex, active.progress);
    }

    const state = active;
    active = null;
    book.classList.remove('is-dragging');
    document.body.classList.remove('book-dragging');
    try { book.releasePointerCapture?.(state.pointerId); } catch {}

    const rect = state.rect;
    const clickForward = state.direction === 'forward' && state.moved < 7 && state.x > rect.width * .88;
    const clickBackward = state.direction === 'backward' && state.moved < 7 && state.x < rect.width * .12;

    let commit = false;
    if (!cancelled) {
      if (state.direction === 'forward') {
        commit = state.progress > .36 || state.velocity < -.34 || clickForward;
      } else {
        commit = state.progress < .64 || state.velocity > .34 || clickBackward;
      }
    }

    const target = state.direction === 'forward'
      ? (commit ? 1 : 0)
      : (commit ? 0 : 1);

    // The logical spread is switched immediately on a successful manual turn.
    // This makes the page behind the moving sheet and the page counter update at
    // the same moment the user releases it instead of waiting for the settle tween.
    if (commit) {
      spread += state.direction === 'forward' ? 1 : -1;
      updateMeta();
    }

    locked = true;
    updateStacking(state.sheetIndex);
    await animateSheet(state.sheetIndex, target, {
      duration: Math.max(260, 610 * Math.abs(target - state.progress) + 120),
      easing: easeOutQuart,
    });
    updateStacking();
    locked = false;
  }

  for (let i = 0; i < sheetCount; i += 1) makeSheet(i);
  renderProgress();
  updateStacking();
  updateMeta();

  book?.addEventListener('pointerdown', beginDrag);
  book?.addEventListener('pointermove', moveDrag);
  book?.addEventListener('pointerup', (event) => finishDrag(event, false));
  book?.addEventListener('pointercancel', (event) => finishDrag(event, true));

  book?.addEventListener('dblclick', (event) => {
    if (locked || active) return;
    const rect = book.getBoundingClientRect();
    const x = event.clientX - rect.left;
    if (x >= rect.width / 2) turnForward();
    else turnBackward();
  });

  addEventListener('keydown', (event) => {
    if (event.key === 'ArrowRight' || event.key === 'PageDown') {
      event.preventDefault();
      turnForward();
    } else if (event.key === 'ArrowLeft' || event.key === 'PageUp') {
      event.preventDefault();
      turnBackward();
    }
  });

  // Dialogs shared with the rest of the site.
  const profileButton = $('.profile-button');
  const authDialog = $('#authDialog');
  const authPanel = $('.auth-panel', authDialog);
  const authClose = $('.auth-close', authDialog);
  const authForm = $('#authForm');
  const contactsButton = $('#contactsButton');
  const contactsDialog = $('#contactsDialog');
  const contactsPanel = $('.contacts-panel', contactsDialog);
  const contactsClose = $('.contacts-close', contactsDialog);
  const cookieButton = $('#cookieButton');
  const cookieDialog = $('#cookieDialog');
  const cookiePanel = $('.cookie-panel', cookieDialog);
  const cookieClose = $('.cookie-close', cookieDialog);
  const functionalCookies = $('#functionalCookies');
  const analyticsCookies = $('#analyticsCookies');
  const necessaryOnly = $('#necessaryOnly');
  const acceptAllCookies = $('#acceptAllCookies');
  const saveCookies = $('#saveCookies');

  const dialogs = [authDialog, contactsDialog, cookieDialog].filter(Boolean);
  function syncModalState() {
    document.body.classList.toggle('modal-open', dialogs.some((dialog) => dialog.classList.contains('is-open')));
  }
  function closeDialog(dialog, trigger) {
    if (!dialog) return;
    dialog.classList.remove('is-open');
    dialog.setAttribute('aria-hidden', 'true');
    trigger?.setAttribute('aria-expanded', 'false');
    syncModalState();
  }
  function openDialog(dialog, trigger, focusTarget) {
    dialogs.forEach((other) => other !== dialog && closeDialog(other));
    dialog.classList.add('is-open');
    dialog.setAttribute('aria-hidden', 'false');
    trigger?.setAttribute('aria-expanded', 'true');
    syncModalState();
    setTimeout(() => focusTarget?.focus(), 80);
  }

  profileButton?.addEventListener('click', () => {
    authDialog.classList.contains('is-open')
      ? closeDialog(authDialog, profileButton)
      : openDialog(authDialog, profileButton, $('input', authDialog));
  });
  authClose?.addEventListener('click', () => closeDialog(authDialog, profileButton));
  authDialog?.addEventListener('click', (event) => { if (!authPanel.contains(event.target)) closeDialog(authDialog, profileButton); });
  authForm?.addEventListener('submit', (event) => event.preventDefault());

  contactsButton?.addEventListener('click', () => openDialog(contactsDialog, contactsButton, contactsClose));
  contactsClose?.addEventListener('click', () => closeDialog(contactsDialog, contactsButton));
  contactsDialog?.addEventListener('click', (event) => { if (!contactsPanel.contains(event.target)) closeDialog(contactsDialog, contactsButton); });

  const cookieKey = 'schedule-cookie-preferences';
  function readCookies() {
    try { return JSON.parse(localStorage.getItem(cookieKey) || '{}'); } catch { return {}; }
  }
  function fillCookies() {
    const prefs = readCookies();
    if (functionalCookies) functionalCookies.checked = Boolean(prefs.functional);
    if (analyticsCookies) analyticsCookies.checked = Boolean(prefs.analytics);
  }
  function saveCookiePrefs(functional, analytics) {
    localStorage.setItem(cookieKey, JSON.stringify({ necessary: true, functional, analytics }));
    closeDialog(cookieDialog, cookieButton);
  }
  cookieButton?.addEventListener('click', () => { fillCookies(); openDialog(cookieDialog, cookieButton, cookieClose); });
  cookieClose?.addEventListener('click', () => closeDialog(cookieDialog, cookieButton));
  cookieDialog?.addEventListener('click', (event) => { if (!cookiePanel.contains(event.target)) closeDialog(cookieDialog, cookieButton); });
  necessaryOnly?.addEventListener('click', () => saveCookiePrefs(false, false));
  acceptAllCookies?.addEventListener('click', () => saveCookiePrefs(true, true));
  saveCookies?.addEventListener('click', () => saveCookiePrefs(Boolean(functionalCookies?.checked), Boolean(analyticsCookies?.checked)));

  addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') return;
    if (authDialog?.classList.contains('is-open')) closeDialog(authDialog, profileButton);
    else if (contactsDialog?.classList.contains('is-open')) closeDialog(contactsDialog, contactsButton);
    else if (cookieDialog?.classList.contains('is-open')) closeDialog(cookieDialog, cookieButton);
  });
})();
