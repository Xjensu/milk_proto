const mode = document.body.dataset.searchMode;
const searchInput = document.getElementById('entitySearchInput');
const searchForm = document.getElementById('entitySearchForm');
const searchClear = document.getElementById('entitySearchClear');
const cardGrid = document.getElementById('entityCardGrid');
const emptyState = document.getElementById('entitySearchEmpty');
const pagination = document.getElementById('entityPagination');

const PAGE_SIZE = mode === 'teacher' ? 8 : 9;
let currentPage = 1;

const teachers = [
  { value: 'Ковалёв Алексей Сергеевич', departments: ['Строительные конструкции'] },
  { value: 'Лисовская Марина Викторовна', departments: ['Инженерная графика', 'Архитектура'] },
  { value: 'Титов Игорь Павлович', departments: ['Материаловедение'] },
  { value: 'Романова Елена Андреевна', departments: ['Высшая математика'] },
  { value: 'Беляев Дмитрий Олегович', departments: ['Строительная механика'] },
  { value: 'Савицкий Артём Николаевич', departments: ['Геодезия'] },
  { value: 'Гриневич Ольга Михайловна', departments: ['Архитектура'] },
  { value: 'Мороз Екатерина Сергеевна', departments: ['Иностранные языки'] },
  { value: 'Козлов Сергей Валерьевич', departments: ['Охрана труда', 'Безопасность жизнедеятельности'] },
  { value: 'Кравченко Антон Ильич', departments: ['Физика'] },
  { value: 'Климович Павел Андреевич', departments: ['Автоматизированное проектирование'] },
  { value: 'Смирнова Ирина Вадимовна', departments: ['Экономика строительства'] }
];

const rooms = [
  { value: '2-312', type: 'Лекционная аудитория', meta: 'Корпус 2' },
  { value: '1-218', type: 'Компьютерный класс', meta: 'Корпус 1' },
  { value: '3-107', type: 'Лаборатория', meta: 'Корпус 3' },
  { value: '4-401', type: 'Учебная аудитория', meta: 'Корпус 4' },
  { value: '2-406', type: 'Лекционная аудитория', meta: 'Корпус 2' },
  { value: '6-204', type: 'Геодезическая лаборатория', meta: 'Корпус 6' },
  { value: '1-306', type: 'Лингафонный кабинет', meta: 'Корпус 1' },
  { value: '7-411', type: 'Компьютерный класс', meta: 'Корпус 7' },
  { value: '4-210', type: 'Лекционная аудитория', meta: 'Корпус 4' },
  { value: '5-115', type: 'Лекционная аудитория', meta: 'Корпус 5' },
  { value: '6-208', type: 'Геодезическая лаборатория', meta: 'Корпус 6' },
  { value: '2-212', type: 'Учебная аудитория', meta: 'Корпус 2' },
  { value: '3-302', type: 'Лекционная аудитория', meta: 'Корпус 3' }
];

const data = mode === 'teacher' ? teachers : rooms;

function normalize(value) {
  return value.toLocaleLowerCase('ru-RU').replace(/ё/g, 'е').trim();
}

function cardMarkup(item, index) {
  if (mode === 'teacher') {
    const department = item.departments.join(' / ');
    return `
      <button class="entity-card teacher-entity-card" type="button" data-value="${item.value}" aria-label="${item.value}">
        <span class="teacher-card-name">${item.value}</span>
        <span class="teacher-card-department">${department}</span>
      </button>`;
  }

  return `
    <button class="entity-card room-entity-card" type="button" data-value="${item.value}" aria-label="${item.value}">
      <span class="room-card-number">${item.value}</span>
      <span class="room-card-type">${item.type}</span>
    </button>`;
}

function searchableText(item) {
  if (mode === 'teacher') return [item.value, ...item.departments].join(' ');
  return [item.value, item.type, item.meta].join(' ');
}

function render({ animate = false } = {}) {
  const query = normalize(searchInput.value);
  const filtered = data.filter((item) => normalize(searchableText(item)).includes(query));
  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  currentPage = Math.min(Math.max(currentPage, 1), totalPages);

  const start = (currentPage - 1) * PAGE_SIZE;
  const visibleItems = filtered.slice(start, start + PAGE_SIZE);
  cardGrid.innerHTML = visibleItems.map(cardMarkup).join('');
  emptyState.hidden = filtered.length !== 0;
  searchClear.classList.toggle('is-visible', Boolean(searchInput.value));
  renderPagination(totalPages, filtered.length);

  if (animate && visibleItems.length) {
    cardGrid.animate(
      [
        { opacity: .35, transform: 'translateY(8px)' },
        { opacity: 1, transform: 'translateY(0)' }
      ],
      { duration: 230, easing: 'cubic-bezier(.2,.8,.2,1)' }
    );
  }
}

