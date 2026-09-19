const params = new URLSearchParams(window.location.search);
const scheduleGroupCode = document.getElementById('scheduleGroupCode');
const scheduleBack = document.getElementById('scheduleBack');
const scheduleKind = document.getElementById('scheduleKind');
const scheduleTable = document.getElementById('scheduleTable');
const subgroupSelector = document.getElementById('subgroupSelector');
const weekSelector = document.getElementById('weekSelector');
const transfersButton = document.getElementById('transfersButton');
const transfersCount = document.getElementById('transfersCount');
const mobileSchedule = document.getElementById('mobileSchedule');
const mobileDayTabs = document.getElementById('mobileDayTabs');
const mobileGlance = document.getElementById('mobileGlance');
const mobileAgenda = document.getElementById('mobileAgenda');

const eventDialog = document.getElementById('eventDialog');
const eventPanel = eventDialog.querySelector('.schedule-event-panel');
const eventClose = document.getElementById('eventClose');
const eventStatus = document.getElementById('eventStatus');
const eventName = document.getElementById('eventName');
const eventMeta = document.getElementById('eventMeta');

const transfersDialog = document.getElementById('transfersDialog');
const transfersPanel = transfersDialog.querySelector('.transfers-panel');
const transfersClose = document.getElementById('transfersClose');
const transfersList = document.getElementById('transfersList');

const weekdays = [
  { key: 'mon', short: 'ПН', label: 'Понедельник', jsDay: 1 },
  { key: 'tue', short: 'ВТ', label: 'Вторник', jsDay: 2 },
  { key: 'wed', short: 'СР', label: 'Среда', jsDay: 3 },
  { key: 'thu', short: 'ЧТ', label: 'Четверг', jsDay: 4 },
  { key: 'fri', short: 'ПТ', label: 'Пятница', jsDay: 5 },
  { key: 'sat', short: 'СБ', label: 'Суббота', jsDay: 6 }
];

const times = [
  { key: '0830', start: '08:30', end: '09:55' },
  { key: '1010', start: '10:10', end: '11:35' },
  { key: '1145', start: '11:45', end: '13:10' },
  { key: '1400', start: '14:00', end: '15:25' },
  { key: '1535', start: '15:35', end: '17:00' },
  { key: '1710', start: '17:10', end: '18:35' }
];

const scheduleLabels = {
  regular: 'Обычное расписание',
  lecture: 'Лекционная неделя',
  credit: 'Зачётная неделя',
  exam: 'Экзаменационная неделя'
};

const teacherParam = params.get('teacher');
const roomParam = params.get('room');
const groupParam = params.get('group');
const scheduleMode = teacherParam ? 'teacher' : roomParam ? 'room' : 'group';
const group = groupParam || 'ПС-11';
const scheduleType = params.get('schedule') || 'regular';

if (scheduleMode === 'teacher') {
  scheduleGroupCode.textContent = teacherParam;
  scheduleKind.textContent = 'Преподаватель';
  scheduleBack.href = 'teacher.html';
  subgroupSelector.hidden = true;
} else if (scheduleMode === 'room') {
  scheduleGroupCode.textContent = `Ауд. ${roomParam}`;
  scheduleKind.textContent = 'Аудитория';
  scheduleBack.href = 'room.html';
  subgroupSelector.hidden = true;
} else {
  scheduleGroupCode.textContent = group;
  scheduleKind.textContent = scheduleLabels[scheduleType] || scheduleLabels.regular;
  scheduleBack.href = 'group.html';
}

