'use strict';
const selected = new URLSearchParams(location.search).get('active') || 'gekko';
document.querySelectorAll('[data-engine]').forEach((button) => {
  button.classList.toggle('selected',button.dataset.engine === selected);
  button.setAttribute('aria-pressed',String(button.dataset.engine === selected));
  button.onclick = () => window.gekkoPicker.choose(button.dataset.engine);
});
document.addEventListener('keydown', (event) => {
  if(event.key === 'Escape') window.gekkoPicker.dismiss();
});