function renderPagination(totalPages, resultCount) {
  if (!pagination || resultCount === 0 || totalPages <= 1) {
    if (pagination) {
      pagination.hidden = true;
      pagination.innerHTML = '';
    }
    return;
  }

  pagination.hidden = false;
  const pageButtons = Array.from({ length: totalPages }, (_, index) => {
    const page = index + 1;
    const active = page === currentPage;
    return `<button class="entity-page-number${active ? ' is-active' : ''}" type="button" data-page="${page}"${active ? ' aria-current="page"' : ''}>${page}</button>`;
  }).join('');

  pagination.innerHTML = `
    <button class="entity-page-arrow" type="button" data-page-action="prev" aria-label="Предыдущая страница" ${currentPage === 1 ? 'disabled' : ''}>‹</button>
    <div class="entity-page-numbers">${pageButtons}</div>
    <button class="entity-page-arrow" type="button" data-page-action="next" aria-label="Следующая страница" ${currentPage === totalPages ? 'disabled' : ''}>›</button>
  `;
}

function setPage(page) {
  const nextPage = Number(page);
  if (!Number.isFinite(nextPage) || nextPage === currentPage) return;
  currentPage = nextPage;
  render({ animate: true });
  const targetY = Math.max(0, cardGrid.getBoundingClientRect().top + window.scrollY - 110);
  window.scrollTo({ top: targetY, behavior: 'smooth' });
}

function openSchedule(itemValue, card) {
  card.classList.add('is-selected');
  const rect = card.getBoundingClientRect();
  const clone = card.cloneNode(true);
  clone.classList.remove('is-selected');
  clone.classList.add('entity-card-flyer');
  Object.assign(clone.style, { left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px` });
  document.body.appendChild(clone);
  const targetW = Math.min(window.innerWidth * .30, 360);
  const targetH = Math.min(window.innerHeight * .28, 240);
  const targetX = (window.innerWidth - targetW) / 2;
  const targetY = (window.innerHeight - targetH) / 2;
  const animation = clone.animate([
    { left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px`, opacity: 1 },
    { left: `${targetX}px`, top: `${targetY}px`, width: `${targetW}px`, height: `${targetH}px`, opacity: 1 }
  ], { duration: 390, easing: 'cubic-bezier(.76,0,.24,1)', fill: 'forwards' });
  animation.finished.finally(() => {
    const params = new URLSearchParams();
    params.set(mode, itemValue);
    window.location.href = mode === 'teacher'
      ? `teacher-detail.html?${params.toString()}`
      : `room-detail.html?${params.toString()}`;
  });
}

searchForm.addEventListener('submit', (event) => event.preventDefault());
searchInput.addEventListener('input', () => { currentPage = 1; render(); });
searchClear.addEventListener('click', () => { searchInput.value = ''; currentPage = 1; render(); searchInput.focus(); });
cardGrid.addEventListener('click', (event) => {
  const card = event.target.closest('.entity-card[data-value]');
  if (!card) return;
  openSchedule(card.dataset.value, card);
});

pagination?.addEventListener('click', (event) => {
  const pageButton = event.target.closest('[data-page]');
  if (pageButton) {
    setPage(pageButton.dataset.page);
    return;
  }

  const actionButton = event.target.closest('[data-page-action]');
  if (!actionButton || actionButton.disabled) return;
  setPage(actionButton.dataset.pageAction === 'prev' ? currentPage - 1 : currentPage + 1);
});

// Footer remains below the first viewport until the user starts scrolling.
function updateFooterVisibility() {
  document.body.classList.toggle('footer-visible', window.scrollY > 2);
}
window.addEventListener('scroll', updateFooterVisibility, { passive: true });
updateFooterVisibility();