const events = [
  // The demo deliberately mixes valid timetable combinations: shared groups,
  // one/two/no teachers and one/two/no rooms.
  { id:'e1', day:'mon', time:'0830', weeks:['odd','even'], subgroup:'all', groups:[group,'ПС-12'], short:'Сопротивление материалов', full:'Сопротивление материалов и основы теории упругости', rooms:['2-312','2-406'], type:'Лекция', teachers:['Ковалёв Алексей Сергеевич','Беляев Дмитрий Олегович'] },
  { id:'e2', day:'mon', time:'1010', weeks:['odd'], subgroup:'1', groups:[group], short:'Инженерная графика', full:'Инженерная и компьютерная графика', rooms:[], type:'Лаб.', teachers:['Лисовская Марина Викторовна','Титов Игорь Павлович'] },
  { id:'e3', day:'mon', time:'1010', weeks:['odd'], subgroup:'2', groups:[group], short:'Материаловедение', full:'Материаловедение и технология конструкционных материалов', rooms:['3-107','6-208'], type:'Лаб.', teachers:['Титов Игорь Павлович'] },
  { id:'e4', day:'tue', time:'1010', weeks:['odd','even'], subgroup:'all', groups:[group,'ЭТ-11'], short:'Высшая математика', full:'Высшая математика. Дифференциальные уравнения', rooms:['4-401'], type:'ПЗ', teachers:[] },
  { id:'e5', day:'tue', time:'1145', weeks:['odd','even'], subgroup:'all', groups:[group], short:'Строительная механика', full:'Строительная механика стержневых систем', rooms:['2-406'], type:'Лекция', teachers:['Беляев Дмитрий Олегович'] },
  { id:'e6', day:'wed', time:'0830', weeks:['even'], subgroup:'all', groups:[group,'МХ-11'], short:'Физика', full:'Общая физика: электричество и магнетизм', rooms:[], type:'Лекция', teachers:['Кравченко Антон Ильич'] },
  { id:'e7', day:'wed', time:'1145', weeks:['odd','even'], subgroup:'1', groups:[group], short:'Геодезия', full:'Инженерная геодезия и основы геоинформатики', rooms:['6-204'], type:'Лаб.', teachers:['Савицкий Артём Николаевич','Ковалёв Алексей Сергеевич'] },
  { id:'e8', day:'wed', time:'1145', weeks:['odd','even'], subgroup:'2', groups:[group], short:'Геодезия', full:'Инженерная геодезия и основы геоинформатики', rooms:['6-208','6-204'], type:'Лаб.', teachers:[] },
  { id:'e9', day:'thu', time:'1400', weeks:['odd','even'], subgroup:'all', groups:[group,'АРХ-21'], short:'Архитектура', full:'Архитектура гражданских и промышленных зданий', rooms:['2-212'], type:'ПЗ', teachers:['Гриневич Ольга Михайловна'] },
  { id:'e10', day:'fri', time:'0830', weeks:['odd','even'], subgroup:'all', groups:[group], short:'Иностранный язык', full:'Иностранный язык в профессиональной деятельности', rooms:['1-306'], type:'ПЗ', teachers:['Мороз Екатерина Сергеевна'], transfer:'removed', transferNote:'Снято из расписания' },
  { id:'e11', day:'fri', time:'1010', weeks:['odd','even'], subgroup:'all', groups:[group], short:'Иностранный язык', full:'Иностранный язык в профессиональной деятельности', rooms:['1-306'], type:'ПЗ', teachers:['Мороз Екатерина Сергеевна'], transfer:'added', transferNote:'Перенесено с 08:30' },
  { id:'e12', day:'fri', time:'1400', weeks:['odd'], subgroup:'all', groups:[group,'ПС-12'], short:'Основы проектирования', full:'Основы автоматизированного проектирования зданий', rooms:[], type:'ПЗ', teachers:[] },
  { id:'e13', day:'sat', time:'1010', weeks:['even'], subgroup:'all', groups:[group,'ЭК-21'], short:'Экономика', full:'Экономика строительного производства', rooms:['3-302'], type:'Лекция', teachers:['Смирнова Ирина Вадимовна','Козлов Сергей Валерьевич'], transfer:'added', transferNote:'Дополнительное занятие' },
  { id:'e14', day:'thu', time:'1535', weeks:['odd','even'], subgroup:'all', groups:[group], short:'Охрана труда', full:'Охрана труда и безопасность жизнедеятельности', rooms:['4-210','4-401'], type:'Лекция', teachers:[] }
];

function getISOWeek(date) {
  const d = new Date(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()));
  const dayNum = d.getUTCDay() || 7;
  d.setUTCDate(d.getUTCDate() + 4 - dayNum);
  const yearStart = new Date(Date.UTC(d.getUTCFullYear(), 0, 1));
  return Math.ceil((((d - yearStart) / 86400000) + 1) / 7);
}

