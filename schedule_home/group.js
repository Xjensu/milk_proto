const facultyGrid = document.getElementById('facultyGrid');
const facultyCards = [...document.querySelectorAll('.faculty-card')];
const facultyDetail = document.getElementById('facultyDetail');
const facultyDetailClose = document.getElementById('facultyDetailClose');
const facultyDetailProduct = document.getElementById('facultyDetailProduct');
const facultyDetailMonogram = document.getElementById('facultyDetailMonogram');
const facultyPrev = document.getElementById('facultyPrev');
const facultyNext = document.getElementById('facultyNext');
const facultyDots = document.getElementById('facultyDots');
const facultyCourses = document.getElementById('facultyCourses');
const facultyGroups = document.getElementById('facultyGroups');
const facultySchedules = document.getElementById('facultySchedules');
const facultyConfirm = document.getElementById('facultyConfirm');

const facultyOrder = ['stf', 'mf', 'upp', 'etf', 'ef', 'ztf', 'vtf'];
const scheduleTypes = [
  { key: 'regular', label: 'ОБЫЧНОЕ' },
  { key: 'lecture', label: 'ЛЕКЦИОННАЯ НЕДЕЛЯ' },
  { key: 'credit', label: 'ЗАЧЕТНАЯ НЕДЕЛЯ' },
  { key: 'exam', label: 'ЭКЗАМЕНАЦИОННАЯ НЕДЕЛЯ' }
];

const facultyData = {
  stf: { monogram: 'СТФ', name: 'СТРОИТЕЛЬНЫЙ', codes: ['ПС', 'АС', 'ВВ', 'ИС'] },
  mf:  { monogram: 'МФ',  name: 'МЕХАНИЧЕСКИЙ', codes: ['МТ', 'ЛТ', 'ТМ', 'ПМ'] },
  upp: { monogram: 'УПП', name: 'УПП', codes: ['УД', 'ОП', 'ЛГ', 'ТК'] },
  etf: { monogram: 'ЭТФ', name: 'ЭЛЕКТРОТЕХНИЧЕСКИЙ', codes: ['ЭТ', 'АЭ', 'ЭС', 'ИЭ'] },
  ef:  { monogram: 'ЭФ',  name: 'ЭКОНОМИЧЕСКИЙ', codes: ['ЭК', 'БУ', 'МН', 'ЛЭ'] },
  ztf: { monogram: 'ЗФ',  name: 'ЗАОЧНЫЙ', codes: ['ЗС', 'ЗМ', 'ЗЭ', 'ЗУ'] },
  vtf: { monogram: 'ВТФ', name: 'ВОЕННО-ТРАНСПОРТНЫЙ', codes: ['ВТ', 'ВМ', 'ВЛ', 'ВО'] }
};

let activeFaculty = facultyOrder[0];
let activeCourse = 1;
let activeGroup = null;
let activeSchedule = 'regular';
let returnCard = null;
let detailAnimating = false;

function groupsFor(key, course) {
  return facultyData[key].codes.map((code, index) => `${code}-${course}${index + 1}`);
}

function renderDetail({ keepGroup = false } = {}) {
  const data = facultyData[activeFaculty];
  facultyDetailMonogram.textContent = data.monogram;
  facultyDetailProduct.dataset.faculty = activeFaculty;

  facultyDots.innerHTML = facultyOrder.map((key) =>
    `<span class="${key === activeFaculty ? 'is-active' : ''}"></span>`
  ).join('');

  facultyCourses.innerHTML = [1, 2, 3, 4, 5].map((course) =>
    `<button type="button" data-course="${course}" class="${course === activeCourse ? 'is-active' : ''}" aria-label="${course} курс">${course}</button>`
  ).join('');

  const groups = groupsFor(activeFaculty, activeCourse);
  if (!keepGroup || !groups.includes(activeGroup)) activeGroup = groups[0];
  facultyGroups.innerHTML = groups.map((group) =>
    `<button type="button" data-group="${group}" class="${group === activeGroup ? 'is-active' : ''}">${group}</button>`
  ).join('');

  facultySchedules.innerHTML = scheduleTypes.map((item) =>
    `<button type="button" data-schedule="${item.key}" class="${item.key === activeSchedule ? 'is-active' : ''}">${item.label}</button>`
  ).join('');

  facultyCourses.querySelectorAll('button').forEach((button) => {
    button.addEventListener('click', () => {
      activeCourse = Number(button.dataset.course);
      activeGroup = null;
      renderDetail();
    });
  });

  facultyGroups.querySelectorAll('button').forEach((button) => {
    button.addEventListener('click', () => {
      activeGroup = button.dataset.group;
      facultyGroups.querySelectorAll('button').forEach((item) => item.classList.toggle('is-active', item === button));
    });
  });

  facultySchedules.querySelectorAll('button').forEach((button) => {
    button.addEventListener('click', () => {
      activeSchedule = button.dataset.schedule;
      facultySchedules.querySelectorAll('button').forEach((item) => item.classList.toggle('is-active', item === button));
    });
  });
}

