const teacherProfiles = [
  {
    value: 'Ковалёв Алексей Сергеевич', code: 'TCH-041', departments: ['Строительные конструкции'],
    degree: 'кандидат технических наук', title: 'доцент', position: 'доцент',
    subjects: [
      ['Строительные конструкции', 'Строит. конструкции'],
      ['Железобетонные конструкции', 'ЖБ конструкции'],
      ['Основы расчёта сооружений', 'Расчёт сооружений']
    ],
    groups: ['ПГС-21', 'ПГС-22', 'ПГС-31', 'ПГС-32']
  },
  {
    value: 'Лисовская Марина Викторовна', code: 'TCH-052', departments: ['Инженерная графика', 'Архитектура'],
    degree: 'кандидат технических наук', title: 'доцент', position: 'доцент',
    subjects: [
      ['Инженерная и компьютерная графика', 'Инж. графика'],
      ['Начертательная геометрия', 'Начерт. геометрия'],
      ['Основы архитектурной графики', 'Арх. графика']
    ],
    groups: ['ПГС-11', 'ПГС-12', 'АРХ-21', 'АРХ-22']
  },
  {
    value: 'Титов Игорь Павлович', code: 'TCH-063', departments: ['Материаловедение'],
    degree: 'кандидат технических наук', title: 'доцент', position: 'заведующий кафедрой',
    subjects: [
      ['Материаловедение и технология материалов', 'Материаловедение'],
      ['Испытание строительных материалов', 'Испытание материалов'],
      ['Технология конструкционных материалов', 'ТКМ']
    ],
    groups: ['МХ-21', 'МХ-22', 'ПГС-23', 'ПГС-24']
  },
  {
    value: 'Романова Елена Андреевна', code: 'TCH-074', departments: ['Высшая математика'],
    degree: 'кандидат физико-математических наук', title: 'доцент', position: 'доцент',
    subjects: [
      ['Высшая математика', 'Высшая математика'],
      ['Теория вероятностей и математическая статистика', 'ТВиМС'],
      ['Математические методы в инженерии', 'Мат. методы']
    ],
    groups: ['ЭТ-11', 'ЭТ-12', 'ЭК-11', 'ЭК-12']
  },
  {
    value: 'Беляев Дмитрий Олегович', code: 'TCH-085', departments: ['Строительная механика'],
    degree: 'кандидат технических наук', title: 'доцент', position: 'доцент',
    subjects: [
      ['Строительная механика', 'Строит. механика'],
      ['Сопротивление материалов', 'Сопр. материалов'],
      ['Динамика сооружений', 'Динамика сооружений']
    ],
    groups: ['ПГС-31', 'ПГС-32', 'МТ-21', 'МТ-22']
  },
  {
    value: 'Савицкий Артём Николаевич', code: 'TCH-096', departments: ['Геодезия'],
    degree: 'кандидат технических наук', title: 'доцент', position: 'старший преподаватель',
    subjects: [
      ['Инженерная геодезия', 'Инж. геодезия'],
      ['Геодезическое обеспечение строительства', 'Геодезия'],
      ['Спутниковые технологии позиционирования', 'GNSS']
    ],
    groups: ['ГД-21', 'ГД-22', 'ПГС-21', 'ПГС-22']
  },
  {
    value: 'Гриневич Ольга Михайловна', code: 'TCH-107', departments: ['Архитектура'],
    degree: 'магистр архитектуры', title: 'доцент', position: 'доцент',
    subjects: [
      ['Архитектурное проектирование', 'Арх. проектирование'],
      ['Композиционное моделирование', 'Композиция'],
      ['История архитектуры', 'История архитектуры']
    ],
    groups: ['АРХ-21', 'АРХ-22', 'АРХ-31', 'АРХ-32']
  },
  {
    value: 'Мороз Екатерина Сергеевна', code: 'TCH-118', departments: ['Иностранные языки'],
    degree: 'магистр филологических наук', title: '—', position: 'старший преподаватель',
    subjects: [
      ['Иностранный язык профессиональной коммуникации', 'Английский язык'],
      ['Технический английский язык', 'Тех. английский'],
      ['Деловая коммуникация', 'Деловая коммуникация']
    ],
    groups: ['ЭК-21', 'ЭТ-21', 'ПГС-21', 'МХ-21']
  },
  {
    value: 'Козлов Сергей Валерьевич', code: 'TCH-129', departments: ['Охрана труда', 'Безопасность жизнедеятельности'],
    degree: 'кандидат технических наук', title: 'доцент', position: 'доцент',
    subjects: [
      ['Охрана труда', 'Охрана труда'],
      ['Безопасность жизнедеятельности', 'БЖД'],
      ['Промышленная безопасность', 'Пром. безопасность']
    ],
    groups: ['УПП-31', 'УПП-32', 'ЭТ-31', 'ПГС-31']
  },
  {
    value: 'Кравченко Антон Ильич', code: 'TCH-140', departments: ['Физика'],
    degree: 'кандидат физико-математических наук', title: 'доцент', position: 'доцент',
    subjects: [
      ['Общая физика: электричество и магнетизм', 'Физика'],
      ['Механика и молекулярная физика', 'Механика'],
      ['Физический практикум', 'Физ. практикум']
    ],
    groups: ['ПГС-11', 'ЭТ-11', 'МХ-11', 'АРХ-11']
  },
  {
    value: 'Климович Павел Андреевич', code: 'TCH-151', departments: ['Автоматизированное проектирование'],
    degree: 'кандидат технических наук', title: 'доцент', position: 'старший преподаватель',
    subjects: [
      ['Основы автоматизированного проектирования зданий', 'Основы САПР'],
      ['Информационное моделирование зданий', 'BIM'],
      ['Компьютерное моделирование', 'Комп. моделирование']
    ],
    groups: ['ПГС-31', 'ПГС-32', 'АРХ-31', 'АРХ-32']
  },
  {
    value: 'Смирнова Ирина Вадимовна', code: 'TCH-162', departments: ['Экономика строительства'],
    degree: 'кандидат экономических наук', title: 'доцент', position: 'доцент',
    subjects: [
      ['Экономика строительного производства', 'Экономика'],
      ['Организация строительного производства', 'Организация'],
      ['Управление проектами', 'Упр. проектами']
    ],
    groups: ['ПГС-31', 'ПГС-32', 'ЭК-21', 'УПП-31']
  }
];

