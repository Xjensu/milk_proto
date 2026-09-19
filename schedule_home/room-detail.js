const roomProfiles = [
  { value:'2-312', code:'ROOM-2-312', type:'Лекционная аудитория', building:'Корпус 2', capacity:'80 мест', equipment:'Проектор · доска', subjects:[['Сопротивление материалов и основы теории упругости','Сопр. материалов'],['Строительные конструкции','Строит. конструкции'],['Охрана труда и безопасность жизнедеятельности','Охрана труда']], teachers:['Ковалёв Алексей Сергеевич','Беляев Дмитрий Олегович','Козлов Сергей Валерьевич'], groups:[['ПГС-21','ПГС-22'],['ПГС-31'],['УПП-31','ЭТ-31']] },
  { value:'1-218', code:'ROOM-1-218', type:'Компьютерный класс', building:'Корпус 1', capacity:'24 места', equipment:'24 ПК · проектор', subjects:[['Инженерная и компьютерная графика','Инж. графика'],['Начертательная геометрия','Начерт. геометрия'],['Основы автоматизированного проектирования зданий','Основы САПР']], teachers:['Лисовская Марина Викторовна','Лисовская Марина Викторовна','Климович Павел Андреевич'], groups:[['ПГС-11'],['АРХ-21'],['ПГС-31']] },
  { value:'3-107', code:'ROOM-3-107', type:'Лаборатория', building:'Корпус 3', capacity:'20 мест', equipment:'Лабораторные стенды', subjects:[['Материаловедение и технология конструкционных материалов','Материаловедение'],['Испытание строительных материалов','Испытание материалов'],['Строительная механика','Строит. механика']], teachers:['Титов Игорь Павлович','Титов Игорь Павлович','Беляев Дмитрий Олегович'], groups:[['МХ-21'],['ПГС-23'],['ПГС-31']] },
  { value:'4-401', code:'ROOM-4-401', type:'Учебная аудитория', building:'Корпус 4', capacity:'42 места', equipment:'Доска · экран', subjects:[['Высшая математика. Дифференциальные уравнения','Высшая математика'],['Архитектурное проектирование','Арх. проектирование'],['Теория вероятностей и математическая статистика','ТВиМС']], teachers:['Романова Елена Андреевна','Гриневич Ольга Михайловна','Романова Елена Андреевна'], groups:[['ЭТ-11'],['АРХ-21'],['ЭК-12']] },
  { value:'2-406', code:'ROOM-2-406', type:'Лекционная аудитория', building:'Корпус 2', capacity:'120 мест', equipment:'Амфитеатр · проектор', subjects:[['Строительная механика стержневых систем','Строит. механика'],['Железобетонные конструкции','ЖБ конструкции'],['Инженерная геодезия','Инж. геодезия']], teachers:['Беляев Дмитрий Олегович','Ковалёв Алексей Сергеевич','Савицкий Артём Николаевич'], groups:[['ПГС-31','ПГС-32'],['ПГС-21','ПГС-22'],['ГД-21','ГД-22']] },
  { value:'6-204', code:'ROOM-6-204', type:'Геодезическая лаборатория', building:'Корпус 6', capacity:'18 мест', equipment:'Тахеометры · GNSS', subjects:[['Инженерная геодезия и основы геоинформатики','Инж. геодезия'],['Спутниковые технологии позиционирования','GNSS'],['Геодезическое обеспечение строительства','Геодезия']], teachers:['Савицкий Артём Николаевич','Савицкий Артём Николаевич','Савицкий Артём Николаевич'], groups:[['ГД-21'],['ГД-22'],['ПГС-21']] },
  { value:'1-306', code:'ROOM-1-306', type:'Лингафонный кабинет', building:'Корпус 1', capacity:'30 мест', equipment:'Аудиосистема · ПК', subjects:[['Иностранный язык в профессиональной деятельности','Иностранный язык'],['Технический английский язык','Тех. английский'],['Деловая коммуникация','Деловая коммуникация']], teachers:['Мороз Екатерина Сергеевна','Мороз Екатерина Сергеевна','Мороз Екатерина Сергеевна'], groups:[['ЭК-21'],['ПГС-21'],['МХ-21']] },
  { value:'7-411', code:'ROOM-7-411', type:'Компьютерный класс', building:'Корпус 7', capacity:'28 мест', equipment:'САПР · 28 ПК', subjects:[['Основы автоматизированного проектирования зданий','Основы САПР'],['Архитектурное проектирование','Арх. проектирование'],['Инженерная и компьютерная графика','Инж. графика']], teachers:['Климович Павел Андреевич','Гриневич Ольга Михайловна','Лисовская Марина Викторовна'], groups:[['ПГС-31'],['АРХ-31'],['ПГС-12']] },
  { value:'4-210', code:'ROOM-4-210', type:'Лекционная аудитория', building:'Корпус 4', capacity:'60 мест', equipment:'Проектор · микрофон', subjects:[['Охрана труда и безопасность жизнедеятельности','Охрана труда'],['Промышленная безопасность','Пром. безопасность'],['Экономика строительного производства','Экономика']], teachers:['Козлов Сергей Валерьевич','Козлов Сергей Валерьевич','Смирнова Ирина Вадимовна'], groups:[['УПП-31','ПГС-31'],['ЭТ-31'],['ПГС-32']] },
  { value:'5-115', code:'ROOM-5-115', type:'Лекционная аудитория', building:'Корпус 5', capacity:'72 места', equipment:'Демонстрационный стол · проектор', subjects:[['Общая физика: электричество и магнетизм','Физика'],['Механика и молекулярная физика','Механика'],['Физический практикум','Физ. практикум']], teachers:['Кравченко Антон Ильич','Кравченко Антон Ильич','Кравченко Антон Ильич'], groups:[['ПГС-11','ЭТ-11'],['МХ-11'],['АРХ-11']] },
  { value:'6-208', code:'ROOM-6-208', type:'Геодезическая лаборатория', building:'Корпус 6', capacity:'18 мест', equipment:'Тахеометры · GNSS', subjects:[['Инженерная геодезия и основы геоинформатики','Инж. геодезия'],['Спутниковые технологии позиционирования','GNSS'],['Геодезическое обеспечение строительства','Геодезия']], teachers:['Савицкий Артём Николаевич','Савицкий Артём Николаевич','Савицкий Артём Николаевич'], groups:[['ПГС-11'],['ГД-22'],['ПГС-22']] },
  { value:'2-212', code:'ROOM-2-212', type:'Учебная аудитория', building:'Корпус 2', capacity:'36 мест', equipment:'Чертёжные столы · экран', subjects:[['Архитектура гражданских и промышленных зданий','Архитектура'],['Архитектурное проектирование','Арх. проектирование'],['Композиционное моделирование','Композиция']], teachers:['Гриневич Ольга Михайловна','Гриневич Ольга Михайловна','Гриневич Ольга Михайловна'], groups:[['ПГС-11'],['АРХ-21'],['АРХ-22']] },
  { value:'3-302', code:'ROOM-3-302', type:'Лекционная аудитория', building:'Корпус 3', capacity:'64 места', equipment:'Проектор · экран', subjects:[['Экономика строительного производства','Экономика'],['Организация строительного производства','Организация'],['Управление проектами','Упр. проектами']], teachers:['Смирнова Ирина Вадимовна','Смирнова Ирина Вадимовна','Смирнова Ирина Вадимовна'], groups:[['ПГС-11'],['ПГС-31'],['ЭК-21']] }
];