function detailTargetRect() {
  return facultyDetailProduct.getBoundingClientRect();
}

function animateProductToDetail(card) {
  const source = card.querySelector('.faculty-product');
  const from = source.getBoundingClientRect();
  const to = detailTargetRect();
  const clone = source.cloneNode(true);
  clone.classList.add('faculty-product-flyer');
  clone.style.left = `${from.left}px`;
  clone.style.top = `${from.top}px`;
  clone.style.width = `${from.width}px`;
  clone.style.height = `${from.height}px`;
  document.body.appendChild(clone);

  source.style.opacity = '0';
  facultyDetailProduct.style.opacity = '0';

  const animation = clone.animate([
    { left: `${from.left}px`, top: `${from.top}px`, width: `${from.width}px`, height: `${from.height}px`, transform: 'scale(1)' },
    { left: `${to.left}px`, top: `${to.top}px`, width: `${to.width}px`, height: `${to.height}px`, transform: 'scale(1)' }
  ], {
    duration: 560,
    easing: 'cubic-bezier(.76,0,.24,1)',
    fill: 'forwards'
  });

  animation.finished.finally(() => {
    clone.remove();
    facultyDetailProduct.style.opacity = '';
    source.style.opacity = '';
    detailAnimating = false;
  });
}

function openFaculty(card) {
  if (detailAnimating) return;
  detailAnimating = true;
  returnCard = card;
  activeFaculty = card.dataset.faculty;
  activeCourse = 1;
  activeGroup = null;
  activeSchedule = 'regular';
  renderDetail();

  facultyDetail.scrollTop = 0;
  facultyDetail.classList.add('is-open');
  facultyDetail.setAttribute('aria-hidden', 'false');
  document.body.classList.add('faculty-detail-open');
  updateFooterVisibility();

  requestAnimationFrame(() => requestAnimationFrame(() => animateProductToDetail(card)));
}

function closeFaculty() {
  if (!facultyDetail.classList.contains('is-open')) return;
  facultyDetail.classList.remove('is-open');
  facultyDetail.setAttribute('aria-hidden', 'true');
  document.body.classList.remove('faculty-detail-open');
  updateFooterVisibility();
  returnCard?.focus();
}

function shiftFaculty(direction) {
  if (detailAnimating) return;
  detailAnimating = true;

  const current = facultyOrder.indexOf(activeFaculty);
  activeFaculty = facultyOrder[(current + direction + facultyOrder.length) % facultyOrder.length];
  activeCourse = 1;
  activeGroup = null;
  activeSchedule = 'regular';

  const outAnimation = facultyDetailProduct.animate([
    { opacity: 1, transform: 'translateX(-50%) translateX(0) scale(1)' },
    { opacity: 0, transform: `translateX(-50%) translateX(${direction > 0 ? -24 : 24}px) scale(.985)` }
  ], { duration: 150, easing: 'ease-in', fill: 'forwards' });

  outAnimation.finished.then(() => {
    outAnimation.cancel();
    renderDetail();

    const inAnimation = facultyDetailProduct.animate([
      { opacity: 0, transform: `translateX(-50%) translateX(${direction > 0 ? 24 : -24}px) scale(.985)` },
      { opacity: 1, transform: 'translateX(-50%) translateX(0) scale(1)' }
    ], { duration: 230, easing: 'cubic-bezier(.2,.8,.2,1)', fill: 'forwards' });

    inAnimation.finished.finally(() => {
      inAnimation.cancel();
      detailAnimating = false;
    });
  }).catch(() => {
    detailAnimating = false;
  });
}