const params = new URLSearchParams(window.location.search);
const requestedTeacher = params.get('teacher');
const teacher = teacherProfiles.find((item) => item.value === requestedTeacher) || teacherProfiles[0];
const returnUrl = params.get('return');
const teacherBack = document.querySelector('.teacher-record-back');
if (returnUrl) teacherBack.href = returnUrl;

document.title = teacher.value;

document.getElementById('teacherRecordNumber').textContent = teacher.code;
document.getElementById('teacherRecordFile').textContent = teacher.code;
document.getElementById('teacherRecordName').textContent = teacher.value;
document.getElementById('teacherSchedulePerson').textContent = teacher.value;

document.getElementById('teacherRecordFields').innerHTML = [
  ['ФИО', teacher.value],
  ['Кафедра', teacher.departments.join(' / ')],
  ['Учёная степень', teacher.degree],
  ['Учёное звание', teacher.title],
  ['Должность', teacher.position]
].map(([label, value]) => `
  <div class="teacher-record-field">
    <span>${label}</span><strong>${value}</strong>
  </div>`).join('');

// Pixel portrait: deliberately abstract and non-photographic.
const pixelPattern = [
  '00000000111100000000',
  '00000011111111000000',
  '00000111111111100000',
  '00001111111111110000',
  '00011122222221111000',
  '00111222222222111100',
  '00112221111222211100',
  '00112210000122211100',
  '00112211001122211100',
  '00112222222222211100',
  '00112222112222211100',
  '00111222222222111100',
  '00011122222221111000',
  '00001112222111110000',
  '00000111221111100000',
  '00000011111111000000',
  '00000001111110000000',
  '00000001111110000000',
  '00000111111111100000',
  '00011111111111111000',
  '00111111111111111100',
  '01111111111111111110'
];
const pixelHead = document.getElementById('teacherPixelHead');
pixelHead.innerHTML = pixelPattern.flatMap((row) => [...row]).map((cell) => `<i class="pixel-${cell}"></i>`).join('');