const params = new URLSearchParams(location.search);
const requestedRoom = params.get('room');
const room = roomProfiles.find((item) => item.value === requestedRoom) || roomProfiles[0];
const returnUrl = params.get('return');
const roomBack = document.getElementById('roomRecordBack');
if (returnUrl) roomBack.href = returnUrl;

document.title = `Аудитория ${room.value}`;
document.getElementById('roomRecordNumber').textContent = room.code;
document.getElementById('roomRecordFile').textContent = room.code;
document.getElementById('roomRecordName').textContent = room.value;
document.getElementById('roomSchedulePerson').textContent = `${room.value} · ${room.type}`;
document.getElementById('roomRecordFields').innerHTML = [
  ['Аудитория', room.value],
  ['Тип', room.type],
  ['Корпус', room.building],
  ['Вместимость', room.capacity],
  ['Оснащение', room.equipment]
].map(([label,value]) => `<div class="teacher-record-field"><span>${label}</span><strong>${value}</strong></div>`).join('');

// Абстрактный пиксельный план аудитории, использующий тот же бордовый акцент.
const planPattern = [
  '00000000000000000000','00111111111111111100','00100000000000000100','00102220222022200100',
  '00100000000000000100','00102220222022200100','00100000000000000100','00102220222022200100',
  '00100000000000000100','00100000000000000100','00122222222222222100','00120000000000002100',
  '00120000000000002100','00120001111000002100','00120001111000002100','00120000000000002100',
  '00122222222222222100','00100000000000000100','00111111111111111100','00000000000000000000',
  '00000001111110000000','00000001111110000000'
];
document.getElementById('roomPixelPlan').innerHTML = planPattern.flatMap((row)=>[...row]).map((cell)=>`<i class="pixel-${cell}"></i>`).join('');