const now = new Date();
const currentWeekNumber = getISOWeek(now);
const currentWeek = currentWeekNumber % 2 === 0 ? 'even' : 'odd';
const todayJsDay = now.getDay();
let selectedWeek = currentWeek;
let selectedSubgroup = 'all';
let selectedMobileDay = weekdays.some((day) => day.jsDay === todayJsDay)
  ? weekdays.find((day) => day.jsDay === todayJsDay).key
  : weekdays[0].key;

function asList(value) {
  if (Array.isArray(value)) return value.filter(Boolean);
  return value ? [value] : [];
}

function listText(value, empty = '—') {
  const items = asList(value);
  return items.length ? items.join(' · ') : empty;
}

function eventRooms(event) { return asList(event.rooms ?? event.room); }
function eventTeachers(event) { return asList(event.teachers ?? event.teacher); }
function eventGroups(event) { return asList(event.groups ?? group); }

function matchesScheduleEntity(event) {
  if (scheduleMode === 'teacher') return eventTeachers(event).includes(teacherParam);
  if (scheduleMode === 'room') return eventRooms(event).includes(roomParam);
  return true;
}

function visibleEventsFor(day, time) {
  return events.filter((event) => {
    if (!matchesScheduleEntity(event)) return false;
    if (event.day !== day || event.time !== time) return false;
    if (!event.weeks.includes(selectedWeek)) return false;
    if (scheduleMode === 'group' && selectedSubgroup !== 'all' && event.subgroup !== 'all' && event.subgroup !== selectedSubgroup) return false;
    return true;
  });
}

function eventMarkup(event) {
  const statusClass = event.transfer ? ` is-${event.transfer}` : '';
  const subgroup = event.subgroup === 'all' ? '' : `${event.subgroup} ПГ`;
  const groups = eventGroups(event);
  const sharedGroups = groups.length > 1 ? groups.join(' · ') : '';
  const status = event.transfer === 'added' ? 'Добавлено' : event.transfer === 'removed' ? 'Убрано' : sharedGroups || subgroup;
  return `
    <button class="schedule-event${statusClass}" type="button" data-event-id="${event.id}" aria-label="${event.full}">
      <span class="schedule-event-top"><span class="schedule-event-type">${event.type}</span><span class="schedule-event-room">${listText(eventRooms(event))}</span></span>
      <span class="schedule-event-name">${event.short}</span>
      <span class="schedule-event-foot"><span>${status}</span><span>↗</span></span>
    </button>`;
}

function bindEventButtons(root) {
  root?.querySelectorAll('[data-event-id]').forEach((button) => {
    button.addEventListener('click', () => openEvent(button.dataset.eventId));
  });
}

function renderTable() {
  const head = [`<div class="schedule-head schedule-head--time">${currentWeekNumber}</div>`]
    .concat(weekdays.map((day) => `<div class="schedule-head ${day.jsDay === todayJsDay ? 'is-today' : ''}">${day.short}</div>`))
    .join('');

  const rows = times.map((time) => {
    const timeCell = `<div class="schedule-time"><div>${time.start}<span>${time.end}</span></div></div>`;
    const dayCells = weekdays.map((day) => {
      const cellEvents = visibleEventsFor(day.key, time.key);
      const todayClass = day.jsDay === todayJsDay ? ' is-today' : '';
      const content = cellEvents.length ? cellEvents.map(eventMarkup).join('') : '<span class="schedule-empty-mark">·</span>';
      return `<div class="schedule-slot${todayClass}">${content}</div>`;
    }).join('');
    return timeCell + dayCells;
  }).join('');

  scheduleTable.innerHTML = head + rows;
  bindEventButtons(scheduleTable);
  renderMobileSchedule();
}

function minutesFromTime(value) {
  const [hours, minutes] = value.split(':').map(Number);
  return hours * 60 + minutes;
}

function getWeekMonday(baseDate = now) {
  const date = new Date(baseDate);
  const jsDay = date.getDay() || 7;
  date.setHours(12, 0, 0, 0);
  date.setDate(date.getDate() - jsDay + 1);
  return date;
}

