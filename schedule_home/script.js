const floors = [...document.querySelectorAll('.floor')];
const transitionLayer = document.getElementById('transitionLayer');
const transitionText = transitionLayer.querySelector('span');

const profileButton = document.querySelector('.profile-button');
const authDialog = document.getElementById('authDialog');
const authPanel = authDialog.querySelector('.auth-panel');
const authClose = authDialog.querySelector('.auth-close');
const authForm = document.getElementById('authForm');
const loginInput = authForm.querySelector('input[name="login"]');

const guideButton = document.getElementById('guideButton');
const guideDialog = document.getElementById('guideDialog');
const guidePanel = guideDialog.querySelector('.guide-panel');
const guideClose = guideDialog.querySelector('.guide-close');
const guideSteps = [...guideDialog.querySelectorAll('.guide-step')];

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

const routeLabels = {
  group: 'Группа',
  teacher: 'Преподаватель',
  room: 'Аудитория'
};

function chooseFloor(button) {
  floors.forEach((floor) => floor.classList.toggle('is-active', floor === button));

  const target = button.dataset.target;
  transitionText.textContent = routeLabels[target];
  transitionLayer.classList.add('is-open');
  transitionLayer.setAttribute('aria-hidden', 'false');

  window.setTimeout(() => {
    const routes = {
      group: 'group.html',
      teacher: 'teacher.html',
      room: 'room.html'
    };
    window.location.href = routes[target] || 'index.html';
  }, 540);
}

function syncBodyLock() {
  const anyDialogOpen = [authDialog, guideDialog, contactsDialog, cookieDialog]
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

function closeGuide(returnFocus = false) {
  guideDialog.classList.remove('is-open');
  guideDialog.setAttribute('aria-hidden', 'true');
  guideButton.setAttribute('aria-expanded', 'false');
  syncBodyLock();
  if (returnFocus) guideButton.focus();
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
  if (except !== 'guide') closeGuide(false);
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

function openGuide() {
  closeOtherDialogs('guide');
  guideDialog.classList.add('is-open');
  guideDialog.setAttribute('aria-hidden', 'false');
  guideButton.setAttribute('aria-expanded', 'true');
  syncBodyLock();
  window.setTimeout(() => guideSteps[0]?.focus(), 80);
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
  try {
    return JSON.parse(localStorage.getItem(COOKIE_PREFERENCES_KEY)) || {};
  } catch {
    return {};
  }
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
  const preferences = {
    necessary: true,
    functional,
    analytics,
    updatedAt: new Date().toISOString()
  };

  localStorage.setItem(COOKIE_PREFERENCES_KEY, JSON.stringify(preferences));
  setCookie('schedule_cookie_consent', 'saved');

  if (functional) setCookie('schedule_functional', '1');
  else removeCookie('schedule_functional');

  if (analytics) setCookie('schedule_analytics', '1');
  else removeCookie('schedule_analytics');
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

floors.forEach((floor) => {
  floor.addEventListener('click', () => chooseFloor(floor));
});

profileButton.addEventListener('click', () => {
  authDialog.classList.contains('is-open') ? closeAuth(true) : openAuth();
});

guideButton.addEventListener('click', () => {
  window.location.href = 'guide.html';
});


guideSteps.forEach((step) => {
  step.addEventListener('click', () => {
    const floor = floors.find((item) => item.dataset.target === step.dataset.guideTarget);
    closeGuide(false);
    if (floor) chooseFloor(floor);
  });
});

contactsButton.addEventListener('click', () => {
  contactsDialog.classList.contains('is-open') ? closeContacts(true) : openContacts();
});

cookieButton.addEventListener('click', openCookies);

authClose.addEventListener('click', () => closeAuth(true));
guideClose.addEventListener('click', () => closeGuide(true));
contactsClose.addEventListener('click', () => closeContacts(true));
cookieClose.addEventListener('click', () => closeCookies(true));

authDialog.addEventListener('click', (event) => {
  if (!authPanel.contains(event.target)) closeAuth(true);
});

guideDialog.addEventListener('click', (event) => {
  if (!guidePanel.contains(event.target)) closeGuide(true);
});

contactsDialog.addEventListener('click', (event) => {
  if (!contactsPanel.contains(event.target)) closeContacts(true);
});

cookieDialog.addEventListener('click', (event) => {
  if (!cookiePanel.contains(event.target)) closeCookies(true);
});

authForm.addEventListener('submit', (event) => {
  event.preventDefault();
  // Здесь можно подключить реальный POST-запрос авторизации.
});

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
  persistCookiePreferences({
    functional: functionalCookies.checked,
    analytics: analyticsCookies.checked
  });
  closeCookies(true);
});

document.addEventListener('keydown', (event) => {
  if (event.key !== 'Escape') return;

  if (cookieDialog.classList.contains('is-open')) closeCookies(true);
  else if (contactsDialog.classList.contains('is-open')) closeContacts(true);
  else if (guideDialog.classList.contains('is-open')) closeGuide(true);
  else if (authDialog.classList.contains('is-open')) closeAuth(true);
});

const initialRoute = window.location.hash.slice(1);
const initialFloor = floors.find((floor) => floor.dataset.target === initialRoute);
if (initialFloor) initialFloor.classList.add('is-active');