// Shared compact dialogs.
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
  document.body.classList.toggle('modal-open', [authDialog, contactsDialog, cookieDialog].some((dialog) => dialog.classList.contains('is-open')));
}
function closeAuth(returnFocus = false) { authDialog.classList.remove('is-open'); authDialog.setAttribute('aria-hidden','true'); profileButton.setAttribute('aria-expanded','false'); syncBodyLock(); if(returnFocus) profileButton.focus(); }
function closeContacts(returnFocus = false) { contactsDialog.classList.remove('is-open'); contactsDialog.setAttribute('aria-hidden','true'); contactsButton.setAttribute('aria-expanded','false'); syncBodyLock(); if(returnFocus) contactsButton.focus(); }
function closeCookies(returnFocus = false) { cookieDialog.classList.remove('is-open'); cookieDialog.setAttribute('aria-hidden','true'); cookieButton.setAttribute('aria-expanded','false'); syncBodyLock(); if(returnFocus) cookieButton.focus(); }
function openAuth() { closeContacts(false); closeCookies(false); authDialog.classList.add('is-open'); authDialog.setAttribute('aria-hidden','false'); profileButton.setAttribute('aria-expanded','true'); syncBodyLock(); setTimeout(()=>loginInput.focus(),80); }
function openContacts() { closeAuth(false); closeCookies(false); contactsDialog.classList.add('is-open'); contactsDialog.setAttribute('aria-hidden','false'); contactsButton.setAttribute('aria-expanded','true'); syncBodyLock(); setTimeout(()=>firstContact?.focus(),80); }
function loadCookiePreferences() { try { const p=JSON.parse(localStorage.getItem(COOKIE_PREFERENCES_KEY))||{}; functionalCookies.checked=Boolean(p.functional); analyticsCookies.checked=Boolean(p.analytics); } catch {} }
function setCookie(name,value,days=365){ document.cookie=`${name}=${encodeURIComponent(value)}; max-age=${days*86400}; path=/; SameSite=Lax`; }
function removeCookie(name){ document.cookie=`${name}=; max-age=0; path=/; SameSite=Lax`; }
function persistCookiePreferences({functional,analytics}){ localStorage.setItem(COOKIE_PREFERENCES_KEY,JSON.stringify({necessary:true,functional,analytics,updatedAt:new Date().toISOString()})); setCookie('schedule_cookie_consent','saved'); functional?setCookie('schedule_functional','1'):removeCookie('schedule_functional'); analytics?setCookie('schedule_analytics','1'):removeCookie('schedule_analytics'); }
function openCookies(){ closeAuth(false); closeContacts(false); loadCookiePreferences(); cookieDialog.classList.add('is-open'); cookieDialog.setAttribute('aria-hidden','false'); cookieButton.setAttribute('aria-expanded','true'); syncBodyLock(); setTimeout(()=>functionalCookies.focus(),80); }

profileButton.addEventListener('click',()=>authDialog.classList.contains('is-open')?closeAuth(true):openAuth());
contactsButton.addEventListener('click',()=>contactsDialog.classList.contains('is-open')?closeContacts(true):openContacts());
cookieButton.addEventListener('click',openCookies);
authClose.addEventListener('click',()=>closeAuth(true)); contactsClose.addEventListener('click',()=>closeContacts(true)); cookieClose.addEventListener('click',()=>closeCookies(true));
authDialog.addEventListener('click',(e)=>{ if(!authPanel.contains(e.target)) closeAuth(true); });
contactsDialog.addEventListener('click',(e)=>{ if(!contactsPanel.contains(e.target)) closeContacts(true); });
cookieDialog.addEventListener('click',(e)=>{ if(!cookiePanel.contains(e.target)) closeCookies(true); });
necessaryOnly.addEventListener('click',()=>{ functionalCookies.checked=false; analyticsCookies.checked=false; persistCookiePreferences({functional:false,analytics:false}); closeCookies(true); });
acceptAllCookies.addEventListener('click',()=>{ functionalCookies.checked=true; analyticsCookies.checked=true; persistCookiePreferences({functional:true,analytics:true}); closeCookies(true); });
saveCookies.addEventListener('click',()=>{ persistCookiePreferences({functional:functionalCookies.checked,analytics:analyticsCookies.checked}); closeCookies(true); });
authForm.addEventListener('submit',(e)=>{ e.preventDefault(); closeAuth(false); });
document.addEventListener('keydown',(e)=>{ if(e.key!=='Escape') return; if(authDialog.classList.contains('is-open')) closeAuth(true); else if(contactsDialog.classList.contains('is-open')) closeContacts(true); else if(cookieDialog.classList.contains('is-open')) closeCookies(true); });

render();