const weekdays = [
  { key:'mon', short:'ПН', label:'Понедельник', jsDay:1 },{ key:'tue', short:'ВТ', label:'Вторник', jsDay:2 },
  { key:'wed', short:'СР', label:'Среда', jsDay:3 },{ key:'thu', short:'ЧТ', label:'Четверг', jsDay:4 },
  { key:'fri', short:'ПТ', label:'Пятница', jsDay:5 },{ key:'sat', short:'СБ', label:'Суббота', jsDay:6 }
];
const times = [
  { key:'1', start:'08:30', end:'10:00' },{ key:'2', start:'10:10', end:'11:40' },{ key:'3', start:'11:50', end:'13:20' },
  { key:'4', start:'14:00', end:'15:30' },{ key:'5', start:'15:40', end:'17:10' },{ key:'6', start:'17:20', end:'18:50' }
];
function isoWeek(date){const d=new Date(Date.UTC(date.getFullYear(),date.getMonth(),date.getDate()));const day=d.getUTCDay()||7;d.setUTCDate(d.getUTCDate()+4-day);const y=new Date(Date.UTC(d.getUTCFullYear(),0,1));return Math.ceil((((d-y)/86400000)+1)/7);}
const now = new Date();
const currentWeekNumber = isoWeek(now);
const currentWeek = currentWeekNumber % 2 === 0 ? 'even' : 'odd';
let selectedWeek = currentWeek;
let selectedMobileDay = weekdays.find((d)=>d.jsDay===now.getDay())?.key || 'mon';

function uniqueList(values){return [...new Set(values.filter(Boolean))];}
function asList(value){if(Array.isArray(value))return value.filter(Boolean);return value?[value]:[];}
function listText(value,empty='—'){const items=asList(value);return items.length?items.join(' · '):empty;}

function buildEvents(profile){
  const s=profile.subjects,t=profile.teachers,g=profile.groups;
  const secondRoom=profile.value==='2-312'?'2-406':'2-312';
  const extraTeacher=t.find((name)=>name&&name!==t[0])||'Ковалёв Алексей Сергеевич';
  return [
    {id:'r1',day:'mon',time:'1',week:'both',subject:s[0],type:'Лекция',rooms:uniqueList([profile.value,secondRoom]),teachers:uniqueList([t[0],extraTeacher]),groups:g[0]},
    {id:'r2',day:'mon',time:'3',week:'odd',subject:s[1],type:'ПЗ',rooms:[profile.value],teachers:[t[1]],groups:g[1]},
    {id:'r3',day:'tue',time:'2',week:'both',subject:s[2],type:'Лаб.',rooms:[profile.value],teachers:[],groups:uniqueList([...(g[2]||[]),...(g[0]||[])]).slice(0,2)},
    {id:'r4',day:'wed',time:'1',week:'even',subject:s[1],type:'Лекция',rooms:uniqueList([profile.value,secondRoom]),teachers:uniqueList([t[1],t[2]]),groups:g[0]},
    {id:'r5',day:'wed',time:'4',week:'both',subject:s[0],type:'ПЗ',rooms:[profile.value],teachers:[t[0]],groups:g[2]},
    {id:'r6',day:'thu',time:'2',week:'both',subject:s[2],type:'Лекция',rooms:[profile.value],teachers:uniqueList([t[2],extraTeacher]),groups:g[1]},
    {id:'r7',day:'fri',time:'3',week:'odd',subject:s[0],type:'Лаб.',rooms:[profile.value],teachers:[],groups:g[0]},
    {id:'r8',day:'sat',time:'1',week:'even',subject:s[1],type:'Конс.',rooms:[profile.value],teachers:[t[1]],groups:g[2]}
  ];
}
const events = buildEvents(room);
const table=document.getElementById('roomScheduleTable');
const weekSelector=document.getElementById('roomWeekSelector');
const mobileDays=document.getElementById('roomMobileDays');
const mobileAgenda=document.getElementById('roomMobileAgenda');
const eventDialog=document.getElementById('roomEventDialog');
const eventClose=document.getElementById('roomEventClose');
const eventType=document.getElementById('roomEventType');
const eventName=document.getElementById('roomEventName');
const eventMeta=document.getElementById('roomEventMeta');