function dateForDay(day) {
  const monday = getWeekMonday();
  if (selectedWeek !== currentWeek) monday.setDate(monday.getDate() + 7);
  const dayIndex = weekdays.findIndex((item) => item.key === day.key);
  monday.setDate(monday.getDate() + dayIndex);
  return monday;
}

function eventsForMobileDay(dayKey) {
  return times.flatMap((time) => visibleEventsFor(dayKey, time.key).map((event) => ({ event, time })));
}

function mobileEventMarkup(item) {
  const { event, time } = item;
  const statusClass = event.transfer ? ` is-${event.transfer}` : '';
  const subgroup = event.subgroup === 'all' ? '' : `${event.subgroup} ПГ`;
  const groups = eventGroups(event);
  const sharedGroups = groups.length > 1 ? groups.join(' · ') : '';
  const note = event.transfer === 'added' ? 'Добавлено' : event.transfer === 'removed' ? 'Убрано' : sharedGroups || subgroup;
  return `
    <button class="mobile-agenda-event${statusClass}" type="button" data-event-id="${event.id}" aria-label="${event.full}">
      <span class="mobile-agenda-time">${time.start}<small>${time.end}</small></span>
      <span class="mobile-agenda-body">
        <span class="mobile-agenda-top"><span>${event.type}</span><span>${listText(eventRooms(event))}</span></span>
        <span class="mobile-agenda-name">${event.short}</span>
        ${note ? `<span class="mobile-agenda-note">${note}</span>` : ''}
      </span>
      <span class="mobile-agenda-arrow" aria-hidden="true">↗</span>
    </button>`;
}

function findNextRealEvent() {
  const startDayIndex = weekdays.findIndex((day) => day.key === selectedMobileDay);
  const currentMinutes = now.getHours() * 60 + now.getMinutes();

  for (let offset = 0; offset < weekdays.length; offset += 1) {
    const day = weekdays[startDayIndex + offset];
    if (!day) break;
    const items = eventsForMobileDay(day.key).filter(({ event }) => event.transfer !== 'removed');
    for (const item of items) {
      if (offset === 0 && selectedWeek === currentWeek && day.jsDay === todayJsDay && minutesFromTime(item.time.start) <= currentMinutes) continue;
      return { ...item, day };
    }
  }
  return null;
}

function renderMobileGlance(selectedDay, items) {
  const isViewingToday = selectedWeek === currentWeek && selectedDay.jsDay === todayJsDay;
  const actualItems = items.filter(({ event }) => event.transfer !== 'removed');
  mobileGlance.className = 'mobile-glance';

  if (isViewingToday) {
    const currentMinutes = now.getHours() * 60 + now.getMinutes();
    const currentItem = actualItems.find(({ time }) => currentMinutes >= minutesFromTime(time.start) && currentMinutes <= minutesFromTime(time.end));
    if (currentItem) {
      mobileGlance.classList.add('is-now');
      mobileGlance.innerHTML = `<div class="mobile-glance-copy"><span class="mobile-glance-kicker">Сейчас</span><strong class="mobile-glance-main">${currentItem.event.short}</strong><span class="mobile-glance-meta">${currentItem.event.type} · ${listText(eventRooms(currentItem.event))}</span></div><span class="mobile-glance-time">${currentItem.time.end}</span>`;
      return;
    }

    const nextItem = actualItems.find(({ time }) => minutesFromTime(time.start) > currentMinutes);
    if (nextItem) {
      mobileGlance.innerHTML = `<div class="mobile-glance-copy"><span class="mobile-glance-kicker">Далее</span><strong class="mobile-glance-main">${nextItem.event.short}</strong><span class="mobile-glance-meta">${nextItem.event.type} · ${listText(eventRooms(nextItem.event))}</span></div><span class="mobile-glance-time">${nextItem.time.start}</span>`;
      return;
    }

    const nextAcrossWeek = findNextRealEvent();
    mobileGlance.classList.add('is-finished');
    if (nextAcrossWeek && nextAcrossWeek.day.key !== selectedDay.key) {
      mobileGlance.innerHTML = `<div class="mobile-glance-copy"><span class="mobile-glance-kicker">На сегодня всё</span><strong class="mobile-glance-main">${nextAcrossWeek.event.short}</strong><span class="mobile-glance-meta">Следующее · ${nextAcrossWeek.day.short} · ${listText(eventRooms(nextAcrossWeek.event))}</span></div><span class="mobile-glance-time">${nextAcrossWeek.time.start}</span>`;
    } else {
      mobileGlance.innerHTML = `<div class="mobile-glance-copy"><span class="mobile-glance-kicker">Сегодня</span><strong class="mobile-glance-main">На сегодня всё</strong><span class="mobile-glance-meta">${currentWeekNumber} неделя</span></div><span class="mobile-glance-time">✓</span>`;
    }
    return;
  }

  const realCount = actualItems.length;
  const first = actualItems[0];
  const date = dateForDay(selectedDay);
  const dateText = new Intl.DateTimeFormat('ru-RU', { day: '2-digit', month: 'short' }).format(date).replace('.', '');
  if (first) {
    mobileGlance.innerHTML = `<div class="mobile-glance-copy"><span class="mobile-glance-kicker">${dateText}</span><strong class="mobile-glance-main">${realCount} ${realCount === 1 ? 'занятие' : realCount < 5 ? 'занятия' : 'занятий'}</strong><span class="mobile-glance-meta">Первое · ${first.event.short}</span></div><span class="mobile-glance-time">${first.time.start}</span>`;
  } else {
    mobileGlance.classList.add('is-finished');
    mobileGlance.innerHTML = `<div class="mobile-glance-copy"><span class="mobile-glance-kicker">${dateText}</span><strong class="mobile-glance-main">Нет занятий</strong><span class="mobile-glance-meta">${selectedWeek === 'even' ? 'Чётная' : 'Нечётная'} неделя</span></div><span class="mobile-glance-time">—</span>`;
  }
}