const weekdays = [
  { key: 'mon', short: 'ПН', label: 'Понедельник', jsDay: 1 },
  { key: 'tue', short: 'ВТ', label: 'Вторник', jsDay: 2 },
  { key: 'wed', short: 'СР', label: 'Среда', jsDay: 3 },
  { key: 'thu', short: 'ЧТ', label: 'Четверг', jsDay: 4 },
  { key: 'fri', short: 'ПТ', label: 'Пятница', jsDay: 5 },
  { key: 'sat', short: 'СБ', label: 'Суббота', jsDay: 6 }
];
const times = [
  { key: '1', start: '08:30', end: '10:00' },
  { key: '2', start: '10:10', end: '11:40' },
  { key: '3', start: '11:50', end: '13:20' },
  { key: '4', start: '14:00', end: '15:30' },
  { key: '5', start: '15:40', end: '17:10' },
  { key: '6', start: '17:20', end: '18:50' }
];

function isoWeek(date) {
  const d = new Date(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()));
  const day = d.getUTCDay() || 7;
  d.setUTCDate(d.getUTCDate() + 4 - day);
  const yearStart = new Date(Date.UTC(d.getUTCFullYear(), 0, 1));
  return Math.ceil((((d - yearStart) / 86400000) + 1) / 7);
}

const now = new Date();
const currentWeekNumber = isoWeek(now);
const currentWeek = currentWeekNumber % 2 === 0 ? 'even' : 'odd';
let selectedWeek = currentWeek;
let selectedMobileDay = weekdays.find((day) => day.jsDay === now.getDay())?.key || 'mon';

function uniqueList(values) {
  return [...new Set(values.filter(Boolean))];
}

function asList(value) {
  if (Array.isArray(value)) return value.filter(Boolean);
  return value ? [value] : [];
}

function listText(value, empty = '—') {
  const items = asList(value);
  return items.length ? items.join(' · ') : empty;
}

function buildEvents(profile) {
  const s = profile.subjects;
  const g = profile.groups;
  const coA = profile.value === 'Ковалёв Алексей Сергеевич' ? 'Беляев Дмитрий Олегович' : 'Ковалёв Алексей Сергеевич';
  const coB = profile.value === 'Лисовская Марина Викторовна' ? 'Титов Игорь Павлович' : 'Лисовская Марина Викторовна';
  return [
    { id: 'e1', day: 'mon', time: '1', week: 'both', subject: s[0], type: 'Лекция', rooms: ['2-406','2-312'], teachers: uniqueList([profile.value, coA]), groups: [g[0], g[1]] },
    { id: 'e2', day: 'mon', time: '3', week: 'odd', subject: s[1], type: 'Практика', rooms: [], teachers: uniqueList([profile.value, coB]), groups: [g[2]] },
    { id: 'e3', day: 'tue', time: '2', week: 'both', subject: s[2], type: 'Лаб.', rooms: ['3-107','6-208'], teachers: [profile.value], groups: [g[1]] },
    { id: 'e4', day: 'wed', time: '1', week: 'even', subject: s[1], type: 'Лекция', rooms: ['1-218'], teachers: [profile.value], groups: [g[0], g[1], g[2]] },
    { id: 'e5', day: 'wed', time: '4', week: 'both', subject: s[0], type: 'Практика', rooms: [], teachers: [profile.value], groups: [g[3]] },
    { id: 'e6', day: 'thu', time: '2', week: 'both', subject: s[2], type: 'Лекция', rooms: ['2-406'], teachers: uniqueList([profile.value, coA]), groups: [g[0], g[3]] },
    { id: 'e7', day: 'fri', time: '3', week: 'odd', subject: s[0], type: 'Лаб.', rooms: ['3-107'], teachers: [profile.value], groups: [g[2]] },
    { id: 'e8', day: 'sat', time: '1', week: 'even', subject: s[1], type: 'Конс.', rooms: ['1-306','1-218'], teachers: [profile.value], groups: [g[0], g[1]] }
  ];
}
const events = buildEvents(teacher);