facultyCards.forEach((card) => card.addEventListener('click', () => openFaculty(card)));
facultyDetailClose.addEventListener('click', closeFaculty);
facultyPrev.addEventListener('click', () => shiftFaculty(-1));
facultyNext.addEventListener('click', () => shiftFaculty(1));

facultyConfirm.addEventListener('click', () => {
  if (!activeGroup) return;
  const params = new URLSearchParams({
    faculty: activeFaculty,
    course: String(activeCourse),
    group: activeGroup,
    schedule: activeSchedule
  });

  const animation = facultyConfirm.animate([
    { transform: 'scale(1)' },
    { transform: 'scale(.76)' },
    { transform: 'scale(1)' }
  ], { duration: 260, easing: 'cubic-bezier(.2,.8,.2,1)' });

  animation.finished.finally(() => {
    window.location.href = `schedule.html?${params.toString()}`;
  });
});

// Footer stays outside the first viewport and slides in only after the user scrolls.
function updateFooterVisibility() {
  const detailIsOpen = facultyDetail.classList.contains('is-open');
  const scrolled = detailIsOpen ? facultyDetail.scrollTop > 2 : window.scrollY > 2;
  document.body.classList.toggle('footer-visible', scrolled);
}

window.addEventListener('scroll', updateFooterVisibility, { passive: true });
facultyDetail.addEventListener('scroll', updateFooterVisibility, { passive: true });
updateFooterVisibility();

// Shared compact dialogs
const profileButton = document.querySelector('.profile-button');
const authDialog = document.getElementById('authDialog');
const authPanel = authDialog.querySelector('.auth-panel');
const authClose = authDialog.querySelector('.auth-close');
const authForm = document.getElementById('authForm');
const loginInput = authForm.querySelector('input[name="login"]');

const contactsButton = document.getElementById('contactsButton');
const contactsDialog = document.getElementById('contactsDialog');
const contactsPanel = contactsDialog.querySelector('.contacts-panel');
const contactsClose = contactsDialog.querySelector('.contacts-close');
const firstContact = contactsDialog.querySelector('.contact-item');

const cookieButton = document.getElementById('cookieButton');
const cookieDialog = document.getElementById('cookieDialog');
const cookiePanel = cookieDialog.querySelector('.cookie-panel');
const cookieClose = cookieDialog.querySelector('.cookie-close');
const functionalCookies = document.getElementById('functionalCookies');
const analyticsCookies = document.getElementById('analyticsCookies');
const necessaryOnly = document.getElementById('necessaryOnly');
const acceptAllCookies = document.getElementById('acceptAllCookies');
const saveCookies = document.getElementById('saveCookies');

const COOKIE_PREFERENCES_KEY = 'schedule-cookie-preferences';

function syncBodyLock() {
  const anyDialogOpen = [authDialog, contactsDialog, cookieDialog]
    .some((dialog) => dialog.classList.contains('is-open'));
  document.body.classList.toggle('modal-open', anyDialogOpen);
}

function closeAuth(returnFocus = false) {
  authDialog.classList.remove('is-open');
  authDialog.setAttribute('aria-hidden', 'true');
  profileButton.setAttribute('aria-expanded', 'false');
  syncBodyLock();
  if (returnFocus) profileButton.focus();
}

function closeContacts(returnFocus = false) {
  contactsDialog.classList.remove('is-open');
  contactsDialog.setAttribute('aria-hidden', 'true');
  contactsButton.setAttribute('aria-expanded', 'false');
  syncBodyLock();
  if (returnFocus) contactsButton.focus();
}

function closeCookies(returnFocus = false) {
  cookieDialog.classList.remove('is-open');
  cookieDialog.setAttribute('aria-hidden', 'true');
  cookieButton.setAttribute('aria-expanded', 'false');
  syncBodyLock();
  if (returnFocus) cookieButton.focus();
}

function closeOtherDialogs(except) {
  if (except !== 'auth') closeAuth(false);
  if (except !== 'contacts') closeContacts(false);
  if (except !== 'cookies') closeCookies(false);
}