function renderMobileSchedule() {
  if (!mobileSchedule) return;
  const selectedDay = weekdays.find((day) => day.key === selectedMobileDay) || weekdays[0];

  mobileDayTabs.innerHTML = weekdays.map((day) => {
    const date = dateForDay(day);
    const isToday = selectedWeek === currentWeek && day.jsDay === todayJsDay;
    const isActive = day.key === selectedMobileDay;
    return `<button class="mobile-day-tab${isToday ? ' is-today' : ''}${isActive ? ' is-active' : ''}" type="button" role="tab" aria-selected="${isActive}" data-mobile-day="${day.key}"><strong>${day.short}</strong><span>${String(date.getDate()).padStart(2, '0')}</span></button>`;
  }).join('');

  const items = eventsForMobileDay(selectedDay.key);
  renderMobileGlance(selectedDay, items);
  mobileAgenda.innerHTML = items.length
    ? items.map(mobileEventMarkup).join('')
    : '<div class="mobile-empty"><strong>—</strong><span>Нет занятий</span></div>';
  bindEventButtons(mobileAgenda);
}

mobileDayTabs?.addEventListener('click', (event) => {
  const button = event.target.closest('[data-mobile-day]');
  if (!button) return;
  selectedMobileDay = button.dataset.mobileDay;
  renderMobileSchedule();
});

let mobileTouchStartX = null;
mobileAgenda?.addEventListener('touchstart', (event) => {
  mobileTouchStartX = event.changedTouches[0]?.clientX ?? null;
}, { passive: true });
mobileAgenda?.addEventListener('touchend', (event) => {
  if (mobileTouchStartX == null) return;
  const endX = event.changedTouches[0]?.clientX ?? mobileTouchStartX;
  const delta = endX - mobileTouchStartX;
  mobileTouchStartX = null;
  if (Math.abs(delta) < 58) return;
  const index = weekdays.findIndex((day) => day.key === selectedMobileDay);
  const nextIndex = delta < 0 ? Math.min(weekdays.length - 1, index + 1) : Math.max(0, index - 1);
  if (nextIndex === index) return;
  selectedMobileDay = weekdays[nextIndex].key;
  renderMobileSchedule();
});

function syncSelectors() {
  subgroupSelector.querySelectorAll('button').forEach((button) => {
    button.classList.toggle('is-active', button.dataset.subgroup === selectedSubgroup);
  });
  weekSelector.querySelectorAll('button').forEach((button) => {
    const isCurrent = button.dataset.week === currentWeek;
    button.classList.toggle('is-current', isCurrent);
    button.classList.toggle('is-active', button.dataset.week === selectedWeek);
    button.title = isCurrent ? `Текущая неделя · ${currentWeekNumber}` : '';
  });
}

