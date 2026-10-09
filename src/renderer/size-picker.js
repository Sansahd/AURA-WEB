'use strict';
const current=new URLSearchParams(location.search).get('active')||'medium';
document.querySelectorAll('[data-size]').forEach((button)=>{
 const active=button.dataset.size===current;
 button.classList.toggle('selected',active);
 button.setAttribute('aria-pressed',String(active));
 button.onclick=()=>window.gekkoSize.choose(button.dataset.size);
});
document.addEventListener('keydown',(event)=>{if(event.key==='Escape')window.gekkoSize.dismiss()});