const table = document.getElementById('teacherScheduleTable');
const weekSelector = document.getElementById('teacherWeekSelector');
const mobileDays = document.getElementById('teacherMobileDays');
const mobileAgenda = document.getElementById('teacherMobileAgenda');
const eventDialog = document.getElementById('teacherEventDialog');
const eventClose = document.getElementById('teacherEventClose');
const eventType = document.getElementById('teacherEventType');
const eventName = document.getElementById('teacherEventName');
const eventMeta = document.getElementById('teacherEventMeta');

function visibleEvent(dayKey, timeKey) {
  return events.find((event) => event.day === dayKey && event.time === timeKey && (event.week === 'both' || event.week === selectedWeek));
}

function eventMarkup(event) {
  return `
    <button class="teacher-schedule-event" type="button" data-event-id="${event.id}" aria-label="${event.subject[0]}">
      <span class="teacher-schedule-event-top"><span>${event.type}</span><span>${listText(event.rooms)}</span></span>
      <span class="teacher-schedule-event-name">${event.subject[1]}</span>
      <span class="teacher-schedule-event-groups">${event.groups.join(' · ')}</span>
    </button>`;
}

function renderTable() {
  const today = now.getDay();
  const head = `<div class="teacher-schedule-head teacher-schedule-head--time">TIME</div>${weekdays.map((day) => `<div class="teacher-schedule-head${selectedWeek === currentWeek && day.jsDay === today ? ' is-today' : ''}">${day.short}</div>`).join('')}`;
  const rows = times.map((time) => {
    const timeCell = `<div class="teacher-schedule-time"><strong>${time.start}</strong><span>${time.end}</span></div>`;
    const cells = weekdays.map((day) => {
      const event = visibleEvent(day.key, time.key);
      const todayClass = selectedWeek === currentWeek && day.jsDay === today ? ' is-today' : '';
      return `<div class="teacher-schedule-slot${todayClass}">${event ? eventMarkup(event) : '<span class="teacher-schedule-empty">·</span>'}</div>`;
    }).join('');
    return timeCell + cells;
  }).join('');
  table.innerHTML = head + rows;
  bindEventButtons(table);
  renderMobile();
}

function renderMobile() {
  mobileDays.innerHTML = weekdays.map((day) => {
    const active = day.key === selectedMobileDay;
    const today = selectedWeek === currentWeek && day.jsDay === now.getDay();
    return `<button type="button" data-day="${day.key}" class="teacher-mobile-day${active ? ' is-active' : ''}${today ? ' is-today' : ''}">${day.short}</button>`;
  }).join('');

  const items = times.map((time) => ({ time, event: visibleEvent(selectedMobileDay, time.key) })).filter((item) => item.event);
  mobileAgenda.innerHTML = items.length ? items.map(({ time, event }) => `
    <button class="teacher-mobile-event" type="button" data-event-id="${event.id}">
      <span class="teacher-mobile-time">${time.start}<small>${time.end}</small></span>
      <span class="teacher-mobile-copy"><span>${event.type} · ${listText(event.rooms)}</span><strong>${event.subject[1]}</strong><small>${event.groups.join(' · ')}</small></span>
    </button>`).join('') : '<div class="teacher-mobile-empty">—</div>';
  bindEventButtons(mobileAgenda);
}

function syncWeekSelector() {
  weekSelector.querySelectorAll('button').forEach((button) => {
    const current = button.dataset.week === currentWeek;
    button.classList.toggle('is-active', button.dataset.week === selectedWeek);
    button.classList.toggle('is-current', current);
    button.title = current ? `Текущая неделя · ${currentWeekNumber}` : '';
  });
}

function currentReturn() {
  return `${location.pathname.split('/').pop()}${location.search}`;
}

function detailLink(kind, value, label) {
  const target = kind === 'teacher' ? 'teacher-detail.html' : 'room-detail.html';
  const query = new URLSearchParams();
  query.set(kind, value);
  query.set('return', currentReturn());
  return `<a class="event-detail-link" href="${target}?${query.toString()}">${label}</a>`;
}

function detailLinks(kind, values) {
  const items = asList(values);
  if (!items.length) return '<span class="event-detail-empty">—</span>';
  return items.map((value) => detailLink(kind, value, value)).join('<span class="event-detail-separator"> · </span>');
}