subgroupSelector.addEventListener('click', (event) => {
  const button = event.target.closest('button[data-subgroup]');
  if (!button) return;
  selectedSubgroup = button.dataset.subgroup;
  syncSelectors();
  renderTable();
});

weekSelector.addEventListener('click', (event) => {
  const button = event.target.closest('button[data-week]');
  if (!button) return;
  selectedWeek = button.dataset.week;
  syncSelectors();
  renderTable();
});

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

  const rooms = eventRooms(event);
  const teachers = eventTeachers(event);
  const groups = eventGroups(event);
  eventStatus.className = 'event-detail-status';
  if (event.transfer) eventStatus.classList.add(`is-${event.transfer}`);
  const statusPrefix = event.transfer === 'added' ? 'Добавлено' : event.transfer === 'removed' ? 'Убрано' : event.type;
  eventStatus.textContent = rooms.length ? `${statusPrefix} · ${listText(rooms)}` : statusPrefix;
  eventName.textContent = event.full;

  const time = times.find((item) => item.key === event.time);
  const day = weekdays.find((item) => item.key === event.day);
  const subgroup = event.subgroup === 'all' ? 'Все' : event.subgroup;
  const meta = [
    [groups.length > 1 ? 'Группы' : 'Группа', groups.length ? groups.join(', ') : '—'],
    [teachers.length > 1 ? 'Преподаватели' : 'Преподаватель', detailLinks('teacher', teachers)],
    [rooms.length > 1 ? 'Аудитории' : 'Аудитория', detailLinks('room', rooms)],
    ['Время', `${day.label} · ${time.start}–${time.end}`],
    ['Подгруппа', subgroup]
  ];
  if (event.transferNote) meta.push(['Перенос', event.transferNote]);
  eventMeta.innerHTML = meta.map(([label, value]) => `<div class="event-detail-item"><small>${label}</small><strong>${value}</strong></div>`).join('');

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

const transferEvents = events.filter((event) => event.transfer && matchesScheduleEntity(event));
transfersCount.textContent = String(transferEvents.length);
transfersList.innerHTML = transferEvents.map((event) => {
  const time = times.find((item) => item.key === event.time);
  const day = weekdays.find((item) => item.key === event.day);
  const sign = event.transfer === 'added' ? '+' : '−';
  return `<div class="transfer-row is-${event.transfer}">
    <span class="transfer-sign">${sign}</span>
    <div><div class="transfer-subject">${event.short}</div><div class="transfer-meta">${event.transferNote}</div></div>
    <div class="transfer-where">${day.short}<br>${time.start}<br>${listText(eventRooms(event))}</div>
  </div>`;
}).join('');

function openTransfers() {
  transfersDialog.classList.add('is-open');
  transfersDialog.setAttribute('aria-hidden', 'false');
  transfersButton.setAttribute('aria-expanded', 'true');
  syncBodyLock();
  transfersClose.focus();
}
function closeTransfers() {
  transfersDialog.classList.remove('is-open');
  transfersDialog.setAttribute('aria-hidden', 'true');
  transfersButton.setAttribute('aria-expanded', 'false');
  syncBodyLock();
}

transfersButton.addEventListener('click', openTransfers);
transfersClose.addEventListener('click', closeTransfers);
eventClose.addEventListener('click', closeEvent);
transfersDialog.addEventListener('click', (event) => { if (!transfersPanel.contains(event.target)) closeTransfers(); });
eventDialog.addEventListener('click', (event) => { if (!eventPanel.contains(event.target)) closeEvent(); });

function updateFooterVisibility() {
  document.body.classList.toggle('footer-visible', window.scrollY > 2);
}
window.addEventListener('scroll', updateFooterVisibility, { passive: true });
updateFooterVisibility();

// Shared dialogs
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
  const anyOpen = [eventDialog, transfersDialog, authDialog, contactsDialog, cookieDialog].some((dialog) => dialog.classList.contains('is-open'));
  document.body.classList.toggle('modal-open', anyOpen);
}