function visibleEvent(day,time){return events.find((e)=>e.day===day&&e.time===time&&(e.week==='both'||e.week===selectedWeek));}
function eventMarkup(e){return `<button class="teacher-schedule-event room-schedule-event" type="button" data-event-id="${e.id}" aria-label="${e.subject[0]}"><span class="teacher-schedule-event-top"><span>${e.type}</span><span>${e.groups.join(' · ')}</span></span><span class="teacher-schedule-event-name">${e.subject[1]}</span><span class="teacher-schedule-event-groups">${listText(e.teachers)}</span></button>`;}
function bindEventButtons(root){root.querySelectorAll('[data-event-id]').forEach((b)=>b.addEventListener('click',()=>openEvent(b.dataset.eventId)));}
function renderTable(){
  const today=now.getDay();
  const head=`<div class="teacher-schedule-head teacher-schedule-head--time">TIME</div>${weekdays.map((d)=>`<div class="teacher-schedule-head${selectedWeek===currentWeek&&d.jsDay===today?' is-today':''}">${d.short}</div>`).join('')}`;
  const rows=times.map((time)=>`<div class="teacher-schedule-time"><strong>${time.start}</strong><span>${time.end}</span></div>${weekdays.map((d)=>{const e=visibleEvent(d.key,time.key);return `<div class="teacher-schedule-slot${selectedWeek===currentWeek&&d.jsDay===today?' is-today':''}">${e?eventMarkup(e):'<span class="teacher-schedule-empty">·</span>'}</div>`;}).join('')}`).join('');
  table.innerHTML=head+rows;bindEventButtons(table);renderMobile();
}
function renderMobile(){
  mobileDays.innerHTML=weekdays.map((d)=>`<button type="button" data-day="${d.key}" class="teacher-mobile-day${d.key===selectedMobileDay?' is-active':''}${selectedWeek===currentWeek&&d.jsDay===now.getDay()?' is-today':''}">${d.short}</button>`).join('');
  const items=times.map((time)=>({time,event:visibleEvent(selectedMobileDay,time.key)})).filter((x)=>x.event);
  mobileAgenda.innerHTML=items.length?items.map(({time,event:e})=>`<button class="teacher-mobile-event" type="button" data-event-id="${e.id}"><span class="teacher-mobile-time">${time.start}<small>${time.end}</small></span><span class="teacher-mobile-copy"><span>${e.type} · ${e.groups.join(' · ')}</span><strong>${e.subject[1]}</strong><small>${listText(e.teachers)}</small></span></button>`).join(''):'<div class="teacher-mobile-empty">—</div>';
  bindEventButtons(mobileAgenda);
}
function syncWeekSelector(){weekSelector.querySelectorAll('button').forEach((b)=>{const current=b.dataset.week===currentWeek;b.classList.toggle('is-active',b.dataset.week===selectedWeek);b.classList.toggle('is-current',current);b.title=current?`Текущая неделя · ${currentWeekNumber}`:'';});}
function currentReturn(){return `${location.pathname.split('/').pop()}${location.search}`;}
function detailLink(kind,value,label){const target=kind==='teacher'?'teacher-detail.html':'room-detail.html';const q=new URLSearchParams();q.set(kind,value);q.set('return',currentReturn());return `<a class="event-detail-link" href="${target}?${q.toString()}">${label}</a>`;}
function detailLinks(kind,values){const items=asList(values);if(!items.length)return '<span class="event-detail-empty">—</span>';return items.map((value)=>detailLink(kind,value,value)).join('<span class="event-detail-separator"> · </span>');}
function openEvent(id){
  const e=events.find((x)=>x.id===id);if(!e)return;const day=weekdays.find((x)=>x.key===e.day);const time=times.find((x)=>x.key===e.time);
  const teachers=asList(e.teachers),rooms=asList(e.rooms),groups=asList(e.groups);
  eventType.textContent=`${e.type} · ${groups.length?groups.join(', '):'—'}`;eventName.textContent=e.subject[0];
  eventMeta.innerHTML=[
    [groups.length>1?'Группы':'Группа',groups.length?groups.join(', '):'—'],
    [teachers.length>1?'Преподаватели':'Преподаватель',detailLinks('teacher',teachers)],
    [rooms.length>1?'Аудитории':'Аудитория',detailLinks('room',rooms)],
    ['Время',`${day.label} · ${time.start}–${time.end}`],['Неделя',e.week==='both'?'Каждую неделю':e.week==='even'?'Чётная':'Нечётная']
  ].map(([label,value])=>`<div class="event-detail-item"><small>${label}</small><strong>${value}</strong></div>`).join('');
  eventDialog.classList.add('is-open');eventDialog.setAttribute('aria-hidden','false');syncBodyLock();eventClose.focus();
}
function closeEvent(){eventDialog.classList.remove('is-open');eventDialog.setAttribute('aria-hidden','true');syncBodyLock();}
weekSelector.addEventListener('click',(ev)=>{const b=ev.target.closest('button[data-week]');if(!b)return;selectedWeek=b.dataset.week;syncWeekSelector();renderTable();});
mobileDays.addEventListener('click',(ev)=>{const b=ev.target.closest('button[data-day]');if(!b)return;selectedMobileDay=b.dataset.day;renderMobile();});
eventClose.addEventListener('click',closeEvent);eventDialog.addEventListener('click',(ev)=>{if(ev.target===eventDialog)closeEvent();});