function openAuth() {
  closeOtherDialogs('auth');
  authDialog.classList.add('is-open');
  authDialog.setAttribute('aria-hidden', 'false');
  profileButton.setAttribute('aria-expanded', 'true');
  syncBodyLock();
  window.setTimeout(() => loginInput.focus(), 80);
}

function openContacts() {
  closeOtherDialogs('contacts');
  contactsDialog.classList.add('is-open');
  contactsDialog.setAttribute('aria-hidden', 'false');
  contactsButton.setAttribute('aria-expanded', 'true');
  syncBodyLock();
  window.setTimeout(() => firstContact?.focus(), 80);
}

function readCookiePreferences() {
  try { return JSON.parse(localStorage.getItem(COOKIE_PREFERENCES_KEY)) || {}; }
  catch { return {}; }
}

function loadCookiePreferences() {
  const preferences = readCookiePreferences();
  functionalCookies.checked = Boolean(preferences.functional);
  analyticsCookies.checked = Boolean(preferences.analytics);
}

function setCookie(name, value, days = 365) {
  const maxAge = days * 24 * 60 * 60;
  document.cookie = `${name}=${encodeURIComponent(value)}; max-age=${maxAge}; path=/; SameSite=Lax`;
}

function removeCookie(name) {
  document.cookie = `${name}=; max-age=0; path=/; SameSite=Lax`;
}

function persistCookiePreferences({ functional, analytics }) {
  const preferences = { necessary: true, functional, analytics, updatedAt: new Date().toISOString() };
  localStorage.setItem(COOKIE_PREFERENCES_KEY, JSON.stringify(preferences));
  setCookie('schedule_cookie_consent', 'saved');
  functional ? setCookie('schedule_functional', '1') : removeCookie('schedule_functional');
  analytics ? setCookie('schedule_analytics', '1') : removeCookie('schedule_analytics');
}

function openCookies() {
  closeOtherDialogs('cookies');
  loadCookiePreferences();
  cookieDialog.classList.add('is-open');
  cookieDialog.setAttribute('aria-hidden', 'false');
  cookieButton.setAttribute('aria-expanded', 'true');
  syncBodyLock();
  window.setTimeout(() => functionalCookies.focus(), 80);
}

profileButton.addEventListener('click', () => authDialog.classList.contains('is-open') ? closeAuth(true) : openAuth());
contactsButton.addEventListener('click', () => contactsDialog.classList.contains('is-open') ? closeContacts(true) : openContacts());
cookieButton.addEventListener('click', openCookies);

authClose.addEventListener('click', () => closeAuth(true));
contactsClose.addEventListener('click', () => closeContacts(true));
cookieClose.addEventListener('click', () => closeCookies(true));

authDialog.addEventListener('click', (event) => { if (!authPanel.contains(event.target)) closeAuth(true); });
contactsDialog.addEventListener('click', (event) => { if (!contactsPanel.contains(event.target)) closeContacts(true); });
cookieDialog.addEventListener('click', (event) => { if (!cookiePanel.contains(event.target)) closeCookies(true); });

authForm.addEventListener('submit', (event) => event.preventDefault());
necessaryOnly.addEventListener('click', () => {
  functionalCookies.checked = false;
  analyticsCookies.checked = false;
  persistCookiePreferences({ functional: false, analytics: false });
  closeCookies(true);
});
acceptAllCookies.addEventListener('click', () => {
  functionalCookies.checked = true;
  analyticsCookies.checked = true;
  persistCookiePreferences({ functional: true, analytics: true });
  closeCookies(true);
});
saveCookies.addEventListener('click', () => {
  persistCookiePreferences({ functional: functionalCookies.checked, analytics: analyticsCookies.checked });
  closeCookies(true);
});

document.addEventListener('keydown', (event) => {
  if (event.key !== 'Escape') return;
  if (facultyDetail.classList.contains('is-open')) { closeFaculty(); return; }
  if (cookieDialog.classList.contains('is-open')) closeCookies(true);
  else if (contactsDialog.classList.contains('is-open')) closeContacts(true);
  else if (authDialog.classList.contains('is-open')) closeAuth(true);
});