function closeAuth(returnFocus = false) {
  authDialog.classList.remove('is-open'); authDialog.setAttribute('aria-hidden', 'true'); profileButton.setAttribute('aria-expanded', 'false'); syncBodyLock(); if (returnFocus) profileButton.focus();
}
function openAuth() {
  closeContacts(false); closeCookies(false); closeEvent(); closeTransfers();
  authDialog.classList.add('is-open'); authDialog.setAttribute('aria-hidden', 'false'); profileButton.setAttribute('aria-expanded', 'true'); syncBodyLock(); window.setTimeout(() => loginInput.focus(), 80);
}
function closeContacts(returnFocus = false) {
  contactsDialog.classList.remove('is-open'); contactsDialog.setAttribute('aria-hidden', 'true'); contactsButton.setAttribute('aria-expanded', 'false'); syncBodyLock(); if (returnFocus) contactsButton.focus();
}
function openContacts() {
  closeAuth(false); closeCookies(false); closeEvent(); closeTransfers();
  contactsDialog.classList.add('is-open'); contactsDialog.setAttribute('aria-hidden', 'false'); contactsButton.setAttribute('aria-expanded', 'true'); syncBodyLock(); window.setTimeout(() => firstContact?.focus(), 80);
}
function closeCookies(returnFocus = false) {
  cookieDialog.classList.remove('is-open'); cookieDialog.setAttribute('aria-hidden', 'true'); cookieButton.setAttribute('aria-expanded', 'false'); syncBodyLock(); if (returnFocus) cookieButton.focus();
}
function readCookiePreferences() { try { return JSON.parse(localStorage.getItem(COOKIE_PREFERENCES_KEY)) || {}; } catch { return {}; } }
function loadCookiePreferences() { const p = readCookiePreferences(); functionalCookies.checked = Boolean(p.functional); analyticsCookies.checked = Boolean(p.analytics); }
function setCookie(name, value, days = 365) { const maxAge = days * 24 * 60 * 60; document.cookie = `${name}=${encodeURIComponent(value)}; max-age=${maxAge}; path=/; SameSite=Lax`; }
function removeCookie(name) { document.cookie = `${name}=; max-age=0; path=/; SameSite=Lax`; }
function persistCookiePreferences({ functional, analytics }) {
  localStorage.setItem(COOKIE_PREFERENCES_KEY, JSON.stringify({ necessary:true, functional, analytics, updatedAt:new Date().toISOString() }));
  setCookie('schedule_cookie_consent','saved'); functional ? setCookie('schedule_functional','1') : removeCookie('schedule_functional'); analytics ? setCookie('schedule_analytics','1') : removeCookie('schedule_analytics');
}
function openCookies() {
  closeAuth(false); closeContacts(false); closeEvent(); closeTransfers(); loadCookiePreferences();
  cookieDialog.classList.add('is-open'); cookieDialog.setAttribute('aria-hidden', 'false'); cookieButton.setAttribute('aria-expanded', 'true'); syncBodyLock(); window.setTimeout(() => functionalCookies.focus(), 80);
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
necessaryOnly.addEventListener('click', () => { functionalCookies.checked=false; analyticsCookies.checked=false; persistCookiePreferences({functional:false,analytics:false}); closeCookies(true); });
acceptAllCookies.addEventListener('click', () => { functionalCookies.checked=true; analyticsCookies.checked=true; persistCookiePreferences({functional:true,analytics:true}); closeCookies(true); });
saveCookies.addEventListener('click', () => { persistCookiePreferences({functional:functionalCookies.checked,analytics:analyticsCookies.checked}); closeCookies(true); });

document.addEventListener('keydown', (event) => {
  if (event.key !== 'Escape') return;
  if (eventDialog.classList.contains('is-open')) closeEvent();
  else if (transfersDialog.classList.contains('is-open')) closeTransfers();
  else if (cookieDialog.classList.contains('is-open')) closeCookies(true);
  else if (contactsDialog.classList.contains('is-open')) closeContacts(true);
  else if (authDialog.classList.contains('is-open')) closeAuth(true);
});

syncSelectors();
renderTable();