function updateFooterVisibility(){document.body.classList.toggle('footer-visible',window.scrollY>2);}window.addEventListener('scroll',updateFooterVisibility,{passive:true});updateFooterVisibility();
const profileButton=document.querySelector('.profile-button'),authDialog=document.getElementById('authDialog'),authPanel=authDialog.querySelector('.auth-panel'),authClose=authDialog.querySelector('.auth-close'),authForm=document.getElementById('authForm'),loginInput=authForm.querySelector('input[name="login"]'),contactsButton=document.getElementById('contactsButton'),contactsDialog=document.getElementById('contactsDialog'),contactsPanel=contactsDialog.querySelector('.contacts-panel'),contactsClose=contactsDialog.querySelector('.contacts-close'),firstContact=contactsDialog.querySelector('.contact-item'),cookieButton=document.getElementById('cookieButton'),cookieDialog=document.getElementById('cookieDialog'),cookiePanel=cookieDialog.querySelector('.cookie-panel'),cookieClose=cookieDialog.querySelector('.cookie-close'),functionalCookies=document.getElementById('functionalCookies'),analyticsCookies=document.getElementById('analyticsCookies'),necessaryOnly=document.getElementById('necessaryOnly'),acceptAllCookies=document.getElementById('acceptAllCookies'),saveCookies=document.getElementById('saveCookies');
const COOKIE_PREFERENCES_KEY='schedule-cookie-preferences';
function syncBodyLock(){document.body.classList.toggle('modal-open',[authDialog,contactsDialog,cookieDialog,eventDialog].some((d)=>d.classList.contains('is-open')));}
function closeAuth(r=false){authDialog.classList.remove('is-open');authDialog.setAttribute('aria-hidden','true');profileButton.setAttribute('aria-expanded','false');syncBodyLock();if(r)profileButton.focus();}
function closeContacts(r=false){contactsDialog.classList.remove('is-open');contactsDialog.setAttribute('aria-hidden','true');contactsButton.setAttribute('aria-expanded','false');syncBodyLock();if(r)contactsButton.focus();}
function closeCookies(r=false){cookieDialog.classList.remove('is-open');cookieDialog.setAttribute('aria-hidden','true');cookieButton.setAttribute('aria-expanded','false');syncBodyLock();if(r)cookieButton.focus();}
function openAuth(){closeContacts();closeCookies();authDialog.classList.add('is-open');authDialog.setAttribute('aria-hidden','false');profileButton.setAttribute('aria-expanded','true');syncBodyLock();setTimeout(()=>loginInput.focus(),80);}
function openContacts(){closeAuth();closeCookies();contactsDialog.classList.add('is-open');contactsDialog.setAttribute('aria-hidden','false');contactsButton.setAttribute('aria-expanded','true');syncBodyLock();setTimeout(()=>firstContact?.focus(),80);}
function loadCookiePreferences(){try{const p=JSON.parse(localStorage.getItem(COOKIE_PREFERENCES_KEY))||{};functionalCookies.checked=Boolean(p.functional);analyticsCookies.checked=Boolean(p.analytics);}catch{}}
function setCookie(n,v,d=365){document.cookie=`${n}=${encodeURIComponent(v)}; max-age=${d*86400}; path=/; SameSite=Lax`;}
function removeCookie(n){document.cookie=`${n}=; max-age=0; path=/; SameSite=Lax`;}
function persistCookiePreferences({functional,analytics}){localStorage.setItem(COOKIE_PREFERENCES_KEY,JSON.stringify({necessary:true,functional,analytics,updatedAt:new Date().toISOString()}));setCookie('schedule_cookie_consent','saved');functional?setCookie('schedule_functional','1'):removeCookie('schedule_functional');analytics?setCookie('schedule_analytics','1'):removeCookie('schedule_analytics');}
function openCookies(){closeAuth();closeContacts();loadCookiePreferences();cookieDialog.classList.add('is-open');cookieDialog.setAttribute('aria-hidden','false');cookieButton.setAttribute('aria-expanded','true');syncBodyLock();setTimeout(()=>functionalCookies.focus(),80);}
profileButton.addEventListener('click',()=>authDialog.classList.contains('is-open')?closeAuth(true):openAuth());contactsButton.addEventListener('click',()=>contactsDialog.classList.contains('is-open')?closeContacts(true):openContacts());cookieButton.addEventListener('click',openCookies);authClose.addEventListener('click',()=>closeAuth(true));contactsClose.addEventListener('click',()=>closeContacts(true));cookieClose.addEventListener('click',()=>closeCookies(true));authDialog.addEventListener('click',(e)=>{if(!authPanel.contains(e.target))closeAuth(true);});contactsDialog.addEventListener('click',(e)=>{if(!contactsPanel.contains(e.target))closeContacts(true);});cookieDialog.addEventListener('click',(e)=>{if(!cookiePanel.contains(e.target))closeCookies(true);});necessaryOnly.addEventListener('click',()=>{functionalCookies.checked=false;analyticsCookies.checked=false;persistCookiePreferences({functional:false,analytics:false});closeCookies(true);});acceptAllCookies.addEventListener('click',()=>{functionalCookies.checked=true;analyticsCookies.checked=true;persistCookiePreferences({functional:true,analytics:true});closeCookies(true);});saveCookies.addEventListener('click',()=>{persistCookiePreferences({functional:functionalCookies.checked,analytics:analyticsCookies.checked});closeCookies(true);});authForm.addEventListener('submit',(e)=>{e.preventDefault();closeAuth();});
document.addEventListener('keydown',(e)=>{if(e.key!=='Escape')return;if(eventDialog.classList.contains('is-open'))closeEvent();else if(authDialog.classList.contains('is-open'))closeAuth(true);else if(contactsDialog.classList.contains('is-open'))closeContacts(true);else if(cookieDialog.classList.contains('is-open'))closeCookies(true);});
syncWeekSelector();renderTable();