function openEvent(id) {
  const event = events.find((item) => item.id === id);
  if (!event) return;
  const day = weekdays.find((item) => item.key === event.day);
  const time = times.find((item) => item.key === event.time);
  const rooms = asList(event.rooms);
  const teachers = asList(event.teachers);
  eventType.textContent = rooms.length ? `${event.type} · ${listText(rooms)}` : event.type;
  eventName.textContent = event.subject[0];
  eventMeta.innerHTML = [
    [event.groups.length > 1 ? 'Группы' : 'Группа', event.groups.join(', ')],
    [teachers.length > 1 ? 'Преподаватели' : 'Преподаватель', detailLinks('teacher', teachers)],
    [rooms.length > 1 ? 'Аудитории' : 'Аудитория', detailLinks('room', rooms)],
    ['Время', `${day.label} · ${time.start}–${time.end}`],
    ['Неделя', event.week === 'both' ? 'Каждую неделю' : event.week === 'even' ? 'Чётная' : 'Нечётная']
  ].map(([label, value]) => `<div class="event-detail-item"><small>${label}</small><strong>${value}</strong></div>`).join('');
  eventDialog.classList.add('is-open');
  eventDialog.setAttribute('aria-hidden', 'false');
  syncBodyLock();
  eventClose.focus();
}

function closeEvent() {
  eventDialog.classList.remove('is-open');
  eventDialog.setAttribute('aria-hidden', 'true');
  syncBodyLock();
}

function bindEventButtons(root) {
  root.querySelectorAll('[data-event-id]').forEach((button) => button.addEventListener('click', () => openEvent(button.dataset.eventId)));
}

weekSelector.addEventListener('click', (event) => {
  const button = event.target.closest('button[data-week]');
  if (!button) return;
  selectedWeek = button.dataset.week;
  syncWeekSelector();
  renderTable();
});

mobileDays.addEventListener('click', (event) => {
  const button = event.target.closest('button[data-day]');
  if (!button) return;
  selectedMobileDay = button.dataset.day;
  renderMobile();
});

eventClose.addEventListener('click', closeEvent);
eventDialog.addEventListener('click', (event) => { if (event.target === eventDialog) closeEvent(); });

// Footer stays below the first viewport and appears only after scroll.
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
  document.body.classList.toggle('modal-open', [authDialog, contactsDialog, cookieDialog, eventDialog].some((dialog) => dialog.classList.contains('is-open')));
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
authClose.addEventListener('click',()=>closeAuth(true));
contactsClose.addEventListener('click',()=>closeContacts(true));
cookieClose.addEventListener('click',()=>closeCookies(true));
authDialog.addEventListener('click',(e)=>{ if(!authPanel.contains(e.target)) closeAuth(true); });
contactsDialog.addEventListener('click',(e)=>{ if(!contactsPanel.contains(e.target)) closeContacts(true); });
cookieDialog.addEventListener('click',(e)=>{ if(!cookiePanel.contains(e.target)) closeCookies(true); });
necessaryOnly.addEventListener('click',()=>{ functionalCookies.checked=false; analyticsCookies.checked=false; persistCookiePreferences({functional:false,analytics:false}); closeCookies(true); });
acceptAllCookies.addEventListener('click',()=>{ functionalCookies.checked=true; analyticsCookies.checked=true; persistCookiePreferences({functional:true,analytics:true}); closeCookies(true); });
saveCookies.addEventListener('click',()=>{ persistCookiePreferences({functional:functionalCookies.checked,analytics:analyticsCookies.checked}); closeCookies(true); });
authForm.addEventListener('submit',(e)=>{ e.preventDefault(); closeAuth(false); });
document.addEventListener('keydown',(e)=>{
  if(e.key !== 'Escape') return;
  if(eventDialog.classList.contains('is-open')) closeEvent();
  else if(authDialog.classList.contains('is-open')) closeAuth(true);
  else if(contactsDialog.classList.contains('is-open')) closeContacts(true);
  else if(cookieDialog.classList.contains('is-open')) closeCookies(true);
});

syncWeekSelector();
renderTable();
